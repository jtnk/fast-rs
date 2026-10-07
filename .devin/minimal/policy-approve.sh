#!/bin/sh
# PermissionRequest(^exec$) — box-policy-aware auto-approval.
#
# The box's own egress declaration is the org's reachability authority:
# if an armed box exists and the command's first network target is already
# inside its declared allow-list, asking the human again buys nothing —
# approve. Anything else abstains (no output, exit 0) and Devin's normal
# permission flow decides. This is the client-side slice of gominimal/arch
# open gap 7 (out-of-band approval): Devin's prompt remains the supervisor
# for everything the declaration does not already answer.
set -u

payload="$(cat)"

# ---- only when an armed box exists ---------------------------------------
ROOT="${DEVIN_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)}"
STATE="$ROOT/.devin/minimal-box.json"
[ -f "$STATE" ] || exit 0
grep -q '"armed"[ ]*:[ ]*true' "$STATE" || exit 0
SID=$(sed -n 's/.*"session_id"[ ]*:[ ]*"\([^"]*\)".*/\1/p' "$STATE" | head -1)
[ -n "$SID" ] || exit 0
command -v min >/dev/null 2>&1 || exit 0

# ---- extract the command --------------------------------------------------
if command -v python3 >/dev/null 2>&1; then
    CMD=$(printf '%s' "$payload" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("tool_input") or {}).get("command",""))' 2>/dev/null)
else
    CMD=$(printf '%s' "$payload" | sed -n 's/.*"command"[ ]*:[ ]*"\(.*\)"[ ,}].*/\1/p' | head -1)
fi
[ -n "$CMD" ] || exit 0

# ---- only network-obvious commands ---------------------------------------
case "$CMD" in
    curl*|wget*|git\ clone*|git\ fetch*|git\ pull*|git\ push*|npm*|pnpm*|yarn*|pip*|pip3*|cargo*|go\ get*|go\ mod*|apt*|apk*|brew*|ssh*|scp*|rsync*|nc\ *) ;;
    *) exit 0 ;;
esac

# first hostname-looking token or URL authority in the command
HOST=$(printf '%s' "$CMD" \
    | grep -oE 'https?://[A-Za-z0-9.-]+' | head -1 | sed 's|https\?://||')
[ -n "$HOST" ] || HOST=$(printf '%s' "$CMD" \
    | grep -oE '([A-Za-z0-9-]+\.)+[A-Za-z]{2,}' | head -1)
[ -n "$HOST" ] || exit 0

# ---- consult the box's own declared policy --------------------------------
POLICY=$(min session policy "$SID" -o json 2>/dev/null) || exit 0
[ -n "$POLICY" ] || exit 0

# allow-all egress already answers for every host
case "$POLICY" in
    *'"effective":"allow-all"'*|*'"effective" *: *"allow-all"'*)
        printf '{"decision":"approve","reason":"the armed Minimal box'"'"'s declared egress is allow-all — the box boundary already answers this."}\n'
        exit 0 ;;
esac

# policy text/json → candidate hostnames; suffix-match ("api.github.com"
# is covered by a declared "github.com" wildcard only when the declaration
# itself says so — exact match or declared "*.suffix" only).
COVERED=
while IFS= read -r h; do
    case "$h" in
        "$HOST") COVERED=1 ;;
        \*.*)    case "$HOST" in *"${h#\*}") COVERED=1 ;; esac ;;
    esac
done <<EOF
$(printf '%s' "$POLICY" | grep -oE '([A-Za-z0-9*-]+\.)+[A-Za-z*]{2,}')
EOF

if [ "$COVERED" = "1" ]; then
    printf '{"decision":"approve","reason":"%s is inside the armed Minimal box'"'"'s declared egress policy — the box boundary already answers this."}\n' "$HOST"
fi
exit 0
