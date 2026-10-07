#!/bin/sh
# cloud-bootstrap.sh — install and verify Minimal inside a headless Linux host
# (Devin Cloud VM, CI runner, fresh dev container). Idempotent, non-interactive,
# strict POSIX sh.
#
#   sh cloud-bootstrap.sh [--channel stable|unstable|nightly] [--check]
#
# Steps:
#   1. Platform gate: Linux, x86_64/aarch64, kernel >= 5.10
#   2. Installer: curl|sh from go.minimal.dev (hash-verified, atomic, idempotent)
#   3. userns remediation: Ubuntu >=24.04 AppArmor profile via sudo when needed
#   4. git insteadOf neutralization for Minimal upstream prefixes (see below)
#   5. Liveness: `min ls` autospawns minimald and proves the client→daemon path
#
# What it deliberately does NOT do: create sessions, touch the project, or
# install packages — that stays with `minbox up` so the box lifecycle is owned
# by one script on every platform.
#
# Env:
#   MINIMAL_CHANNEL   release channel (default: stable)
#   MINIMAL_NO_SUDO   if set, never attempt sudo remediation — fail instead

set -eu

CHANNEL="${MINIMAL_CHANNEL:-stable}"
CHECK_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --channel) CHANNEL="$2"; shift 2 ;;
        --channel=*) CHANNEL="${1#*=}"; shift ;;
        --check) CHECK_ONLY=1; shift ;;
        *) echo "cloud-bootstrap: unknown arg '$1'" >&2; exit 2 ;;
    esac
done

say()  { printf 'cloud-bootstrap: %s\n' "$*"; }
fail() { printf 'cloud-bootstrap: FAIL: %s\n' "$*" >&2; exit 1; }

# --- 1. platform gate -------------------------------------------------------
[ "$(uname -s)" = "Linux" ] || fail "this script is for Linux; on macOS use the normal installer"
case "$(uname -m)" in
    x86_64|aarch64) ;;
    *) fail "unsupported arch $(uname -m) (need x86_64 or aarch64)" ;;
esac
_kver=$(uname -r | cut -d. -f1-2)
_kmaj=${_kver%%.*}; _kmin=${_kver#*.}
if [ "$_kmaj" -lt 5 ] || { [ "$_kmaj" -eq 5 ] && [ "$_kmin" -lt 10 ]; }; then
    fail "kernel $_kver too old (Minimal needs >= 5.10)"
fi

BINDIR="$HOME/.local/bin"
PATH="$BINDIR:$PATH"; export PATH

# --- 2. userns gate ----------------------------------------------------------
# Minimal sessions are unprivileged user namespaces. Three ways a stock
# kernel can refuse them; check all so the error is specific.
userns_ok() {
    # Debian-style kill switch
    v=$(sysctl -n kernel.unprivileged_userns_clone 2>/dev/null || echo "")
    [ "$v" = "0" ] && return 1
    # Ubuntu 24.04+ AppArmor restriction: 1 means "must have a profile"
    v=$(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || echo "")
    if [ "$v" = "1" ]; then
        [ -e "$HOME/.local/share/minimal/apparmor/minimald" ] || return 2
    fi
    # Namespace quota of zero
    v=$(sysctl -n user.max_user_namespaces 2>/dev/null || echo "")
    [ "$v" = "0" ] && return 1
    return 0
}

remediate_apparmor() {
    _aa="$HOME/.local/share/minimal/apparmor/install-apparmor-profile.sh"
    [ -f "$_aa" ] || fail "AppArmor restricts unprivileged userns and no Minimal AppArmor profile was installed"
    [ -z "${MINIMAL_NO_SUDO:-}" ] || fail "AppArmor restricts unprivileged userns; sudo needed but MINIMAL_NO_SUDO is set"
    command -v sudo >/dev/null 2>&1 || fail "AppArmor restricts unprivileged userns and sudo is unavailable. Grant a sudoer, or set kernel.apparmor_restrict_unprivileged_userns=0"
    say "installing Minimal AppArmor profile (unprivileged userns permission for minimald)"
    sudo sh "$_aa" || fail "AppArmor profile install failed"
}

# --- 3. install --------------------------------------------------------------
need_install=1
if [ -x "$BINDIR/min" ]; then
    need_install=0
    say "min already installed: $(min --version 2>/dev/null | head -1)"
fi

# --- 3b. git insteadOf neutralization ---------------------------------------
# Managed hosts (Devin Cloud VMs, some CI images) rewrite github.com URLs to an
# authenticated proxy via url.<proxy>.insteadOf. `git remote get-url` applies
# that rewrite on read, so minimald's upstream-cache identity check compares
# the *proxy* URL against [upstream].repo and dies with
# `vcs: invalid remote path`. Neutralize it only for the prefixes Minimal
# caches: an identity insteadOf rule (base == match) wins longest-match, so
# Minimal sees raw URLs while every other repo keeps proxying.
neutralize_insteadof() {
    command -v git >/dev/null 2>&1 || return 0
    _urls="https://github.com/gominimal/"
    if [ -f minimal.toml ]; then
        _u=$(sed -n 's/^[[:space:]]*repo[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' \
            minimal.toml | head -1)
        case "$_u" in
            https://*/*/*|http://*/*/*) _urls="$_urls ${_u%/*}" ;;
        esac
    fi
    for _u in $_urls; do
        _p="${_u%/}/"
        # ls-remote --get-url expands insteadOf rules: if the prefix survives
        # unchanged there is no rewrite (or our identity rule is already in).
        _eff=$(git ls-remote --get-url "$_p" 2>/dev/null || echo "$_p")
        [ "$_eff" = "$_p" ] && continue
        git config --global "url.$_p.insteadof" "$_p" \
            && say "git insteadOf rewrite detected ($_p -> $_eff); identity exception added"
    done
}

if [ "$CHECK_ONLY" = 1 ]; then
    [ "$need_install" = 0 ] || fail "min not installed"
    _rc=0; userns_ok || _rc=$?
    case "$_rc" in
        1) fail "unprivileged user namespaces disabled" ;;
        2) fail "Ubuntu AppArmor userns restriction active; Minimal AppArmor profile missing" ;;
    esac
    neutralize_insteadof
    min ls >/dev/null 2>&1 || fail "min installed but daemon does not answer"
    say "check OK: $(min --version 2>/dev/null | head -1), daemon live"
    exit 0
fi

if [ "$need_install" = 1 ]; then
    command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 \
        || fail "need curl or wget for the installer"
    say "installing Minimal ($CHANNEL channel)"
    # The installer stops a running daemon on upgrade; force-stop makes that
    # non-interactive. Fresh installs ignore it.
    MINIMAL_INSTALL_FORCE_STOP=1 sh -c \
        "curl --proto '=https' --tlsv1.2 -fsSL 'https://go.minimal.dev/$CHANNEL' | sh -s -- --force-stop" \
        || fail "installer failed"
    [ -x "$BINDIR/min" ] || fail "installer ran but $BINDIR/min is missing"
fi

# --- 4. userns remediation + liveness ---------------------------------------
_rc=0; userns_ok || _rc=$?
case "$_rc" in
    1) fail "unprivileged user namespaces disabled (kernel.unprivileged_userns_clone=0 or max_user_namespaces=0) — needs a sysctl change with root" ;;
    2) remediate_apparmor ;;
esac

neutralize_insteadof

say "probing daemon (autospawns minimald on first contact)"
_i=0
while ! min ls >/dev/null 2>&1; do
    _i=$((_i+1))
    [ "$_i" -ge 30 ] && fail "minimald did not answer within 30s"
    sleep 1
done

say "OK: $(min --version 2>/dev/null | head -1)"
say "Minimal ready. Next: 'minbox up' (or /minimal:box-up) in your project."
