#!/usr/bin/env bash
set -euo pipefail

# This script launches Claude with --dangerously-skip-permissions, which is
# only safe inside the ephemeral Minimal sandbox. Refuse to run on a host
# shell so an accidental invocation cannot lower safety.
if [ "${IS_SANDBOX:-}" != "1" ]; then
  echo "scripts/setup-dev.sh must be run inside the Minimal sandbox (IS_SANDBOX=1)." >&2
  echo "Use 'min run dev' instead." >&2
  exit 1
fi

# The sandbox terminfo db has no xterm-ghostty entry, so `clear` (and any
# other terminfo consumer) errors out. Override before zellij starts so
# child panes inherit a known TERM.
export TERM=xterm-256color

# Kill any prior `dev` session so each run starts from a clean layout.
zellij delete-session --force dev 2>/dev/null || true

zellij --config zellij-config.kdl --layout zellij.kdl attach --create dev

# Wipe zellij's "Bye from Zellij!" trailer.
clear
