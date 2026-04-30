use crate::measure::speed::{bytes_to_mbps, is_stable};
use anyhow::Result;
use futures::stream::{self, FuturesUnordered, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);
const STABILITY_WINDOW: usize = 10;
const STABILITY_TOLERANCE: f64 = 0.05;
const MAX_DURATION: Duration = Duration::from_secs(15);
const CONNECTIONS: usize = 8;
const CHUNK_SIZE: usize = 64 * 1024;
const TOTAL_PAYLOAD: usize = 25 * 1024 * 1024; // each POST sends up to 25 MiB

/// Measure aggregate upload throughput (Mbps) across the given target URLs.
pub async fn measure(
    client: &reqwest::Client,
    urls: &[String],
    shutdown_signal: Arc<AtomicBool>,
) -> Result<f64> {
    let total_bytes = Arc::new(AtomicU64::new(0));

    let mut tasks = FuturesUnordered::new();
    for i in 0..CONNECTIONS {
        let url = urls[i % urls.len()].clone();
        let client = client.clone();
        let bytes = total_bytes.clone();
        let shutdown = shutdown_signal.clone();
        tasks.push(tokio::spawn(async move {
            stream_upload(&client, &url, bytes, shutdown).await
        }));
    }

    let start = Instant::now();
    let mut samples = Vec::new();
    let mut last_total: u64 = 0;
    let mut last_tick = Instant::now();

    let final_mbps = loop {
        sleep(SAMPLE_INTERVAL).await;
        let now = Instant::now();
        let cur = total_bytes.load(Ordering::Relaxed);
        let delta = cur.saturating_sub(last_total);
        let dt = now - last_tick;
        last_total = cur;
        last_tick = now;

        let mbps = bytes_to_mbps(delta, dt);
        samples.push(mbps);

        if is_stable(&samples, STABILITY_WINDOW, STABILITY_TOLERANCE)
            || start.elapsed() >= MAX_DURATION
        {
            let n = STABILITY_WINDOW.min(samples.len());
            break samples[samples.len() - n..].iter().sum::<f64>() / n as f64;
        }
    };

    shutdown_signal.store(true, Ordering::Relaxed);
    while tasks.next().await.is_some() {}
    Ok(final_mbps)
}

async fn stream_upload(
    client: &reqwest::Client,
    url: &str,
    counter: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    while !shutdown.load(Ordering::Relaxed) {
        let counter_inner = counter.clone();
        let shutdown_inner = shutdown.clone();
        let body_stream = stream::unfold(0usize, move |sent| {
            let counter = counter_inner.clone();
            let shutdown = shutdown_inner.clone();
            async move {
                if sent >= TOTAL_PAYLOAD || shutdown.load(Ordering::Relaxed) {
                    return None;
                }
                let chunk = vec![0u8; CHUNK_SIZE.min(TOTAL_PAYLOAD - sent)];
                counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                let next = sent + chunk.len();
                Some((Ok::<_, std::io::Error>(chunk), next))
            }
        });
        let body = reqwest::Body::wrap_stream(body_stream);
        let _ = client.post(url).body(body).send().await;
    }
    Ok(())
}
