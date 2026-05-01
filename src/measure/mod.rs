pub mod download;
pub mod latency;
pub mod speed;
pub mod upload;

use crate::api::Targets;
use anyhow::Result;
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Phases the orchestrator runs through, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    UnloadedLatency,
    Download,
    LoadedLatency,
    Upload,
}

/// Progress events emitted during measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    PhaseStart(Phase),
    Throughput { mbps: f64 },
    Latency { ms: f64 },
    PhaseEnd(Phase),
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub download_mbps: f64,
    pub upload_mbps: Option<f64>,
    pub unloaded_latency_ms: f64,
    pub loaded_latency_ms: f64,
    pub server_locations: Vec<String>,
    pub client_ip: String,
    pub client_isp: String,
}

pub struct Options {
    pub no_upload: bool,
}

/// Run the full measurement, optionally emitting Progress events.
pub async fn run_with_progress(
    client: &reqwest::Client,
    targets: &Targets,
    opts: &Options,
    progress: Option<tokio::sync::mpsc::Sender<Progress>>,
) -> Result<Report> {
    let urls: Vec<String> = targets.targets.iter().map(|t| t.url.clone()).collect();
    let first = urls.first().expect("targets is non-empty").clone();

    // 1. Unloaded latency
    if let Some(tx) = &progress {
        let _ = tx.send(Progress::PhaseStart(Phase::UnloadedLatency)).await;
    }
    let unloaded = latency::probe(client, &first, 10).await?;
    if let Some(tx) = &progress {
        for d in &unloaded {
            let _ = tx
                .send(Progress::Latency {
                    ms: d.as_secs_f64() * 1000.0,
                })
                .await;
        }
        let _ = tx.send(Progress::PhaseEnd(Phase::UnloadedLatency)).await;
    }
    let (unloaded_min, _) = latency::summarize_ms(&unloaded);

    // 2. Download
    if let Some(tx) = &progress {
        let _ = tx.send(Progress::PhaseStart(Phase::Download)).await;
    }
    let dl_shutdown = Arc::new(AtomicBool::new(false));
    let download_handle = {
        let client = client.clone();
        let urls = urls.clone();
        let shutdown = dl_shutdown.clone();
        let progress = progress.clone();
        tokio::spawn(
            async move { download::measure(&client, &urls, shutdown, progress.as_ref()).await },
        )
    };

    // Loaded-latency probes after a short ramp-up.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    if let Some(tx) = &progress {
        let _ = tx.send(Progress::PhaseStart(Phase::LoadedLatency)).await;
    }
    let loaded = latency::probe(client, &first, 5).await.unwrap_or_default();
    if let Some(tx) = &progress {
        for d in &loaded {
            let _ = tx
                .send(Progress::Latency {
                    ms: d.as_secs_f64() * 1000.0,
                })
                .await;
        }
        let _ = tx.send(Progress::PhaseEnd(Phase::LoadedLatency)).await;
    }
    let (loaded_min, _) = if loaded.is_empty() {
        (unloaded_min, unloaded_min)
    } else {
        latency::summarize_ms(&loaded)
    };

    let download_mbps = download_handle.await??;
    if let Some(tx) = &progress {
        let _ = tx.send(Progress::PhaseEnd(Phase::Download)).await;
    }

    // 3. Upload
    let upload_mbps = if opts.no_upload {
        None
    } else {
        if let Some(tx) = &progress {
            let _ = tx.send(Progress::PhaseStart(Phase::Upload)).await;
        }
        let ul_shutdown = Arc::new(AtomicBool::new(false));
        let mbps = upload::measure(client, &urls, ul_shutdown, progress.as_ref()).await?;
        if let Some(tx) = &progress {
            let _ = tx.send(Progress::PhaseEnd(Phase::Upload)).await;
        }
        Some(mbps)
    };

    let server_locations: Vec<String> = targets
        .targets
        .iter()
        .map(|t| format!("{}, {}", t.location.city, t.location.country))
        .collect();

    Ok(Report {
        download_mbps,
        upload_mbps,
        unloaded_latency_ms: unloaded_min,
        loaded_latency_ms: loaded_min,
        server_locations,
        client_ip: targets.client.ip.clone(),
        client_isp: targets.client.isp.clone(),
    })
}

/// Run the full measurement against the given targets.
pub async fn run(client: &reqwest::Client, targets: &Targets, opts: &Options) -> Result<Report> {
    run_with_progress(client, targets, opts, None).await
}
