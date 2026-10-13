#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# busctl answers like doxia-shell would (or fails, when it isn't running);
# omarchy-shell records whether Quickshell's OSD was asked.
printf '%s\n' '#!/bin/bash' 'printf "%s\n" "$@" >"$TEST_DIR/busctl.args"' 'exit "$TEST_BUSCTL_EXIT"' >"$tmpdir/busctl"
printf '%s\n' '#!/bin/bash' 'printf "%s\n" "$@" >"$TEST_DIR/shell.args"' >"$tmpdir/omarchy-shell"
chmod +x "$tmpdir/busctl" "$tmpdir/omarchy-shell"

osd() {
  rm -f "$tmpdir/busctl.args" "$tmpdir/shell.args"
  TEST_DIR="$tmpdir" TEST_BUSCTL_EXIT=$1 PATH="$tmpdir:$ROOT/bin:$PATH" omarchy-osd "${@:2}"
}

osd 0 -i volume -p 40
[[ -f $tmpdir/busctl.args ]] || fail "omarchy-osd asks doxia-shell first"
[[ ! -f $tmpdir/shell.args ]] || fail "Quickshell is left alone when doxia-shell answers"
payload=$(sed -n 11p "$tmpdir/busctl.args")
[[ $(jq -r '.icon + " " + .value + " " + .progressText' <<<"$payload") == "volume 40 40%" ]] ||
  fail "doxia-shell gets the OSD payload: $payload"

osd 1 -m "Olá"
[[ -f $tmpdir/shell.args ]] || fail "Quickshell shows the OSD when doxia-shell is not running"
[[ $(sed -n 3p "$tmpdir/shell.args") == "show" ]] || fail "the fallback calls osd show"

pass "omarchy-osd routes to doxia-shell, then Quickshell"
