# fastrs-cli Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** A pure-HTTP Rust CLI (`fastrs`) that measures download/upload throughput, unloaded latency, and bufferbloat against Netflix's fast.com endpoints — modeled after `fast-cli` for surface area, ported in spirit from `fastcom-speed-cli`.

**Architecture:** Three sequential phases — token scrape → target discovery → measurement. The measurement phase fans out 8 concurrent reqwest streams using `tokio` + `futures`, samples cumulative bytes every 200 ms, and computes Mbps. Output is human-friendly (live updating) by default, JSON with `--json`. No headless browser.

**Tech Stack:** Rust 2021, `tokio` (rt-multi-thread), `reqwest` (rustls-tls + stream), `clap` (derive), `serde` + `serde_json`, `regex`, `anyhow`, `futures`. Dev: `wiremock`.

**Reference:** Design doc at `docs/plans/2026-04-30-fastrs-cli-design.md`.

**Conventions for the implementer:**
- TDD where the unit is testable in isolation (parsers, pure helpers, output formatters). Network measurement code is integration-tested behind `#[ignore]`.
- Commit after each task completes (test green + code in place).
- Run `cargo fmt` before each commit. Run `cargo clippy -- -D warnings` before tasks 11 and 12.
- Use `anyhow::Result<T>` everywhere outside `main`. `main` returns `anyhow::Result<()>`.
- All public functions get a one-line `///` doc comment. No further commentary unless the *why* is non-obvious.

---

## Task 1: Bootstrap Cargo project

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `.gitignore`

**Step 1: Install Rust toolchain**

Run: `min add rust`
Expected: prints `Installed rust` (or similar). Verify with `cargo --version`.

**Step 2: Initialize the crate**

Run: `cargo init --name fastrs --bin .`
Expected: creates `Cargo.toml`, `src/main.rs`, and `.gitignore`. The CLAUDE.md / minimal.toml / docs/ already in place are untouched.

**Step 3: Replace `Cargo.toml` contents**

Write `Cargo.toml`:

```toml
[package]
name = "fastrs"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fastrs"
path = "src/main.rs"

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream", "json"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
anyhow = "1"
futures = "0.3"

[dev-dependencies]
wiremock = "0.6"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync", "test-util"] }
```

**Step 4: Stub `src/main.rs`**

```rust
fn main() -> anyhow::Result<()> {
    Ok(())
}
```

**Step 5: Verify it builds**

Run: `cargo check`
Expected: `Finished ...` with no errors. (First run will fetch the registry — may take a minute.)

**Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "Bootstrap fastrs Cargo project"
```

---

## Task 2: CLI argument parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Write the failing test**

In `src/cli.rs`:

```rust
use clap::Parser;

/// Measure internet speed against fast.com.
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Args {
    /// Emit a single JSON object instead of human-friendly output.
    #[arg(long)]
    pub json: bool,

    /// Skip the upload phase.
    #[arg(long = "no-upload")]
    pub no_upload: bool,

    /// Print one human-readable line, no live updates.
    #[arg(long = "single-line")]
    pub single_line: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_false() {
        let args = Args::try_parse_from(["fastrs"]).unwrap();
        assert_eq!(args, Args { json: false, no_upload: false, single_line: false });
    }

    #[test]
    fn parses_all_flags() {
        let args = Args::try_parse_from(["fastrs", "--json", "--no-upload", "--single-line"]).unwrap();
        assert!(args.json && args.no_upload && args.single_line);
    }
}
```

In `src/main.rs`:

```rust
mod cli;

fn main() -> anyhow::Result<()> {
    let _args = <cli::Args as clap::Parser>::parse();
    Ok(())
}
```

**Step 2: Run the tests**

Run: `cargo test --lib cli`
Expected: 2 tests pass.

**Step 3: Smoke-test the binary**

Run: `cargo run -- --help`
Expected: clap-rendered help text listing `--json`, `--no-upload`, `--single-line`.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add CLI argument parsing"
```

---

## Task 3: Token extraction from fast.com JS bundle

The fast.com homepage embeds a `<script src="/app-XXXXX.js">`. That JS file contains a hardcoded token used to authenticate against `api.fast.com`. We need to scrape both URLs.

**Files:**
- Create: `src/api.rs`
- Modify: `src/main.rs` (add `mod api;`)

**Step 1: Write the failing parser tests** (pure-string parsing, no network yet)

```rust
use anyhow::{Context, Result};
use regex::Regex;

/// Extract the path of the application JS bundle from the fast.com homepage HTML.
pub fn parse_app_js_path(html: &str) -> Result<String> {
    let re = Regex::new(r#"src="(/app-[A-Za-z0-9]+\.js)""#).unwrap();
    let caps = re.captures(html).context("no app-*.js script tag found in fast.com HTML")?;
    Ok(caps[1].to_string())
}

/// Extract the API token from the fast.com app JS bundle.
pub fn parse_token(js: &str) -> Result<String> {
    let re = Regex::new(r#"token:"([A-Za-z0-9]+)""#).unwrap();
    let caps = re.captures(js).context("no token literal found in fast.com JS bundle")?;
    Ok(caps[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_app_js_path() {
        let html = r#"<html><body><script src="/app-abc123.js"></script></body></html>"#;
        assert_eq!(parse_app_js_path(html).unwrap(), "/app-abc123.js");
    }

    #[test]
    fn extracts_app_js_path_returns_error_when_missing() {
        assert!(parse_app_js_path("<html></html>").is_err());
    }

    #[test]
    fn extracts_token() {
        let js = r#"function n(){return{urlCount:5,token:"YXNkZmFzZGZhc2RmYXNkZg",https:!0}}"#;
        assert_eq!(parse_token(js).unwrap(), "YXNkZmFzZGZhc2RmYXNkZg");
    }

    #[test]
    fn extracts_token_returns_error_when_missing() {
        assert!(parse_token("var x = 1;").is_err());
    }
}
```

In `src/main.rs`, add `mod api;` next to `mod cli;`.

**Step 2: Run the tests**

Run: `cargo test --lib api`
Expected: 4 tests pass.

**Step 3: Add the network fetch wrapper**

Append to `src/api.rs`:

```rust
const FASTCOM_HOMEPAGE: &str = "https://fast.com";

/// Scrape fast.com homepage + JS bundle, return the API token.
pub async fn fetch_token(client: &reqwest::Client, base_url: &str) -> Result<String> {
    let html = client.get(base_url).send().await?.error_for_status()?.text().await?;
    let js_path = parse_app_js_path(&html)?;
    let js_url = format!("{base_url}{js_path}");
    let js = client.get(&js_url).send().await?.error_for_status()?.text().await?;
    parse_token(&js)
}

/// Convenience wrapper that fetches the token using the real fast.com URL.
pub async fn fetch_token_default(client: &reqwest::Client) -> Result<String> {
    fetch_token(client, FASTCOM_HOMEPAGE).await
}
```

**Step 4: Write the wiremock test**

Append to `src/api.rs`'s test module:

```rust
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_token_walks_homepage_then_js_bundle() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(r#"<script src="/app-xyz.js"></script>"#))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/app-xyz.js"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(r#"...token:"DEADBEEF",..."#))
            .mount(&server).await;

        let client = reqwest::Client::new();
        let token = fetch_token(&client, &server.uri()).await.unwrap();
        assert_eq!(token, "DEADBEEF");
    }
```

**Step 5: Run all `api` tests**

Run: `cargo test --lib api`
Expected: 5 tests pass.

**Step 6: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add fast.com token discovery"
```

---

## Task 4: Target discovery

Calling `https://api.fast.com/netflix/speedtest/v2?https=true&token=...&urlCount=5` returns a JSON document with `client` info and a list of `targets` (CDN URLs to hit).

**Files:**
- Modify: `src/api.rs`

**Step 1: Add types and parser**

Append to `src/api.rs` (before the test module):

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub city: String,
    pub country: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub ip: String,
    pub asn: String,
    pub isp: String,
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub name: String,
    pub url: String,
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Targets {
    pub client: Client,
    pub targets: Vec<Target>,
}

const TARGETS_API: &str = "https://api.fast.com";

/// Fetch CDN targets and client metadata from the fast.com API.
pub async fn fetch_targets(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    url_count: u32,
) -> Result<Targets> {
    let url = format!(
        "{base_url}/netflix/speedtest/v2?https=true&token={token}&urlCount={url_count}"
    );
    let resp = client.get(&url).send().await?.error_for_status()?;
    Ok(resp.json::<Targets>().await?)
}

/// Convenience wrapper using the production API base URL.
pub async fn fetch_targets_default(
    client: &reqwest::Client,
    token: &str,
    url_count: u32,
) -> Result<Targets> {
    fetch_targets(client, TARGETS_API, token, url_count).await
}
```

**Step 2: Add the failing test**

Append to the test module in `src/api.rs`:

```rust
    #[tokio::test]
    async fn fetch_targets_parses_response() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "client": {
                "ip": "203.0.113.7",
                "asn": "AS15169",
                "isp": "TestNet",
                "location": {"city": "Dublin", "country": "Ireland"}
            },
            "targets": [
                {
                    "name": "ipv4-c001-dub001-ix.1.oca.nflxvideo.net",
                    "url": "https://ipv4-c001-dub001-ix.1.oca.nflxvideo.net/speedtest?c=ie&n=15169&v=4&e=1",
                    "location": {"city": "Dublin", "country": "Ireland"}
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/netflix/speedtest/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server).await;

        let client = reqwest::Client::new();
        let targets = fetch_targets(&client, &server.uri(), "TOKEN", 5).await.unwrap();
        assert_eq!(targets.client.ip, "203.0.113.7");
        assert_eq!(targets.targets.len(), 1);
        assert!(targets.targets[0].url.contains("nflxvideo.net"));
    }
```

**Step 3: Run tests**

Run: `cargo test --lib api`
Expected: 6 tests pass.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add fast.com target discovery"
```

---

## Task 5: Pure helpers — speed math + stability check

These power the measurement loops. Pure functions, easy to TDD.

**Files:**
- Create: `src/measure/mod.rs`, `src/measure/speed.rs`
- Modify: `src/main.rs` (add `mod measure;`)

**Step 1: Add module skeleton**

`src/measure/mod.rs`:

```rust
pub mod speed;
```

In `src/main.rs`, add `mod measure;`.

**Step 2: Write failing tests**

`src/measure/speed.rs`:

```rust
use std::time::Duration;

/// Convert (bytes, elapsed) to megabits per second.
pub fn bytes_to_mbps(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 { return 0.0; }
    (bytes as f64 * 8.0) / 1_000_000.0 / secs
}

/// Return true when the most recent `window` samples vary by less than `tolerance` (relative).
pub fn is_stable(samples: &[f64], window: usize, tolerance: f64) -> bool {
    if samples.len() < window { return false; }
    let recent = &samples[samples.len() - window..];
    let avg = recent.iter().sum::<f64>() / recent.len() as f64;
    if avg <= 0.0 { return false; }
    let max_dev = recent.iter().map(|x| (x - avg).abs()).fold(0.0_f64, f64::max);
    (max_dev / avg) < tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_megabyte_per_second_is_eight_mbps() {
        assert!((bytes_to_mbps(1_000_000, Duration::from_secs(1)) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn zero_elapsed_is_zero_mbps() {
        assert_eq!(bytes_to_mbps(1_000_000, Duration::from_secs(0)), 0.0);
    }

    #[test]
    fn stable_sequence_is_stable() {
        let samples = vec![100.0, 99.5, 100.2, 99.8, 100.1];
        assert!(is_stable(&samples, 5, 0.05));
    }

    #[test]
    fn rising_sequence_is_not_stable() {
        let samples = vec![10.0, 20.0, 50.0, 80.0, 100.0];
        assert!(!is_stable(&samples, 5, 0.05));
    }

    #[test]
    fn fewer_than_window_is_not_stable() {
        assert!(!is_stable(&[100.0, 100.0], 5, 0.05));
    }
}
```

**Step 3: Run tests**

Run: `cargo test --lib measure::speed`
Expected: 5 tests pass.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add speed math + stability helpers"
```

---

## Task 6: Latency measurement

Sequential GETs against one target, measure wall-clock for each, report min and average.

**Files:**
- Create: `src/measure/latency.rs`
- Modify: `src/measure/mod.rs`

**Step 1: Stub module**

In `src/measure/mod.rs`:

```rust
pub mod latency;
pub mod speed;
```

**Step 2: Implement (no unit test — pure I/O orchestration; integration-tested later)**

`src/measure/latency.rs`:

```rust
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
```

**Step 3: Run tests**

Run: `cargo test --lib measure::latency`
Expected: 1 test passes; `cargo build` succeeds.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add latency probe + summary"
```

---

## Task 7: Download measurement

Fan out 8 concurrent streamed GETs, sample cumulative bytes every 200 ms, stop when stable or 15 s elapsed.

**Files:**
- Create: `src/measure/download.rs`
- Modify: `src/measure/mod.rs`

**Step 1: Add to module list**

`src/measure/mod.rs`:

```rust
pub mod download;
pub mod latency;
pub mod speed;
```

**Step 2: Implement**

`src/measure/download.rs`:

```rust
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
    if n == 0 { return 0.0; }
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
            if shutdown.load(Ordering::Relaxed) { return Ok(()); }
            match chunk {
                Ok(bytes) => { counter.fetch_add(bytes.len() as u64, Ordering::Relaxed); }
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
```

**Step 3: Run tests**

Run: `cargo test --lib measure::download`
Expected: 2 tests pass; build succeeds.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add concurrent download measurement"
```

---

## Task 8: Upload measurement

Mirror of download, but POSTing random bytes from a bounded buffer.

**Files:**
- Create: `src/measure/upload.rs`
- Modify: `src/measure/mod.rs`

**Step 1: Add to module list**

```rust
pub mod download;
pub mod latency;
pub mod speed;
pub mod upload;
```

**Step 2: Implement**

`src/measure/upload.rs`:

```rust
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
```

**Step 3: Build**

Run: `cargo build`
Expected: succeeds.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add concurrent upload measurement"
```

---

## Task 9: Orchestrator + Report type

Run all phases, return a `Report`. Keep the loaded-latency phase by re-running probes during the last second of download.

**Files:**
- Modify: `src/measure/mod.rs`

**Step 1: Add the orchestrator**

`src/measure/mod.rs`:

```rust
pub mod download;
pub mod latency;
pub mod speed;
pub mod upload;

use crate::api::Targets;
use anyhow::Result;
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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

/// Run the full measurement against the given targets.
pub async fn run(
    client: &reqwest::Client,
    targets: &Targets,
    opts: &Options,
) -> Result<Report> {
    let urls: Vec<String> = targets.targets.iter().map(|t| t.url.clone()).collect();
    let first = urls.first().expect("targets is non-empty").clone();

    // 1. Unloaded latency
    let unloaded = latency::probe(client, &first, 10).await?;
    let (unloaded_min, _) = latency::summarize_ms(&unloaded);

    // 2. Download
    let dl_shutdown = Arc::new(AtomicBool::new(false));
    let download_handle = {
        let client = client.clone();
        let urls = urls.clone();
        let shutdown = dl_shutdown.clone();
        tokio::spawn(async move { download::measure(&client, &urls, shutdown).await })
    };

    // Loaded-latency probes after a short ramp-up.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let loaded = latency::probe(client, &first, 5).await.unwrap_or_default();
    let (loaded_min, _) = if loaded.is_empty() {
        (unloaded_min, unloaded_min)
    } else {
        latency::summarize_ms(&loaded)
    };

    let download_mbps = download_handle.await??;

    // 3. Upload
    let upload_mbps = if opts.no_upload {
        None
    } else {
        let ul_shutdown = Arc::new(AtomicBool::new(false));
        Some(upload::measure(client, &urls, ul_shutdown).await?)
    };

    let server_locations: Vec<String> = targets.targets.iter()
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
```

**Step 2: Build**

Run: `cargo build`
Expected: succeeds.

**Step 3: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add measurement orchestrator + Report type"
```

---

## Task 10: Output renderers

Three renderers: `JsonRenderer` (final JSON), `SingleLineRenderer` (one line of human text), `LiveRenderer` (in-place updates on stderr).

For TDD, focus on JSON and SingleLine — Live is opaque and proven by manual smoke.

**Files:**
- Create: `src/output.rs`
- Modify: `src/main.rs` (add `mod output;`)

**Step 1: Implement + tests**

`src/output.rs`:

```rust
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
        report.server_locations.first().map(|s| s.as_str()).unwrap_or("unknown"),
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
    println!("Client:          {} / {}", report.client_ip, report.client_isp);
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
```

In `src/main.rs`, add `mod output;`.

**Step 2: Run tests**

Run: `cargo test --lib output`
Expected: 3 tests pass.

**Step 3: Commit**

```bash
cargo fmt
git add -A
git commit -m "Add output renderers"
```

---

## Task 11: Wire it all up in main

**Files:**
- Modify: `src/main.rs`

**Step 1: Final main**

```rust
mod api;
mod cli;
mod measure;
mod output;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    let client = reqwest::Client::builder()
        .user_agent("fastrs/0.1")
        .build()?;

    if !args.json && !args.single_line {
        eprintln!("Connecting to fast.com...");
    }

    let token = api::fetch_token_default(&client).await?;
    let targets = api::fetch_targets_default(&client, &token, 5).await?;

    let report = measure::run(
        &client,
        &targets,
        &measure::Options { no_upload: args.no_upload },
    ).await?;

    if args.json {
        output::render_json(&report)?;
    } else if args.single_line {
        println!("{}", output::render_single_line(&report));
    } else {
        output::render_summary(&report);
    }
    Ok(())
}
```

**Step 2: Lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. Fix anything that comes up.

**Step 3: Build release**

Run: `cargo build --release`
Expected: produces `target/release/fastrs`.

**Step 4: Commit**

```bash
cargo fmt
git add -A
git commit -m "Wire main: token → targets → measure → render"
```

---

## Task 12: Integration test + README

**Files:**
- Create: `tests/end_to_end.rs`, `README.md`

**Step 1: Write `#[ignore]`-gated end-to-end test**

`tests/end_to_end.rs`:

```rust
//! Hits real fast.com. Run with `cargo test -- --ignored`.

#[tokio::test]
#[ignore]
async fn full_run_against_real_fastcom() {
    let client = reqwest::Client::builder().user_agent("fastrs/0.1").build().unwrap();
    let token = fastrs::api::fetch_token_default(&client).await.unwrap();
    assert!(!token.is_empty());
    let targets = fastrs::api::fetch_targets_default(&client, &token, 3).await.unwrap();
    assert!(!targets.targets.is_empty());

    let report = fastrs::measure::run(
        &client,
        &targets,
        &fastrs::measure::Options { no_upload: true },
    ).await.unwrap();
    assert!(report.download_mbps > 0.1);
}
```

This requires the binary crate to also expose a library. Add to `Cargo.toml`:

```toml
[lib]
name = "fastrs"
path = "src/lib.rs"
```

Create `src/lib.rs`:

```rust
pub mod api;
pub mod cli;
pub mod measure;
pub mod output;
```

And update `src/main.rs` to import from the library:

```rust
use anyhow::Result;
use clap::Parser;
use fastrs::{api, cli, measure, output};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    let client = reqwest::Client::builder().user_agent("fastrs/0.1").build()?;

    if !args.json && !args.single_line {
        eprintln!("Connecting to fast.com...");
    }

    let token = api::fetch_token_default(&client).await?;
    let targets = api::fetch_targets_default(&client, &token, 5).await?;

    let report = measure::run(
        &client,
        &targets,
        &measure::Options { no_upload: args.no_upload },
    ).await?;

    if args.json {
        output::render_json(&report)?;
    } else if args.single_line {
        println!("{}", output::render_single_line(&report));
    } else {
        output::render_summary(&report);
    }
    Ok(())
}
```

Remove the `mod ...;` declarations from `main.rs` (they now live in `lib.rs`).

**Step 2: Verify everything still compiles and unit tests pass**

Run: `cargo test --lib`
Expected: all unit tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

**Step 3: Run the integration test against real fast.com**

Run: `cargo test --test end_to_end -- --ignored --nocapture`
Expected: passes (download > 0.1 Mbps). May take 20–30 s. If your sandbox blocks outbound HTTPS, document that and skip.

**Step 4: Write README**

`README.md`:

```markdown
# fastrs

Pure-Rust speed test against Netflix's fast.com.

## Install

    cargo install --path .

## Usage

    fastrs                   # human-friendly, live updates
    fastrs --single-line     # one-line summary
    fastrs --no-upload       # skip the upload phase
    fastrs --json            # machine-readable output

## Output (`--json`)

    {
      "download_mbps": 487.2,
      "upload_mbps": 56.4,
      "unloaded_latency_ms": 11,
      "loaded_latency_ms": 38,
      "server_locations": ["Dublin, Ireland"],
      "client_ip": "203.0.113.7",
      "client_isp": "TestNet"
    }
```

**Step 5: Final commit**

```bash
cargo fmt
git add -A
git commit -m "Add integration test + README"
```

---

## Definition of done

- `cargo test --lib` — green.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo run --release -- --json` against real fast.com prints a populated JSON object.
- `cargo run --release -- --single-line` prints a human-readable line.
- README explains install + usage.
