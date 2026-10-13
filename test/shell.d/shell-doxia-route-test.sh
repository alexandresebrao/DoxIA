#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# busctl answers as doxia-shell would (TEST_ANSWER), or fails like a bus
# without it; qs records whether the call went on to Quickshell.
cat >"$tmpdir/busctl" <<'STUB'
#!/bin/bash
printf '%s\n' "$@" >"$TEST_DIR/busctl.args"
[[ -n $TEST_ANSWER ]] || exit 1
jq -cn --arg a "$TEST_ANSWER" '{type: "s", data: [$a]}'
STUB
printf '%s\n' '#!/bin/bash' 'printf "%s\n" "$@" >"$TEST_DIR/qs.args"' 'echo quickshell' >"$tmpdir/qs"
chmod +x "$tmpdir/busctl" "$tmpdir/qs"

call() {
  local answer=$1
  shift
  rm -f "$tmpdir/busctl.args" "$tmpdir/qs.args"
  OMARCHY_PATH="$ROOT" TEST_DIR="$tmpdir" TEST_ANSWER=$answer DOXIA_SHELL=1 PATH="$tmpdir:$ROOT/bin:$PATH" \
    WAYLAND_DISPLAY=wayland-test omarchy-shell "$@"
}

out=$(call on notifications toggleDnd)
[[ $out == "on" ]] || fail "doxia-shell's answer is the output: $out"
[[ ! -f $tmpdir/qs.args ]] || fail "Quickshell is left alone when doxia-shell answers"
[[ $(sed -n 8,12p "$tmpdir/busctl.args" | paste -sd' ') == "Call ssas notifications toggleDnd 0" ]] ||
  fail "the call carries target, method and no args: $(paste -sd' ' "$tmpdir/busctl.args")"

out=$(call ok osd show '{"value": "40"}')
[[ $(sed -n 12,13p "$tmpdir/busctl.args" | paste -sd'|') == '1|{"value": "40"}' ]] || fail "args go through as one string each"

out=$(call "Target not found." shell ping)
[[ $out == "quickshell" && -f $tmpdir/qs.args ]] || fail "targets doxia-shell lacks go to Quickshell"

out=$(call "" osd show "{}")
[[ $out == "quickshell" ]] || fail "without doxia-shell on the bus, Quickshell answers"

if call "Function not found." osd nope 2>/dev/null; then fail "an unknown method fails"; fi

rm -f "$tmpdir/busctl.args"
OMARCHY_PATH="$ROOT" TEST_DIR="$tmpdir" TEST_ANSWER=on PATH="$tmpdir:$ROOT/bin:$PATH" WAYLAND_DISPLAY=wayland-test \
  omarchy-shell osd ping >/dev/null
[[ ! -f $tmpdir/busctl.args ]] || fail "DOXIA_SHELL=0 (the test default) skips doxia-shell"

pass "omarchy-shell asks doxia-shell first, then Quickshell"
