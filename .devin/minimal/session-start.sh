#!/bin/sh
# SessionStart — report the Minimal landscape for this project and inject
# it into the agent's context. Never blocks, never fails the session.
set -u

ROOT="${DEVIN_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)}"
STATE="$ROOT/.devin/minimal-box.json"

# Two installation shapes: plugin (scripts live in the plugin dir, skills
# /minimal:box-* exist) and vendored (this script sits in a repo's
# .devin/minimal/ kit — Devin Cloud sessions get routing this way). The
# steering text must name whichever entry point actually exists.
SELF_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
case "$SELF_DIR" in
    */.devin/minimal)
        VENDORED=1
        UP="sh .devin/minimal/minbox up"
        DOWN="sh .devin/minimal/minbox down" ;;
    *)
        VENDORED=0
        UP="/minimal:box-up"
        DOWN="/minimal:box-down" ;;
esac

if ! command -v min >/dev/null 2>&1; then
    if [ "$VENDORED" = 1 ]; then
        CTX="minimal kit: min is not on PATH. Bootstrap it once with 'sh .devin/minimal/cloud-bootstrap.sh' (installs Minimal, fixes userns/AppArmor, starts the daemon), then arm the box with '$UP'."
    else
        CTX="minimal plugin: min is not on PATH — box routing is dormant until you install Minimal and run $UP."
    fi
    printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$CTX"
    exit 0
fi

VER=$(min version 2>/dev/null | head -1 | sed 's/^Client: *//')
[ -n "$VER" ] || VER="unknown"

armed_session=""
if [ -f "$STATE" ] && grep -q '"armed"[ ]*:[ ]*true' "$STATE"; then
    armed_session=$(sed -n 's/.*"session"[ ]*:[ ]*"\([^"]*\)".*/\1/p' "$STATE" | head -1)
fi

LIVE=$(min session list --json 2>/dev/null | grep -c '"id"' || true)

if [ -n "$armed_session" ]; then
    if min session list --json 2>/dev/null | grep -q "$armed_session"; then
        CTX="MINIMAL BOX ARMED: session '$armed_session' is live. Every exec command is transparently rewritten to run inside the box at /workbench (changed files are pushed first). Your file tools still edit host files — the sync covers them. Prefix 'host:' to run on the host. '$DOWN' disarms."
    else
        CTX="MINIMAL BOX ARMED but DEAD: session '$armed_session' no longer exists. exec calls will be BLOCKED until you run '$UP' to recreate it or '$DOWN' to disarm. Do this first."
    fi
elif [ "$LIVE" -gt 0 ] 2>/dev/null; then
    CTX="minimal: min $VER present, $LIVE session(s) running but none armed for this project. '$UP' activates or adopts one and routes exec into it."
else
    CTX="minimal: min $VER present, no sessions. Box routing is dormant — '$UP' creates the project's box and arms it."
fi

# additionalContext is a JSON string; sessions/versions are DNS-safe already.
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$CTX"
exit 0
