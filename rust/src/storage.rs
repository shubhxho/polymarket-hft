//! Hash-chained single-writer paper-state WAL. Events are flushed before state changes.
use crate::{
    Result,
    engine::{Config, Engine, Event, Fill},
    news::{JevResponse, NewsDecision, NewsItem, NewsPipeline, Route},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};
const MAX_FRAME: usize = 1_048_576;
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
#[derive(Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    Config {
        version: u32,
        config: Config,
    },
    Event {
        event: Event,
    },
    News {
        item: NewsItem,
        response: Option<JevResponse>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    number: u64,
    previous: String,
    payload: String,
    hash: String,
}
fn digest(number: u64, previous: &str, payload: &str) -> String {
    let mut h = Sha256::new();
    h.update(number.to_le_bytes());
    h.update(previous.as_bytes());
    h.update(payload.as_bytes());
    format!("{:x}", h.finalize())
}
fn event_hash(event: &Event) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(event).map_err(|e| e.to_string())?)
    ))
}
pub struct Journal {
    file: File,
    engine: Engine,
    news: NewsPipeline,
    number: u64,
    previous: String,
    events: Vec<String>,
    poisoned: bool,
    pub recovered_tail_bytes: u64,
}
impl Journal {
    pub fn open(path: impl AsRef<Path>, requested: Option<Config>) -> Result<Self> {
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("journal_open:{e}"))?;
        file.try_lock()
            .map_err(|_| "journal_already_locked_or_lock_unavailable")?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        let mut reader = BufReader::new(file.try_clone().map_err(|e| e.to_string())?);
        let (mut offset, mut number) = (0u64, 0u64);
        let mut previous = ZERO.to_owned();
        let mut engine = None;
        let mut news = NewsPipeline::default();
        let mut events = Vec::new();
        let mut recovered_tail_bytes = 0;
        loop {
            let mut bytes = Vec::new();
            let n = (&mut reader)
                .take((MAX_FRAME + 1) as u64)
                .read_until(b'\n', &mut bytes)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if n > MAX_FRAME {
                return Err("journal_frame_too_large".into());
            }
            if bytes.last() != Some(&b'\n') {
                recovered_tail_bytes = len - offset;
                file.set_len(offset).map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                break;
            }
            let f: Frame = serde_json::from_slice(&bytes)
                .map_err(|_| format!("journal_corrupt_frame_at:{offset}"))?;
            if f.number != number + 1
                || f.previous != previous
                || f.hash != digest(f.number, &f.previous, &f.payload)
            {
                return Err(format!("journal_integrity_failure_at:{offset}"));
            }
            let record: Record =
                serde_json::from_str(&f.payload).map_err(|_| "journal_invalid_payload")?;
            match record {
                Record::Config { version, config } if number == 0 && version == 1 => {
                    if requested.as_ref().is_some_and(|c| c != &config) {
                        return Err("journal_config_mismatch".into());
                    }
                    engine = Some(Engine::new(config)?);
                }
                Record::Event { event } if engine.is_some() => {
                    events.push(event_hash(&event)?);
                    let _ = engine.as_mut().unwrap().apply(event);
                }
                Record::News { item, response } if engine.is_some() => {
                    let d = news.ingest(&item, response.as_ref())?;
                    if d.route == Route::Halt {
                        engine.as_mut().unwrap().apply(Event::Halt {
                            reason: format!("news:{}", d.id),
                        })?;
                    }
                }
                _ => return Err("journal_invalid_record_order_or_version".into()),
            }
            number = f.number;
            previous = f.hash;
            offset += n as u64;
        }
        drop(reader);
        file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
        let empty = engine.is_none();
        let engine = match engine {
            Some(e) => e,
            None => Engine::new(requested.unwrap_or_default())?,
        };
        let mut journal = Self {
            file,
            engine,
            news,
            number,
            previous,
            events,
            poisoned: false,
            recovered_tail_bytes,
        };
        if empty {
            journal.append(Record::Config {
                version: 1,
                config: journal.engine.config().clone(),
            })?;
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|e| format!("journal_parent_sync:{e}"))?;
        }
        Ok(journal)
    }
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    pub fn is_replayed(&self, index: usize, event: &Event) -> Result<bool> {
        if let Some(expected) = self.events.get(index) {
            if &event_hash(event)? != expected {
                return Err(format!("replay_prefix_mismatch_at:{}", index + 1));
            }
            return Ok(true);
        }
        if index != self.events.len() {
            return Err("non_contiguous_replay_index".into());
        }
        Ok(false)
    }
    pub fn apply(&mut self, event: Event) -> Result<std::result::Result<Option<Fill>, String>> {
        let hash = event_hash(&event)?;
        self.append(Record::Event {
            event: event.clone(),
        })?;
        self.events.push(hash);
        Ok(self.engine.apply(event))
    }
    pub fn validate_news(&self, item: &NewsItem) -> Result<String> {
        self.news.validate(item)
    }
    pub fn ingest_news(
        &mut self,
        item: NewsItem,
        response: Option<JevResponse>,
    ) -> Result<NewsDecision> {
        let mut staged = self.news.clone();
        let d = staged.ingest(&item, response.as_ref())?;
        self.append(Record::News { item, response })?;
        self.news = staged;
        if d.route == Route::Halt {
            self.engine.apply(Event::Halt {
                reason: format!("news:{}", d.id),
            })?;
        }
        Ok(d)
    }
    fn append(&mut self, record: Record) -> Result<()> {
        if self.poisoned {
            return Err("journal_poisoned_restart_required".into());
        }
        let payload = serde_json::to_string(&record).map_err(|e| e.to_string())?;
        let number = self
            .number
            .checked_add(1)
            .ok_or("journal_sequence_overflow")?;
        let hash = digest(number, &self.previous, &payload);
        let f = Frame {
            number,
            previous: self.previous.clone(),
            payload,
            hash: hash.clone(),
        };
        let mut bytes = serde_json::to_vec(&f).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err("journal_frame_too_large".into());
        }
        let offset = self.file.stream_position().map_err(|e| e.to_string())?;
        if let Err(error) = self
            .file
            .write_all(&bytes)
            .and_then(|_| self.file.sync_all())
        {
            self.poisoned = true;
            let _ = self.file.set_len(offset);
            let _ = self.file.sync_all();
            return Err(format!("journal_write_failed:{error}"));
        }
        self.number = number;
        self.previous = hash;
        Ok(())
    }
}
