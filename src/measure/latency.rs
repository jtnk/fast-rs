use anyhow::Result;
use std::time::{Duration, Instant};

/// Probe a target URL N times sequentially, return per-request durations.
pub async fn probe(client: &reqwest::Client, url: &str, n: usize) -> Result<Vec<Duration>> {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        let resp = client.get(url).send().await?;
        // Drain body so the connection completes its round-trip.
        let _ = resp.bytes().await?;
        samples.push(start.elapsed());
    }
    Ok(samples)
}

/// Min and arithmetic mean of a non-empty slice of durations, in milliseconds.
pub fn summarize_ms(samples: &[Duration]) -> (f64, f64) {
    assert!(!samples.is_empty());
    let ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    let min = ms.iter().cloned().fold(f64::INFINITY, f64::min);
    let avg = ms.iter().sum::<f64>() / ms.len() as f64;
    (min, avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_min_and_avg() {
        let samples = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
        ];
        let (min, avg) = summarize_ms(&samples);
        assert!((min - 10.0).abs() < 1e-9);
        assert!((avg - 20.0).abs() < 1e-9);
    }
}
