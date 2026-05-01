# fastrs TUI mode — Design

Add an interactive terminal UI with live throughput chart and latency sparkline, modeled after [`cloudflare-speed-cli`](https://github.com/kavehtehrani/cloudflare-speed-cli). Opt-in via `--tui`.

## Goals

- Live updating throughput chart (download + upload, color-coded) while measurement runs.
- Latency sparkline with unloaded / loaded readouts.
- Stats panel: current/peak Mbps, server, client.
- After measurement, TUI stays up showing final values; `q` quits.
- No regression for existing modes (`--json`, `--single-line`, default summary).

## Non-goals

- Tabbed views, history persistence, re-run keybinding (kept for a later iteration).
- Auto-detected TUI when stdout is a TTY. Always opt-in.

## CLI

```
fastrs --tui [--no-upload]
```

`--tui` conflicts with `--json` and `--single-line` at parse time.

## Architecture

The orchestrator currently returns only a final `Report`. To drive a chart, samples must flow out *while* `run` works. Pattern: a `tokio::sync::mpsc` channel.

```rust
pub enum Phase { UnloadedLatency, Download, LoadedLatency, Upload }

pub enum Progress {
    PhaseStart(Phase),
    Throughput { mbps: f64 },
    Latency { ms: f64 },
    PhaseEnd(Phase),
}
```

`measure::run` grows a parallel `run_with_progress(..., tx: mpsc::Sender<Progress>)` entry point. The existing `run` keeps its current signature and delegates to `run_with_progress` with a discarded sender — so existing callers (`--json` / `--single-line` / default summary) need no changes.

Internally, the download/upload sampling loops already emit a sample every 200 ms. They get an optional `&mpsc::Sender<Progress>` parameter and emit `Throughput { mbps }` per tick. The latency probe emits `Latency { ms }` per round-trip.

The TUI subscribes to the receiver, updates its `App` state on each event, and re-renders. When the orchestrator returns, the channel closes; the TUI flips to "press q" mode showing the final `Report`.

## UI layout

Three vertical regions in a fixed single screen:

```
┌─ fastrs ── Phase: Download (8s / 15s) ──────────────────┐
│                                                         │
│   Throughput (Mbps)                                     │
│   500 ┤        ╭──────────────                          │
│   400 ┤       ╱                  ─── download           │
│   300 ┤      ╱                   ─── upload             │
│   200 ┤    ╱                                            │
│   100 ┤  ╱                                              │
│     0 ┴──────────────────────────────────────────       │
│       0s    5s    10s   15s   20s   25s   30s           │
│                                                         │
├─ Latency ───────────────────────────────────────────────┤
│   ▁▂▃▂▁▁▂▆█▇▆▇█▇█▇▆▆█  (unloaded 12 ms / loaded 38 ms)  │
├─ Stats ─────────────────────────────────────────────────┤
│  ↓ 487.2 Mbps    ↑ 56.4 Mbps    Server: Dublin, IE      │
│  Peak ↓ 502.1    Peak ↑ 64.8    Client: 203.0.113.7 ... │
│                                                         │
│  press q to quit                                        │
└─────────────────────────────────────────────────────────┘
```

- **Top (~70 % height)** — ratatui `Chart` widget. X axis = elapsed seconds; Y axis = Mbps with auto-scale (max of all samples × 1.1). Two datasets, cyan for download, magenta for upload. Both share the chart so the upload phase visibly picks up after download finishes.
- **Middle (~3 lines)** — ratatui `Sparkline` of recent latency probes. After each phase ends, the readout (`unloaded 12 ms / loaded 38 ms`) is filled in.
- **Bottom (~5 lines)** — stats panel: current Mbps, peak Mbps, server location, client IP/ISP. Footer: `press q to quit` (or `done — press q to quit` once the measurement returns).

Title bar shows active phase + elapsed time / max time for that phase.

## Crate strategy

Put the TUI behind an opt-in cargo feature `tui`, on by default. Users who want a tiny binary can `cargo install --no-default-features`. CI builds default features.

```toml
[features]
default = ["tui"]
tui = ["dep:ratatui", "dep:crossterm"]

[dependencies]
ratatui = { version = "0.30", optional = true }
crossterm = { version = "0.30", optional = true }
```

Approximate binary growth: ~600 KB.

## Module layout

```
src/
  cli.rs            # add `tui: bool` flag (with conflicts_with = ["json", "single_line"])
  measure/
    mod.rs          # add Progress enum + Phase enum + run_with_progress entry
    download.rs     # accept Option<&mpsc::Sender<Progress>>, emit Throughput
    upload.rs       # same
    latency.rs      # emit Latency
  tui.rs            # NEW, gated on #[cfg(feature = "tui")]
  main.rs           # if args.tui { tui::run(...).await } else { existing path }
```

## `tui::run` skeleton

```rust
#[cfg(feature = "tui")]
pub async fn run(
    client: &reqwest::Client,
    targets: &Targets,
    opts: &Options,
) -> Result<Report> {
    let (tx, mut rx) = mpsc::channel(64);
    let mut terminal = init_terminal()?;
    let mut app = App::new(targets);
    let measurement = tokio::spawn({
        let client = client.clone();
        let targets = targets.clone();
        let opts = opts.clone();
        async move { measure::run_with_progress(&client, &targets, &opts, tx).await }
    });

    loop {
        tokio::select! {
            evt = read_terminal_event() => if app.handle_event(evt) { break },
            sample = rx.recv() => match sample {
                Some(p) => app.apply(p),
                None => { app.measurement_done(); break_to_done_state },
            },
        }
        terminal.draw(|f| app.render(f))?;
    }

    // After measurement: wait for q.
    while !app.quit { ... }
    restore_terminal()?;
    measurement.await?
}
```

## Testing

- **Unit**: `App::apply(Progress)` is a pure state transition. Cover phase advance, peak update, sparkline buffer rotation.
- **Manual**: render path. No automated TUI rendering tests — too brittle.

## Build / packaging

- Default cargo build keeps the TUI on; release binaries (linux/mac/windows) all ship with the chart.
- `--no-default-features` builds a slim binary without ratatui/crossterm.
- README gets a "TUI mode" section + screenshot.
