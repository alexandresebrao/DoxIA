#!/bin/bash

# omarchy-update-check (dnf, Flatpak and the DoxIA checkout, as JSON) and
# omarchy-update-available (its one-line-per-source summary), with stubbed
# dnf, rpm, flatpak and git.

set -euo pipefail

source "$(dirname "$0")/base-test.sh"

require_command jq

test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT

stub_bin="$test_tmp/bin"
git_log="$test_tmp/git.log"
mkdir -p "$stub_bin" "$test_tmp/home"

cat >"$stub_bin/dnf" <<'SH'
#!/bin/bash
case "${TEST_DNF:-updates}" in
  updates)
    printf 'Upgrades\nflatpak.x86_64                1.18.4-1.fc44 updates\nflatpak-libs.x86_64           1.18.4-1.fc44 updates\nrsync.x86_64                  3.5.1-1.fc44  updates\n'
    exit 100
    ;;
  none) exit 0 ;;
  fail) echo "repo down" >&2; exit 1 ;;
esac
SH

cat >"$stub_bin/rpm" <<'SH'
#!/bin/bash
printf 'flatpak.x86_64 1.18.2-1.fc44\nflatpak-libs.x86_64 1.18.2-1.fc44\nrsync.x86_64 3.5.0-2.fc44\n'
SH

cat >"$stub_bin/flatpak" <<'SH'
#!/bin/bash
case "${TEST_FLATPAK:-none}" in
  updates) printf 'app/com.spotify.Client/x86_64/stable\tSpotify\nruntime/org.freedesktop.Platform/x86_64/25.08\tFreedesktop Platform\n' ;;
  none) ;;
esac
SH

cat >"$stub_bin/git" <<'SH'
#!/bin/bash
printf '%s\n' "$*" >>"$TEST_GIT_LOG"
shift 2
case "$1" in
  fetch) exit 0 ;;
  rev-parse)
    [[ $2 == "--short" ]] && { echo abc1234; exit 0; }
    [[ ${TEST_GIT_UPSTREAM:-origin/fedora} != "none" ]] || exit 1
    echo "${TEST_GIT_UPSTREAM:-origin/fedora}"
    ;;
  log)
    for ((i = 1; i <= ${TEST_GIT_BEHIND:-0}; i++)); do echo "Commit $i"; done
    ;;
esac
SH
chmod +x "$stub_bin"/*

# run <tool> [VAR=value...] [args...]
run() {
  local tool=$1 vars=() args=()
  shift
  for a in "$@"; do
    if [[ $a == *=* ]]; then vars+=("$a"); else args+=("$a"); fi
  done
  env HOME="$test_tmp/home" TEST_GIT_LOG="$git_log" PATH="$stub_bin:$PATH" \
    OMARCHY_PATH="$test_tmp/checkout" "${vars[@]}" "$ROOT/bin/$tool" "${args[@]}"
}

out="$test_tmp/out"

# dnf updates: listed with installed and new versions
run omarchy-update-check TEST_DNF=updates >"$out" && status=0 || status=$?
(( status == 0 )) || fail "update check exits 0 when dnf has updates"
[[ $(jq '.system | length' "$out") == 3 ]] || fail "update check lists each dnf package" "$(cat "$out")"
[[ $(jq -r '.system[] | select(.name == "rsync") | "\(.from) \(.to)"' "$out") == "3.5.0-2.fc44 3.5.1-1.fc44" ]] ||
  fail "update check pairs the installed and new versions" "$(cat "$out")"
[[ -s $test_tmp/home/.cache/omarchy/update-status.json ]] || fail "update check keeps a cached copy"
pass "update check lists dnf updates with versions"

# --cached reads the copy without asking dnf again
run omarchy-update-check TEST_DNF=fail --cached >"$out" && status=0 || status=$?
[[ $(jq '.system | length' "$out") == 3 ]] || fail "update check --cached prints the last summary"
pass "update check --cached reuses the last summary"

# Flatpak apps and runtimes
run omarchy-update-check TEST_DNF=none TEST_FLATPAK=updates >"$out" && status=0 || status=$?
(( status == 0 )) || fail "update check exits 0 when only Flatpak has updates"
[[ $(jq -r '.flatpak.apps[0].name' "$out") == "Spotify" && $(jq '.flatpak.runtimes' "$out") == 1 ]] ||
  fail "update check separates Flatpak apps and runtimes" "$(cat "$out")"
pass "update check lists Flatpak apps and runtimes"

# A dnf failure is reported, not mistaken for "up to date" silently
run omarchy-update-check TEST_DNF=fail >"$out" && status=0 || status=$?
[[ $(jq '.errors | length' "$out") == 1 ]] || fail "update check reports a dnf failure" "$(cat "$out")"
pass "update check reports a repository failure"

# DoxIA commits
: >"$git_log"
run omarchy-update-available TEST_DNF=none TEST_GIT_BEHIND=2 >"$out" && status=0 || status=$?
(( status == 0 )) || fail "update checker exits successfully when dev commits are available"
grep -Fx 'omarchy-dev-checkout 2 new commits on origin/fedora' "$out" >/dev/null ||
  fail "update checker reports available dev commits" "$(cat "$out")"
grep -Fx -- "-C $test_tmp/checkout fetch --quiet" "$git_log" >/dev/null ||
  fail "update checker fetches the dev checkout upstream" "$(cat "$git_log")"
pass "update checker detects new commits in the dev checkout"

# Summary lines per source
run omarchy-update-available TEST_DNF=updates TEST_FLATPAK=updates >"$out" && status=0 || status=$?
(( status == 0 )) || fail "update checker exits successfully when updates are available"
grep -Fx '3 system package(s)' "$out" >/dev/null || fail "update checker counts system packages" "$(cat "$out")"
grep -Fx '1 Flatpak app(s)' "$out" >/dev/null || fail "update checker counts Flatpak apps" "$(cat "$out")"
pass "update checker summarizes each source"

# Nothing to do
run omarchy-update-available TEST_DNF=none TEST_GIT_UPSTREAM=none >"$out" && status=0 || status=$?
(( status == 1 )) || fail "update checker exits non-zero when no updates are available"
grep -q '^DoxIA is up to date$' "$out" || fail "update checker prints up-to-date message"
pass "update checker reports an up-to-date system"
