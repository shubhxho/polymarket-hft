use polymarket_hft::simulation;
use std::{fmt::Write as _, fs};
fn artifacts(report: &simulation::Report) -> Result<(), Box<dyn std::error::Error>> {
    let mut md = String::from(
        "# Synthetic execution stress\n\nPassing means accounting and risk invariants held. It does not establish live readiness or profitability. Zero P&L can mean no trades. Remaining inventory is marked at available bid depth.\n\n![Median marked net P&L by stress and policy](simulations.svg)\n\n| Scenario | Fixed policy | Paths | Median net P&L ($) | Worst ($) | Profitable paths | Maximum drawdown ($) |\n|---|---|---:|---:|---:|---:|---:|\n",
    );
    let height = 100 + report.summaries.len() * 30;
    let largest = report
        .summaries
        .iter()
        .map(|r| r.median_pnl_micro.unsigned_abs())
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1100\" height=\"{height}\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">Synthetic marked net P&amp;L, not live profits</title><desc id=\"desc\">Median across fixed seeds. Left bars are losses; right bars gains. Zero may mean no fills.</desc><rect width=\"1100\" height=\"{height}\" fill=\"#0b1220\"/><g fill=\"#e5e7eb\" font-family=\"monospace\" font-size=\"13\"><text x=\"20\" y=\"28\">Synthetic stress: median marked net P&amp;L in dollars</text><text x=\"20\" y=\"50\">Fixed policies | charged replay fees | no profitability claim</text><path d=\"M 650 65 V {height}\" stroke=\"#64748b\"/>"
    );
    for (i, row) in report.summaries.iter().enumerate() {
        writeln!(
            md,
            "| {} | {} | {} | {:.4} | {:.4} | {} | {:.4} |",
            row.scenario,
            row.policy,
            row.paths,
            row.median_pnl_micro as f64 / 1e6,
            row.worst_pnl_micro as f64 / 1e6,
            row.profitable_paths,
            row.max_drawdown_micro as f64 / 1e6
        )?;
        let width = row.median_pnl_micro.unsigned_abs() as f64 / largest * 260.0;
        let x = if row.median_pnl_micro < 0 {
            650.0 - width
        } else {
            650.0
        };
        let color = if row.median_pnl_micro < 0 {
            "#fb7185"
        } else {
            "#38bdf8"
        };
        let y = 80 + i * 30;
        // Scenario and policy labels originate from fixed Rust constants.
        write!(
            svg,
            "<text x=\"20\" y=\"{y}\">{} / {}</text><rect x=\"{x:.2}\" y=\"{}\" width=\"{width:.2}\" height=\"16\" fill=\"{color}\"/><text x=\"930\" y=\"{y}\">${:.4}</text>",
            row.scenario,
            row.policy,
            y - 12,
            row.median_pnl_micro as f64 / 1e6
        )?;
    }
    svg.push_str("</g></svg>\n");
    md.push_str("\n## Assumptions\n\n");
    for assumption in &report.assumptions {
        writeln!(md, "- {assumption}")?;
    }
    md.push_str("\n[Raw paths, rejections and checks](simulations.json). [Methods and source inspiration](../docs/SIMULATIONS.md).\n");
    fs::create_dir_all("reports")?;
    for (name, content) in [("SIMULATIONS.md", md), ("simulations.svg", svg)] {
        let path = format!("reports/{name}");
        let temporary = format!("{path}.tmp");
        fs::write(&temporary, content)?;
        fs::rename(temporary, path)?;
    }
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let report_files = args.last().is_some_and(|s| s == "--report");
    if report_files {
        args.pop();
    }
    if args.len() > 2 {
        return Err("usage: hft-simulations [steps:8..10000] [seeds:1..64] [--report]".into());
    }
    let steps = args.first().map(|s| s.parse()).transpose()?.unwrap_or(2000);
    let seeds = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(16);
    let report = simulation::run(steps, seeds)?;
    if report_files {
        artifacts(&report)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
