use crate::measure::Report;
use anyhow::Result;
use std::io::Write;

/// Print final JSON to stdout.
pub fn render_json(report: &Report) -> Result<()> {
    let s = serde_json::to_string_pretty(report)?;
    println!("{s}");
    Ok(())
}

/// One-line human-readable summary.
pub fn render_single_line(report: &Report) -> String {
    let upload = match report.upload_mbps {
        Some(u) => format!("↑ {u:.1} Mbps"),
        None => "↑ skipped".to_string(),
    };
    format!(
        "↓ {:.1} Mbps  {}  latency {:.0}/{:.0} ms (unloaded/loaded)  via {}",
        report.download_mbps,
        upload,
        report.unloaded_latency_ms,
        report.loaded_latency_ms,
        report
            .server_locations
            .first()
            .map(|s| s.as_str())
            .unwrap_or("unknown"),
    )
}

/// Multi-line human-readable summary printed when live updates are done.
pub fn render_summary(report: &Report) {
    println!("Download:        {:>7.1} Mbps", report.download_mbps);
    match report.upload_mbps {
        Some(u) => println!("Upload:          {:>7.1} Mbps", u),
        None => println!("Upload:          (skipped)"),
    }
    println!("Latency unloaded:{:>7.0} ms", report.unloaded_latency_ms);
    println!("Latency loaded:  {:>7.0} ms", report.loaded_latency_ms);
    println!(
        "Client:          {} / {}",
        report.client_ip, report.client_isp
    );
    if let Some(loc) = report.server_locations.first() {
        println!("Server:          {}", loc);
    }
}

/// In-place updating progress line on stderr.
pub struct LiveRenderer;

impl LiveRenderer {
    pub fn update(&mut self, label: &str, mbps: f64) {
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r{label}: {mbps:>7.1} Mbps    ");
        let _ = err.flush();
    }

    pub fn finish(&mut self) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Report {
        Report {
            download_mbps: 123.4,
            upload_mbps: Some(45.6),
            unloaded_latency_ms: 12.0,
            loaded_latency_ms: 38.0,
            server_locations: vec!["Dublin, Ireland".to_string()],
            client_ip: "203.0.113.7".to_string(),
            client_isp: "TestNet".to_string(),
        }
    }

    #[test]
    fn json_round_trip() {
        let report = fixture();
        let s = serde_json::to_string(&report).unwrap();
        assert!(s.contains("\"download_mbps\":123.4"));
        assert!(s.contains("\"upload_mbps\":45.6"));
        assert!(s.contains("\"client_ip\":\"203.0.113.7\""));
    }

    #[test]
    fn single_line_includes_all_metrics() {
        let s = render_single_line(&fixture());
        assert!(s.contains("123.4"));
        assert!(s.contains("45.6"));
        assert!(s.contains("12"));
        assert!(s.contains("Dublin"));
    }

    #[test]
    fn single_line_when_upload_skipped() {
        let mut r = fixture();
        r.upload_mbps = None;
        let s = render_single_line(&r);
        assert!(s.contains("skipped"));
    }
}
