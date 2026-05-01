use crate::measure::speed::{bytes_to_mbps, is_stable};
use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);
const STABILITY_WINDOW: usize = 10; // 2 s at 200 ms cadence
const STABILITY_TOLERANCE: f64 = 0.05;
const MAX_DURATION: Duration = Duration::from_secs(15);
const CONNECTIONS: usize = 8;

/// Measure aggregate download throughput (Mbps) across the given target URLs.
///
/// The shutdown_signal arc is set true when the loop decides we're done; spawned
/// download tasks watch it to stop reading.
pub async fn measure(
    client: &reqwest::Client,
    urls: &[String],
    shutdown_signal: Arc<std::sync::atomic::AtomicBool>,
    progress: Option<&tokio::sync::mpsc::Sender<crate::measure::Progress>>,
) -> Result<f64> {
    let total_bytes = Arc::new(AtomicU64::new(0));

    let mut tasks = FuturesUnordered::new();
    for i in 0..CONNECTIONS {
        let url = urls[i % urls.len()].clone();
        let client = client.clone();
        let bytes = total_bytes.clone();
        let shutdown = shutdown_signal.clone();
        tasks.push(tokio::spawn(async move {
            stream_download(&client, &url, bytes, shutdown).await
        }));
    }

    let start = Instant::now();
    let mut samples = Vec::new();
    let mut last_total: u64 = 0;
    let mut last_tick = Instant::now();
    let final_mbps;

    loop {
        sleep(SAMPLE_INTERVAL).await;
        let now = Instant::now();
        let cur = total_bytes.load(Ordering::Relaxed);
        let delta_bytes = cur.saturating_sub(last_total);
        let delta_t = now - last_tick;
        last_total = cur;
        last_tick = now;

        let mbps = bytes_to_mbps(delta_bytes, delta_t);
        samples.push(mbps);

        if let Some(tx) = progress {
            let _ = tx.send(crate::measure::Progress::Throughput { mbps }).await;
        }

        if is_stable(&samples, STABILITY_WINDOW, STABILITY_TOLERANCE)
            || start.elapsed() >= MAX_DURATION
        {
            final_mbps = recent_avg(&samples, STABILITY_WINDOW);
            break;
        }
    }

    shutdown_signal.store(true, Ordering::Relaxed);
    while tasks.next().await.is_some() {}
    Ok(final_mbps)
}

fn recent_avg(samples: &[f64], window: usize) -> f64 {
    let n = window.min(samples.len());
    if n == 0 {
        return 0.0;
    }
    samples[samples.len() - n..].iter().sum::<f64>() / n as f64
}

async fn stream_download(
    client: &reqwest::Client,
    url: &str,
    counter: Arc<AtomicU64>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    while !shutdown.load(Ordering::Relaxed) {
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(_) => continue, // transient; try again
        };
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if shutdown.load(Ordering::Relaxed) {
                return Ok(());
            }
            match chunk {
                Ok(bytes) => {
                    counter.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                }
                Err(_) => break,
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_avg_handles_empty() {
        assert_eq!(recent_avg(&[], 5), 0.0);
    }

    #[test]
    fn recent_avg_limits_to_window() {
        assert!((recent_avg(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3) - 5.0).abs() < 1e-9);
    }
}
