#!/usr/bin/env bash
# Banner pane for `min run dev`. Prints the project label + task shortcuts,
# then sleeps to keep the pane alive (the interactive shell lives in the
# sibling pane below).

if [ -t 1 ]; then
  R=$'\e[0m'    ; D=$'\e[2m'    ; B=$'\e[1m'
  CY=$'\e[38;5;87m'              # cyan      — brand
  PK=$'\e[38;5;213m'             # pink      — section headers
  GY=$'\e[38;5;245m'             # grey      — chrome
  GR=$'\e[38;5;120m'             # green     — accent
  YL=$'\e[38;5;221m'             # yellow    — caution
fi

cat <<EOF

  ${CY}█▀▀ ▄▀█ █▀ ▀█▀ █▀█ █▀${R}    ${GR}⚡${R} ${D}pure-Rust fast.com speed test${R}
  ${CY}█▀  █▀█ ▄█  █  █▀▄ ▄█${R}    ${D}you're inside the${R} ${B}Minimal${R} ${D}sandbox${R}
  ${GY}────────────────────────────────────────────────────────${R}

  ${PK}▸ tasks${R}  ${D}(each spawns a fresh sub-sandbox)${R}
    ${B}min run build${R}            ${D}cargo build --release${R}
    ${B}min run run${R}              ${D}cargo run --release${R}
    ${B}min run fast-rs-debug${R}    ${D}cargo run --release w/ RUST_LOG=debug${R}
    ${B}min run test${R}             ${D}cargo test${R}
    ${B}min run test-live${R}        ${D}cargo test -- --ignored${R}  ${YL}(hits fast.com)${R}
    ${B}min run lint${R}             ${D}cargo clippy --all-targets -- -D warnings${R}
    ${B}min run fmt${R}              ${D}cargo fmt${R}
    ${B}min run webapp${R}           ${D}python3 webapp/server.py${R}

  ${PK}▸ sandbox${R}
    ${B}min add${R}  ${GY}<pkg>${R}       ${D}install a package for this session${R}
    ${B}min search${R} ${GY}<term>${R}    ${D}find a package by name${R}

  ${GY}quit zellij: ${R}${B}Ctrl-q${R}   ${GY}·   detach: ${R}${B}Ctrl-o${R} ${GY}then${R} ${B}d${R}

EOF

exec sleep infinity
