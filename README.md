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
