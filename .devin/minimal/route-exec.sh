#!/bin/sh
# PreToolUse(^exec$) — transparently route Devin's shell commands into the
# armed Minimal box by rewriting tool_input.command.
#
#   in : {"tool_name":"exec","tool_input":{"command":"make test", …}}
#   out: {"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":
#         {"command":"sh '<plugin>/scripts/minbox' exec 'make test'"}}}
#
# Silent exit 0 (no output) = leave the command untouched. Routing only
# engages once .devin/minimal-box.json says a box is armed; everything
# Minimal's own machinery needs (`min`, the min:// git helper, `host:`
# escapes, interactive shells) stays on the host.
set -u

# minbox lives next to this script — in the plugin's scripts/ dir when the
# plugin is installed, or in a vendored <repo>/.devin/minimal/ kit. The hook
# command is always `sh <path>/route-exec.sh`, so $0's directory is the
# reliable anchor (DEVIN_PLUGIN_ROOT only exists for plugin hooks anyway).
SELF_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
MINBOX="${MINBOX:-$SELF_DIR/minbox}"

payload="$(cat)"

# ---- extract tool_input.command (+ whether this is an interactive shell) --
# python3 first (exact JSON); sed fallback for the common single-line case.
if command -v python3 >/dev/null 2>&1; then
    eval "$(printf '%s' "$payload" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    i = d.get("tool_input", {}) or {}
    cmd = i.get("command", "")
    inter = bool(i.get("shell_id") or i.get("tty"))
except Exception:
    cmd, inter = "", False
# sh-quote for safe eval: single-quote, escape embedded quotes
print("CMD=" + "\x27" + cmd.replace("\x27", "\x27\\\x27\x27") + "\x27")
print("INTER=" + ("1" if inter else "0"))
' 2>/dev/null)"
else
    # crude but safe-enough extraction for "command": "…" on one line;
    # bail (leave unrouted) when the value has embedded escapes we cannot
    # reproduce faithfully.
    CMD=$(printf '%s' "$payload" | sed -n 's/.*"command"[ ]*:[ ]*"\(.*\)"[ ,}].*/\1/p' | head -1)
    case "$CMD" in *\\*) CMD="";; esac
    INTER=0
fi

[ -n "${CMD:-}" ] || exit 0
[ "${INTER:-0}" = "1" ] && exit 0   # persistent/interactive shells keep their PTY

# ---- pass-through rules ---------------------------------------------------
case "$CMD" in
    min\ *|min|mip\ *|mip|minbox*|devin\ *|devin|osascript*|open\ *) exit 0 ;;
    host:*)
        # explicit host escape: strip the prefix, run locally
        stripped=$(printf '%s' "$CMD" | sed 's/^host://')
        esc=$(printf '%s' "$stripped" | sed 's/\\/\\\\/g; s/"/\\"/g')
        printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"%s"}}}\n' "$esc"
        exit 0 ;;
esac
case "$CMD" in *min://*) exit 0 ;; esac     # git push/fetch min://<session>
[ "${MINBOX_OFF:-}" = "1" ] && exit 0

# ---- armed? ---------------------------------------------------------------
ROOT="${DEVIN_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)}"
STATE="$ROOT/.devin/minimal-box.json"
[ -f "$STATE" ] || exit 0
grep -q '"armed"[ ]*:[ ]*true' "$STATE" || exit 0

# ---- rewrite --------------------------------------------------------------
# The rewritten command runs under the host shell; single-quote the payload
# so it reaches minbox exec byte-for-byte.
sq=$(printf '%s' "$CMD" | sed "s/'/'\\\\''/g")
# Quote the minbox path too — repo dirs can contain spaces.
mbsq=$(printf '%s' "$MINBOX" | sed "s/'/'\\\\''/g")
esc=$(printf '%s' "sh '$mbsq' exec '$sq'" \
    | sed 's/\\/\\\\/g; s/"/\\"/g' | tr '\n' ' ')
printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"%s"}}}\n' "$esc"
exit 0
