# fastrs

Pure-Rust speed test against Netflix's [fast.com](https://fast.com).

A single static binary that talks to fast.com's HTTP API directly. Inspired by [`fastcom-speed-cli`](https://pypi.org/project/fastcom-speed-cli/) and [`fast-cli`](https://github.com/sindresorhus/fast-cli).

## Install

### From source

    cargo install --path .

### Pre-built binaries

Grab the latest release for your platform from the [Releases](../../releases) page.

## Usage

    fastrs                   # human-friendly multi-line summary
    fastrs --single-line     # one-line summary
    fastrs --no-upload       # skip the upload phase
    fastrs --json            # machine-readable output

### Example

    $ fastrs
    Connecting to fast.com...
    Download:         2879.3 Mbps
    Upload:           3102.1 Mbps
    Latency unloaded:     81 ms
    Latency loaded:      435 ms
    Client:          203.0.113.7 / Telus
    Server:          Vancouver, CA

The gap between unloaded and loaded latency is *bufferbloat* — how much the connection's queuing delay rises under load.

### JSON output

    $ fastrs --json
    {
      "download_mbps": 487.2,
      "upload_mbps": 56.4,
      "unloaded_latency_ms": 11,
      "loaded_latency_ms": 38,
      "server_locations": ["Dublin, Ireland"],
      "client_ip": "203.0.113.7",
      "client_isp": "TestNet"
    }

## How it works

1. Scrape the API token from fast.com's JS bundle.
2. Hit `https://api.fast.com/netflix/speedtest/v2` for a list of Netflix CDN target URLs.
3. Probe unloaded latency with sequential GETs against the first target.
4. Open 8 concurrent streamed GETs across the targets, sample bytes every 200 ms, stop when throughput stabilizes or 15 s elapses.
5. Re-probe latency mid-download to compute loaded latency / bufferbloat.
6. Repeat with concurrent POSTs for the upload phase.

## Building

    cargo build --release

The release binary lands at `target/release/fastrs`.

## Development

    cargo test                                 # unit tests
    cargo test -- --ignored                    # also run the live integration test against fast.com
    cargo clippy --all-targets -- -D warnings  # lint
    cargo fmt                                  # format

## License

Apache-2.0. See [LICENSE](LICENSE).
