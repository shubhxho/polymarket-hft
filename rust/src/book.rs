use crate::{
    Result,
    base::{BOOK_LEVELS, PRICE_SCALE},
};
use serde::{Deserialize, Serialize};
const MASK_WORDS: usize = BOOK_LEVELS.div_ceil(64);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub price: u32,
    pub shares: u32,
}

#[derive(Clone)]
pub struct Book {
    pub tick: u32,
    bids: [u32; BOOK_LEVELS],
    asks: [u32; BOOK_LEVELS],
    bid_mask: [u64; MASK_WORDS],
    ask_mask: [u64; MASK_WORDS],
    bid_top: Option<usize>,
    ask_top: Option<usize>,
    pub sequence: u64,
    pub time_ms: u64,
    synchronized: bool,
}

impl Book {
    pub fn new(tick: u32) -> Result<Self> {
        if tick == 0
            || !PRICE_SCALE.is_multiple_of(tick)
            || PRICE_SCALE / tick >= BOOK_LEVELS as u32
        {
            return Err("unsupported_tick".into());
        }
        Ok(Self {
            tick,
            bids: [0; BOOK_LEVELS],
            asks: [0; BOOK_LEVELS],
            bid_mask: [0; MASK_WORDS],
            ask_mask: [0; MASK_WORDS],
            bid_top: None,
            ask_top: None,
            sequence: 0,
            time_ms: 0,
            synchronized: false,
        })
    }
    fn index(&self, price: u32) -> Result<usize> {
        if price == 0 || price >= PRICE_SCALE || !price.is_multiple_of(self.tick) {
            return Err("invalid_price_or_tick".into());
        }
        Ok((price / self.tick) as usize)
    }
    pub fn snapshot(
        &mut self,
        bids: &[Level],
        asks: &[Level],
        sequence: u64,
        time_ms: u64,
    ) -> Result<()> {
        if sequence == 0 || sequence <= self.sequence || time_ms < self.time_ms {
            return Err("stale_snapshot".into());
        }
        self.synchronized = false;
        let mut next = Self::new(self.tick)?;
        for (levels, side) in [(bids, Side::Buy), (asks, Side::Sell)] {
            for level in levels {
                let i = next.index(level.price)?;
                let array = if side == Side::Buy {
                    &mut next.bids
                } else {
                    &mut next.asks
                };
                if array[i] != 0 || level.shares == 0 {
                    return Err("duplicate_or_empty_level".into());
                }
                next.set(side, i, level.shares);
            }
        }
        next.validate_spread()?;
        next.sequence = sequence;
        next.time_ms = time_ms;
        next.synchronized = true;
        *self = next;
        Ok(())
    }
    pub fn update(&mut self, side: Side, level: Level, sequence: u64, time_ms: u64) -> Result<()> {
        if !self.synchronized
            || sequence != self.sequence.saturating_add(1)
            || time_ms < self.time_ms
        {
            self.synchronized = false;
            return Err("sequence_gap_or_stale_update: snapshot_required".into());
        }
        let i = match self.index(level.price) {
            Ok(i) => i,
            Err(error) => {
                self.synchronized = false;
                return Err(error);
            }
        };
        let old = if side == Side::Buy {
            self.bids[i]
        } else {
            self.asks[i]
        };
        self.set(side, i, level.shares);
        if let Err(error) = self.validate_spread() {
            self.set(side, i, old);
            self.synchronized = false;
            return Err(error);
        }
        self.sequence = sequence;
        self.time_ms = time_ms;
        Ok(())
    }
    fn validate_spread(&self) -> Result<()> {
        if let (Some(bid), Some(ask)) = (self.best_bid(), self.best_ask())
            && bid >= ask
        {
            return Err("crossed_book".into());
        }
        Ok(())
    }
    pub fn best_bid(&self) -> Option<u32> {
        self.bid_top.map(|i| i as u32 * self.tick)
    }
    pub fn best_ask(&self) -> Option<u32> {
        self.ask_top.map(|i| i as u32 * self.tick)
    }
    pub fn ready(&self, now: u64, max_age: u64) -> bool {
        self.synchronized && now >= self.time_ms && now - self.time_ms <= max_age
    }
    pub fn levels(&self, side: Side) -> impl Iterator<Item = Level> + '_ {
        let (array, masks, reverse) = if side == Side::Buy {
            (&self.asks, &self.ask_mask, false)
        } else {
            (&self.bids, &self.bid_mask, true)
        };
        (0..MASK_WORDS)
            .map(move |n| if reverse { MASK_WORDS - 1 - n } else { n })
            .flat_map(move |word| {
                let mut mask = masks[word];
                std::iter::from_fn(move || {
                    if mask == 0 {
                        return None;
                    }
                    let bit = if reverse {
                        63 - mask.leading_zeros()
                    } else {
                        mask.trailing_zeros()
                    } as usize;
                    mask &= !(1u64 << bit);
                    let i = word * 64 + bit;
                    Some(Level {
                        price: i as u32 * self.tick,
                        shares: array[i],
                    })
                })
            })
    }
    pub(crate) fn consume(&mut self, side: Side, fills: &[Level]) {
        let book_side = if side == Side::Buy {
            Side::Sell
        } else {
            Side::Buy
        };
        for fill in fills {
            let i = (fill.price / self.tick) as usize;
            let old = if book_side == Side::Buy {
                self.bids[i]
            } else {
                self.asks[i]
            };
            self.set(book_side, i, old - fill.shares);
        }
    }
    fn set(&mut self, side: Side, i: usize, shares: u32) {
        let (array, mask, top) = if side == Side::Buy {
            (&mut self.bids, &mut self.bid_mask, &mut self.bid_top)
        } else {
            (&mut self.asks, &mut self.ask_mask, &mut self.ask_top)
        };
        array[i] = shares;
        if shares > 0 {
            mask[i / 64] |= 1u64 << (i % 64);
            if top.is_none_or(|t| if side == Side::Buy { i > t } else { i < t }) {
                *top = Some(i);
            }
        } else {
            mask[i / 64] &= !(1u64 << (i % 64));
            if *top == Some(i) {
                *top = if side == Side::Buy {
                    mask.iter()
                        .rposition(|&m| m != 0)
                        .map(|w| w * 64 + (63 - mask[w].leading_zeros()) as usize)
                } else {
                    mask.iter()
                        .position(|&m| m != 0)
                        .map(|w| w * 64 + mask[w].trailing_zeros() as usize)
                };
            }
        }
    }
}
