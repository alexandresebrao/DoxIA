#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

run_node_test <<'JS'
const nightlight = requireFromRoot('shell/plugins/services/nightlight/NightlightModel.js')

assertEqual(nightlight.temperatureFromOutput('4000\n'), 4000, 'nightlight parses probe temperature')
assertEqual(nightlight.temperatureFromOutput(''), null, 'nightlight treats an empty probe as unknown')
assertEqual(nightlight.isNightlight(4000), true, 'nightlight reports warm temperatures as enabled')
assertEqual(nightlight.isNightlight(5999), true, 'nightlight reports warmer-than-identity values as enabled')
assertEqual(nightlight.isNightlight(6000), false, 'nightlight reports identity temperature as disabled')
assertEqual(nightlight.isNightlight(null), false, 'nightlight reports unknown temperature as disabled')
JS

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

mkdir -p "$TMPDIR/bin"
STATE="$TMPDIR/nightlight-temp"
SHELL_LOG="$TMPDIR/omarchy-shell-log"

cat >"$TMPDIR/bin/omarchy-nightlight-temperature" <<'SH'
#!/bin/bash

if [[ -n ${1:-} ]]; then
  printf '%s\n' "$1" >"$NIGHTLIGHT_STATE"
else
  cat "$NIGHTLIGHT_STATE" 2>/dev/null || exit 1
fi
SH

cat >"$TMPDIR/bin/pgrep" <<'SH'
#!/bin/bash
exit 0
SH

cat >"$TMPDIR/bin/omarchy-shell" <<'SH'
#!/bin/bash
printf '%s\n' "$*" >>"$OMARCHY_SHELL_LOG"
SH

chmod +x "$TMPDIR/bin/omarchy-nightlight-temperature" "$TMPDIR/bin/pgrep" "$TMPDIR/bin/omarchy-shell"

nightlight_cli() {
  PATH="$TMPDIR/bin:$PATH" \
  NIGHTLIGHT_STATE="$STATE" \
  OMARCHY_SHELL_LOG="$SHELL_LOG" \
    "$ROOT/bin/omarchy-toggle-nightlight" "$@"
}

nightlight_status() {
  printf '%s\n' "$1" >"$STATE"
  nightlight_cli --status
}

[[ $(nightlight_status 4000 | jq -r .enabled) == "true" ]] || fail "nightlight status reports 4000K as enabled"
pass "nightlight status reports 4000K as enabled"

[[ $(nightlight_status 5999 | jq -r .enabled) == "true" ]] || fail "nightlight status reports warmer-than-identity values as enabled"
pass "nightlight status reports warmer-than-identity values as enabled"

[[ $(nightlight_status 6000 | jq -r .enabled) == "false" ]] || fail "nightlight status reports identity temperature as disabled"
pass "nightlight status reports identity temperature as disabled"

[[ $(nightlight_status 6500 | jq -r .enabled) == "false" ]] || fail "nightlight status reports daylight temperature as disabled"
pass "nightlight status reports daylight temperature as disabled"

printf '6500\n' >"$STATE"
: >"$SHELL_LOG"
nightlight_cli >/dev/null
[[ $(<"$STATE") == 4000 ]] || fail "nightlight toggle warms the screen from daylight"
pass "nightlight toggle warms the screen from daylight"

grep -Fqx -- '-q nightlight refresh' "$SHELL_LOG" || fail "nightlight toggle nudges the shell nightlight service"
pass "nightlight toggle nudges the shell nightlight service"

nightlight_cli >/dev/null
[[ $(<"$STATE") == 6500 ]] || fail "nightlight toggle restores daylight from night light"
pass "nightlight toggle restores daylight from night light"

if rg -q 'omarchy.indicators' "$ROOT/bin/omarchy-toggle-nightlight"; then
  fail "nightlight toggle leaves indicator refresh to the nightlight service"
fi
pass "nightlight toggle leaves indicator refresh to the nightlight service"

# The real helper: gammastep holds the temperature while it runs.
mkdir -p "$TMPDIR/gamma-bin" "$TMPDIR/runtime"
cat >"$TMPDIR/gamma-bin/gammastep" <<'SH'
#!/bin/bash
exec sleep 30
SH
chmod +x "$TMPDIR/gamma-bin/gammastep"

temperature() {
  PATH="$TMPDIR/gamma-bin:$PATH" XDG_RUNTIME_DIR="$TMPDIR/runtime" "$ROOT/bin/omarchy-nightlight-temperature" "$@"
}

[[ $(temperature) == 6500 ]] || fail "nightlight helper reports daylight with no gammastep running"
pass "nightlight helper reports daylight with no gammastep running"

temperature 4000
[[ $(temperature) == 4000 ]] || fail "nightlight helper reports the temperature gammastep holds"
pass "nightlight helper reports the temperature gammastep holds"

read -r gamma_pid _ <"$TMPDIR/runtime/omarchy-nightlight"
temperature 6500
kill -0 "$gamma_pid" 2>/dev/null && fail "nightlight helper stops gammastep for daylight"
[[ $(temperature) == 6500 ]] || fail "nightlight helper stops gammastep for daylight"
pass "nightlight helper stops gammastep for daylight"
