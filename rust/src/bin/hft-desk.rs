use polymarket_hft::{desk, hardware, quant::Quantized};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const WEATHER: &str = include_str!("../../../reports/weather-10000.json");
const BENCH: &str = include_str!("../../../reports/benchmark-current-compact.json");
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    scenario: String,
    seed: u32,
    day: usize,
}
fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}
fn now() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
fn dispatch(
    request: &desk::Request,
    weather: &Value,
    meta: &Value,
    latest: &mut Vec<polymarket_hft::signals::SignalRequest>,
    model: Option<&hardware::LoadedModel>,
    time: u64,
) -> Result<Value, String> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/meta") => Ok(meta.clone()),
        ("GET", "/api/weather") => Ok(weather.clone()),
        ("GET", "/api/latest") => desk::rank(latest.clone(), time),
        ("POST", "/api/replay") => (|| {
            let r: Replay = serde_json::from_slice(&request.body).map_err(|e| e.to_string())?;
            let p = weather["portfolios"]
                .as_array()
                .ok_or("missing portfolios")?
                .iter()
                .find(|p| p["scenario"] == r.scenario && p["seed"] == r.seed)
                .ok_or("unknown_portfolio")?;
            let days = p["daily_equity_micro"]
                .as_array()
                .ok_or("missing daily equity")?;
            let cash = days
                .get(r.day)
                .and_then(Value::as_u64)
                .ok_or("day_out_of_range")?;
            let start = p["starting_cash_micro"]
                .as_u64()
                .ok_or("missing initial capital")?;
            Ok(
                json!({"day":r.day+1,"cash_micro":cash,"pnl_micro":i128::from(cash)-i128::from(start),"synthetic":true,"scenario":p["scenario"],"seed":p["seed"],"days":days.len(),"scope":"Accelerated playback of daily Rust simulation settlements; not intraday HFT","submission_enabled":false}),
            )
        })(),
        ("POST", "/api/signals") => (|| {
            let input = serde_json::from_slice(&request.body).map_err(|e| e.to_string())?;
            let result = desk::rank(input, now().map_err(|e| e.to_string())?)?;
            *latest = serde_json::from_slice(&request.body).map_err(|e| e.to_string())?;
            Ok(result)
        })(),
        ("POST", "/api/infer") => (|| {
            let model = model.ok_or("no_model_loaded: restart with weights.hqt and backend")?;
            let input: Vec<f32> =
                serde_json::from_slice(&request.body).map_err(|e| e.to_string())?;
            Ok(
                json!({"backend":model.backend(),"values":model.infer_batch(&input)?,"trained_model_verified":false,"calibrated_probability":false,"api_cost_usd":0}),
            )
        })(),
        _ => Err("not_found".into()),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    method: String,
    path: String,
    #[serde(default)]
    body: Value,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let stdio = args.first().is_some_and(|s| s == "--stdio");
    if stdio {
        args.remove(0);
    }
    if ![0, 1, 3].contains(&args.len()) {
        return Err(
            "usage: hft-desk [--stdio] [port] [weights.hqt cpu|auto|mlx-cpu|mlx-metal|cuda]".into(),
        );
    }
    let port = args
        .first()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(8787u16);
    if port == 0 {
        return Err("port must be nonzero".into());
    }
    let model = if args.len() == 3 {
        let mut bytes = Vec::new();
        File::open(&args[1])?
            .take(160 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 160 * 1024 * 1024 {
            return Err("model_too_large".into());
        }
        Some(hardware::LoadedModel::new(
            Quantized::decode(&bytes)?,
            &args[2],
        )?)
    } else {
        None
    };
    let weather: Value = serde_json::from_str(WEATHER)?;
    let benchmark: Value = serde_json::from_str(BENCH)?;
    let meta = json!({"version":env!("CARGO_PKG_VERSION"),"chain_id":137,"hardware":hardware::detect(),"model_backend":model.as_ref().map(|m| m.backend()),"model_loaded":model.is_some(),"submission_enabled":false,"api_cost_usd":0,"weather_sha256":format!("{:x}",Sha256::digest(WEATHER.as_bytes())),"benchmark_sha256":format!("{:x}",Sha256::digest(BENCH.as_bytes())),"benchmark":benchmark,"weather":{"portfolios":weather["portfolios"].as_array().ok_or("missing portfolios")?.len(),"contracts":weather["contracts"],"buckets":weather["evaluated_buckets"]},"credibility":{"company_affiliation":"Unverified; no affiliation claimed","external_comparative_win":false,"profitability_established":false,"data":"synthetic weather / user-supplied candidate data"}});
    if stdio {
        let stdin = std::io::stdin();
        let mut lines = polymarket_hft::buffer::Lines::new(stdin.lock(), desk::BODY_LIMIT)?;
        let stdout = std::io::stdout();
        let mut output = polymarket_hft::buffer::JsonLines::new(stdout.lock(), true);
        let mut latest = Vec::new();
        while let Some((_, bytes)) = lines.next_line()? {
            let envelope: Envelope = serde_json::from_slice(bytes)?;
            let request = desk::Request {
                method: envelope.method,
                path: envelope.path,
                body: serde_json::to_vec(&envelope.body)?,
            };
            let value = match dispatch(
                &request,
                &weather,
                &meta,
                &mut latest,
                model.as_ref(),
                now()?,
            ) {
                Ok(data) => json!({"ok":true,"data":data}),
                Err(error) => json!({"ok":false,"error":error}),
            };
            output.record(&value)?;
        }
        output.finish()?;
        return Ok(());
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!(
        "Desk: http://127.0.0.1:{port} | local research | no order submission | no hosted inference calls"
    );
    let mut window = now()?;
    let mut requests = 0u32;
    let mut latest = Vec::new();
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else { continue };
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let time = now()?;
        if time.saturating_sub(window) >= 1000 {
            window = time;
            requests = 0;
        }
        requests += 1;
        if requests > 32 {
            let _ = respond(
                &mut stream,
                "429 Too Many Requests",
                "application/json",
                br#"{"error":"local_rate_limit"}"#,
            );
            continue;
        }
        let request = match desk::read_request(&mut BufReader::new(&mut stream), port) {
            Ok(r) => r,
            Err(reason) => {
                let body = serde_json::to_vec(&json!({"error":reason}))?;
                let _ = respond(&mut stream, "400 Bad Request", "application/json", &body);
                continue;
            }
        };
        let asset: Option<(&str, &[u8])> = if request.method == "GET" {
            match request.path.as_str() {
                "/" => Some((
                    "text/html; charset=utf-8",
                    include_bytes!("../../../ui/index.html"),
                )),
                "/app.css" => Some(("text/css", include_bytes!("../../../ui/app.css"))),
                "/app.js" => Some(("text/javascript", include_bytes!("../../../ui/app.js"))),
                "/InterVariable.woff2" => Some((
                    "font/woff2",
                    include_bytes!("../../../ui/InterVariable.woff2"),
                )),
                "/desk.gif" => Some((
                    "image/gif",
                    include_bytes!("../../../reports/weather-release.gif"),
                )),
                _ => None,
            }
        } else {
            None
        };
        if let Some((kind, bytes)) = asset {
            let _ = respond(&mut stream, "200 OK", kind, bytes);
            continue;
        }
        let result = dispatch(&request, &weather, &meta, &mut latest, model.as_ref(), time);
        let (status, value) = match result {
            Ok(value) => ("200 OK", value),
            Err(error) => (
                if error == "not_found" {
                    "404 Not Found"
                } else {
                    "422 Unprocessable Entity"
                },
                json!({"error":error}),
            ),
        };
        let body = serde_json::to_vec(&value)?;
        let _ = respond(&mut stream, status, "application/json", &body);
    }
    Ok(())
}
