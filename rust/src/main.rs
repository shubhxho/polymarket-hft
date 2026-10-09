use polymarket_hft::{
    Result, base,
    buffer::Lines,
    engine::{Config, Engine, Event},
    model::{Bucket, Metric, probabilities},
    news::{JevClient, JevResponse, NewsItem},
    storage::Journal,
};
use serde::Deserialize;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Forecast {
    buckets: Vec<Bucket>,
    metric: Metric,
    observed: f64,
    mu: f64,
    sigma: f64,
    hours: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewsEnvelope {
    item: NewsItem,
    response: Option<JevResponse>,
}
fn lines(path: &str, mut consume: impl FnMut(usize, &str) -> Result<()>) -> Result<usize> {
    let mut reader: Box<dyn BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        Box::new(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
    };
    let mut index = 0;
    let mut input = Lines::new(&mut reader, 1_048_576).map_err(|e| e.to_string())?;
    while let Some((physical, bytes)) = input.next_line().map_err(|e| e.to_string())? {
        let line =
            std::str::from_utf8(bytes).map_err(|_| format!("line {physical}: input_not_utf8"))?;
        if line.trim().is_empty() {
            continue;
        }
        consume(index, line)?;
        index += 1;
    }
    Ok(index)
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("durable-replay") if (3..=4).contains(&args.len())=>{
            let config=if let Some(path)=args.get(3){Some(serde_json::from_reader(File::open(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?)}else{None};
            let mut journal=Journal::open(&args[2],config)?;
            let recovered=journal.event_count();let mut applied=0;
            let count=lines(&args[1],|i,line|{let event:Event=serde_json::from_str(line).map_err(|e|format!("line {}:{e}",i+1))?;if journal.is_replayed(i,&event)?{println!("{}",serde_json::json!({"event":i+1,"recovered":true}));}else{match journal.apply(event)?{Ok(fill)=>println!("{}",serde_json::json!({"event":i+1,"fill":fill})),Err(reason)=>println!("{}",serde_json::json!({"event":i+1,"rejected":reason}))}applied+=1;}Ok(())})?;
            if count<recovered{return Err("replay_input_shorter_than_journal_prefix".into());}
            println!("{}",serde_json::json!({"summary":{"events":count,"new_events":applied,"recovered_events":recovered,"cash_micro":journal.engine().cash_micro(),"realized_pnl_micro":journal.engine().realized_pnl_micro().to_string(),"recovered_tail_bytes":journal.recovered_tail_bytes}}));
        }
        Some("journal") if args.len()==2=>{
            let journal=Journal::open(&args[1],None)?;println!("{}",serde_json::json!({"events":journal.event_count(),"cash_micro":journal.engine().cash_micro(),"realized_pnl_micro":journal.engine().realized_pnl_micro().to_string(),"recovered_tail_bytes":journal.recovered_tail_bytes,"chain_id":base::CHAIN_ID}));
        }
        Some("news") if args.len()==3||(args.len()==4&&args[3]=="--jev")=>{
            let mut journal=Journal::open(&args[2],None)?;
            let client=if args.len()==4{Some(JevClient::from_env()?)}else{None};
            lines(&args[1],|i,line|{
                let mut envelope:NewsEnvelope=serde_json::from_str(line).map_err(|e|format!("line {}:{e}",i+1))?;
                if let Err(reason)=journal.validate_news(&envelope.item){println!("{}",serde_json::json!({"id":envelope.item.id,"rejected":reason}));return Ok(());}
                let mut service_error=None;
                if let Some(c)=&client{if envelope.response.is_some(){return Err("recorded_response_with_live_jev_mode".into());}match c.judge(&envelope.item){Ok(r)=>envelope.response=Some(r),Err(reason)=>service_error=Some(reason)}}
                match journal.ingest_news(envelope.item,envelope.response){Ok(decision)=>println!("{}",serde_json::json!({"decision":decision,"service_error":service_error})),Err(reason)=>println!("{}",serde_json::json!({"rejected":reason}))}Ok(())
            })?;
        }
        Some("fair") if args.len() == 2 => {
            let f: Forecast =
                serde_json::from_reader(File::open(&args[1]).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let p = probabilities(&f.buckets, f.metric, f.observed, f.mu, f.sigma, f.hours)?;
            println!(
                "{}",
                serde_json::json!({"probabilities":p,"model":"untrained_truncated_normal","live_ready":false})
            );
        }
        Some("base") if args.len() == 1 => println!(
            "{}",
            serde_json::json!({"chain": base::CHAIN_NAME, "chain_id": base::CHAIN_ID, "rpc": base::RPC, "live": false, "reason": "No verified Base Polymarket exchange deployment; settlement unavailable under Base-only policy"})
        ),
        Some("live") => return Err(
            "Base-only live execution unavailable: no verified Base Polymarket exchange deployment"
                .into(),
        ),
        Some("replay") if (2..=3).contains(&args.len()) => {
            let config = if let Some(path) = args.get(2) {
                serde_json::from_reader(File::open(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?
            } else {
                Config::default()
            };
            let mut engine = Engine::new(config)?;
            let (mut events,mut fills,mut rejects)=(0u64,0u64,0u64);
            lines(&args[1],|n,line| {
                let event:Event=serde_json::from_str(line).map_err(|e|format!("line {}: {e}",n+1))?;
                events+=1;
                match engine.apply(event) {
                    Ok(Some(fill))=>{fills+=1;println!("{}",serde_json::to_string(&fill).map_err(|e|e.to_string())?);}
                    Ok(None)=>(),
                    Err(reason)=>{rejects+=1;println!("{}",serde_json::json!({"line":n+1,"rejected":reason}));}
                }
                Ok(())
            })?;
            println!(
                "{}",
                serde_json::json!({"summary": {"events": events, "fills": fills, "rejects": rejects, "cash_micro": engine.cash_micro(), "realized_pnl_micro": engine.realized_pnl_micro().to_string(), "paper": true, "chain_id": base::CHAIN_ID}})
            );
        }
        _ => return Err(
            "usage: hft base | fair <forecast.json> | replay <jsonl|-> [config.json] | durable-replay <jsonl|-> <journal> [config.json] | journal <journal> | news <jsonl|-> <journal> [--jev] | live"
                .into(),
        ),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
