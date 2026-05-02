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

tmux -f tmux.conf new-session -d -s dev
tmux send-keys -t dev 'claude --dangerously-skip-permissions' Enter
tmux split-window -t dev -h 'bash'
tmux select-layout -t dev even-horizontal
tmux select-pane -t dev:0.0
tmux attach-session -t dev
