//! Render a small terminal-style GIF from the actual synthetic weather report.
use serde_json::Value;
use std::{fs, io::Write, path::PathBuf, process::Command};
const W: usize = 640;
const H: usize = 300;
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '-' | '_' => [0, 0, 0, 31, 0, 0, 0],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '$' => [4, 15, 20, 14, 5, 30, 4],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        _ => [0; 7],
    }
}
fn text(pixels: &mut [u8], text: &str, y: usize, color: [u8; 3]) {
    for (i, c) in text.to_ascii_uppercase().chars().take(49).enumerate() {
        for (row, bits) in glyph(c).into_iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..2 {
                    for dx in 0..2 {
                        let x = 24 + i * 12 + col * 2 + dx;
                        let y = y + row * 2 + dy;
                        if y < H {
                            let n = (y * W + x) * 3;
                            pixels[n..n + 3].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }
}
struct Frames(PathBuf);
impl Drop for Frames {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let desk = args == ["--desk"];
    if !args.is_empty() && !desk {
        return Err("usage: hft-release [--desk]".into());
    }
    let path = "reports/weather-10000.json";
    if fs::metadata(path)?.len() > 8 * 1024 * 1024 {
        return Err("weather report too large".into());
    }
    let report: Value = serde_json::from_slice(&fs::read(path)?)?;
    if report["synthetic"] != true || report["starting_bankroll_micro"] != 10_000_000_000u64 {
        return Err("unexpected weather report".into());
    }
    let portfolios = report["portfolios"]
        .as_array()
        .ok_or("missing portfolios")?;
    let directory = Frames(PathBuf::from(format!(".local/gif-{}", std::process::id())));
    fs::create_dir_all(&directory.0)?;
    let mut slides = vec![vec![
        "RUST / POLYGON PAPER RESEARCH".into(),
        "WEATHER PORTFOLIO REPLAY".into(),
        "$10,000 PER INDEPENDENT PATH".into(),
        format!("{} PORTFOLIOS / 8 CITIES", portfolios.len()),
        "HIGH + LOW / ALL FIVE BUCKETS".into(),
        "SYNTHETIC DATA - NO REAL MONEY".into(),
    ]];
    for scenario in polymarket_hft::weather::SCENARIOS {
        let mut values: Vec<f64> = portfolios
            .iter()
            .filter(|p| p["scenario"] == scenario)
            .map(|p| {
                p["pnl_micro"]
                    .as_i64()
                    .map(|v| v as f64 / 1e6)
                    .ok_or("missing pnl")
            })
            .collect::<Result<_, _>>()?;
        if values.is_empty() {
            return Err("missing scenario".into());
        }
        values.sort_by(f64::total_cmp);
        let median = (values[(values.len() - 1) / 2] + values[values.len() / 2]) / 2.;
        slides.push(vec![
            "RUST / POLYGON PAPER RESEARCH".into(),
            scenario.replace('_', " "),
            format!("MEDIAN PNL: ${median:+.2}"),
            format!("WORST PNL: ${:+.2}", values[0]),
            "20 DAYS / 16 SEEDED PORTFOLIOS".into(),
            "PROFITABILITY IS NOT ESTABLISHED".into(),
        ]);
    }
    if desk {
        let p = portfolios
            .iter()
            .find(|p| p["scenario"] == "normal" && p["seed"] == 0)
            .ok_or("missing demo portfolio")?;
        let equity = p["daily_equity_micro"].as_array().ok_or("missing equity")?;
        slides.clear();
        for (index, cash) in std::iter::once(10_000_000_000u64)
            .chain(equity.iter().map(|v| v.as_u64().unwrap_or(0)))
            .enumerate()
        {
            slides.push(vec![
                "PM / RESEARCH DESK - RUST CORE".into(),
                "SIMULATION / DATA / EVIDENCE / INFERENCE".into(),
                "-----------------------------------------".into(),
                format!("POLYGON 137 / NORMAL / SEED 0 / DAY {index:02}"),
                format!("SETTLED CASH   $ {:.2}", cash as f64 / 1e6),
                format!(
                    "PAPER PNL      $ {:+.2}",
                    (cash as f64 - 10_000_000_000.) / 1e6
                ),
                "FORECAST / FEE CURVE / KELLY / SETTLEMENT".into(),
                "INPUT: SYNTHETIC / NOT HISTORICAL".into(),
                "DATA MODE: SUPPLIED CANDIDATES ONLY".into(),
                "MODEL: OPTIONAL LOCAL PACKED WEIGHTS".into(),
                "HOSTED API CALLS: 0 / SUBMISSION: OFF".into(),
                "LOSS INCLUDED / PROFITABILITY UNPROVEN".into(),
            ]);
        }
    }
    for (index, lines) in slides.iter().enumerate() {
        let mut pixels = [12, 18, 28].repeat(W * H);
        for (row, line) in lines.iter().enumerate() {
            text(
                &mut pixels,
                line,
                if desk { 18 + row * 23 } else { 24 + row * 40 },
                if row == 0 {
                    [52, 211, 153]
                } else if row == if desk { 11 } else { 5 } {
                    [251, 146, 146]
                } else {
                    [226, 232, 240]
                },
            );
        }
        let mut file = fs::File::create(directory.0.join(format!("frame-{index:03}.ppm")))?;
        write!(file, "P6\n{W} {H}\n255\n")?;
        file.write_all(&pixels)?;
    }
    let output = if desk {
        "reports/desk-release.gif"
    } else {
        "reports/weather-release.gif"
    };
    let temporary = if desk {
        "reports/desk-release.tmp.gif"
    } else {
        "reports/weather-release.tmp.gif"
    };
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-framerate", "1/2", "-i"])
        .arg(directory.0.join("frame-%03d.ppm"))
        .args([
            "-vf",
            "split[a][b];[a]palettegen[p];[b][p]paletteuse",
            "-loop",
            "0",
            temporary,
        ])
        .status()?;
    if !status.success() {
        let _ = fs::remove_file(temporary);
        return Err("ffmpeg failed".into());
    }
    fs::rename(temporary, output)?;
    println!("Rendered {output} from measured synthetic outcomes");
    Ok(())
}
