# fastrs-cli — Design

A Rust port of [`fastcom-speed-cli`](https://pypi.org/project/fastcom-speed-cli/), modeled after `fast-cli` for surface area. Pure-HTTP — no headless browser.

## Goals

- Single static binary, fast startup, minimal dependencies.
- Measure download speed, upload speed, unloaded latency, and loaded latency (bufferbloat) against Netflix's fast.com endpoints.
- Two output modes:
  - **Live** (default): in-place updating line on stderr, summary on stdout when done.
  - **JSON** (`--json`): single final JSON object on stdout.

## Non-goals

- Browser-faithful timing or reproducing the Python tool's `--min-duration`/`--max-duration`/`--interval` knob set.
- Time-series streaming output. Final results only.

## CLI

```
fastrs [--json] [--no-upload] [--single-line]
```

- `--json` — emit final JSON, suppress live UI.
- `--no-upload` — skip the upload phase.
- `--single-line` — print one human-readable line, no live updates (useful for cron/CI).

## Architecture

Three phases run sequentially:

1. **Token discovery** — `GET https://fast.com/`, parse out `app-*.js` URL, fetch it, regex-extract the hardcoded API token (`token:"[A-Za-z0-9]+"`).
2. **Target discovery** — `GET https://api.fast.com/netflix/speedtest/v2?https=true&token={t}&urlCount=5`. Response includes a list of CDN target URLs and `client` + `targets[].location` metadata.
3. **Measurement** —
   - **Unloaded latency**: N=10 sequential HTTPS GETs of a small range against the first target. Record min and average wall-clock time.
   - **Download**: 8 concurrent streamed GETs across the targets, sample cumulative bytes every 200 ms, ramp until either rate stabilizes (variance over the last 2 s under threshold) or the 15 s cap is reached.
   - **Upload** (skipped if `--no-upload`): 8 concurrent POSTs with random bytes from a bounded buffer, same sampling logic.
   - **Loaded latency** (bufferbloat): rerun latency probes during the last second of the download phase, before tearing connections down.

## Module layout

```
src/
  main.rs           # clap entrypoint, dispatch
  cli.rs            # Args struct
  api.rs            # token scrape + target discovery
  measure/
    mod.rs          # orchestrator: runs phases, returns Report
    latency.rs      # unloaded + loaded probes
    download.rs     # concurrent download sampling
    upload.rs       # concurrent upload sampling
  output.rs         # LiveRenderer, JsonRenderer, SingleLineRenderer
```

## Data shapes

```rust
struct Report {
    download_mbps: f64,
    upload_mbps: Option<f64>,
    unloaded_latency_ms: f64,
    loaded_latency_ms: f64,
    server_locations: Vec<String>,
    client_ip: String,
    client_isp: String,
}
```

JSON output is `Report` serialized verbatim (snake_case).

## Crate choices

| Crate | Use |
|---|---|
| `tokio` (rt-multi-thread, macros) | async runtime |
| `reqwest` (rustls-tls, stream) | HTTP client, streaming bodies |
| `clap` (derive) | CLI parsing |
| `serde` + `serde_json` | JSON I/O |
| `regex` | token extraction |
| `anyhow` | error propagation |
| `futures` | concurrent stream joining |

## Error handling

- All network errors bubble up as `anyhow::Error` with context.
- Token-scrape failure is fatal with a clear message ("fast.com layout changed?").
- A single target failing mid-download is non-fatal; we continue with the remaining streams. Total failure of all streams aborts.

## Testing

- **Unit**: mock HTTP with `wiremock` for `api.rs` (token regex, target parsing). Pure-function tests for the speed-computation helpers.
- **Integration**: `#[ignore]`-gated end-to-end test that hits real fast.com. Run manually / in CI on demand.

## Build / packaging

`Cargo.toml` declares the binary as `fastrs`. Project lives at the repo root (`Cargo.toml`, `src/`). `minimal.toml` already exists; the brief `min run build` / `min run test` tasks can be added.
