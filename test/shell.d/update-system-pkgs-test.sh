#!/bin/bash
#
# omarchy-update-system-pkgs runs dnf's upgrade, now or staged for the next
# boot. For the update window it also turns dnf's [n/total] lines into step
# progress, and must still hand back dnf's own exit status through that pipe.

set -euo pipefail

source "$(dirname "$0")/base-test.sh"

test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT

stub_bin="$test_tmp/bin"
mkdir -p "$stub_bin"

cat >"$stub_bin/sudo" <<'STUB'
#!/bin/bash
exec "$@"
STUB

# Prints the transcript under test and exits with the status the case asks for.
cat >"$stub_bin/dnf" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$DNF_CALLS"
[[ -f $DNF_OUTPUT ]] && cat "$DNF_OUTPUT"
exit "${DNF_STATUS:-0}"
STUB

cat >"$stub_bin/omarchy-update-state" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$STATE_CALLS"
STUB

chmod +x "$stub_bin/sudo" "$stub_bin/dnf" "$stub_bin/omarchy-update-state"

run_system_pkgs() {
  : >"$test_tmp/dnf-calls"
  : >"$test_tmp/state-calls"
  DNF_CALLS="$test_tmp/dnf-calls" \
    DNF_OUTPUT="$test_tmp/dnf-output" \
    STATE_CALLS="$test_tmp/state-calls" \
    PATH="$stub_bin:$PATH" \
    bash "$ROOT/bin/omarchy-update-system-pkgs" "$@" >"$test_tmp/out" 2>&1
}

: >"$test_tmp/dnf-output"

run_system_pkgs || fail "a terminal upgrade that works reports a failure"
[[ $(<"$test_tmp/dnf-calls") == "upgrade --refresh -y --setopt=max_parallel_downloads=10" ]] ||
  fail "a terminal upgrade runs dnf upgrade --refresh -y" "$(<"$test_tmp/dnf-calls")"
[[ ! -s $test_tmp/state-calls ]] || fail "a terminal upgrade reports no window progress"
pass "a terminal upgrade installs the packages now"

run_system_pkgs --offline || fail "a staged upgrade that works reports a failure"
[[ $(<"$test_tmp/dnf-calls") == "upgrade --refresh -y --setopt=max_parallel_downloads=10 --offline" ]] ||
  fail "--offline stages the upgrade for the next boot" "$(<"$test_tmp/dnf-calls")"
pass "--offline stages the upgrade for the next boot"

if DNF_STATUS=1 run_system_pkgs; then
  fail "a failed terminal upgrade passes for a whole one"
fi
pass "a failed terminal upgrade fails the step"

# Downloads fill the first half of the step and the transaction the second;
# each percentage is reported once.
cat >"$test_tmp/dnf-output" <<'OUT'
Updating and loading repositories:
[1/2] foo-1.0-1.fc44.x86_64 100% |   1.0 MiB/s |   1.0 MiB |  00m01s
[1/2] foo-1.0-1.fc44.x86_64 100% |   1.0 MiB/s |   1.0 MiB |  00m01s
[2/2] bar-2.0-1.fc44.x86_64 100% |   1.0 MiB/s |   1.0 MiB |  00m01s
Running transaction
[1/4] Verify package files 100% |   1.0 KiB/s |   2.0   B |  00m00s
[4/4] Upgrading bar-0:2.0-1.fc44.x86_64 100% |   1.0 KiB/s |   2.0   B |  00m00s
Complete!
OUT

OMARCHY_UPDATE_GUI=1 run_system_pkgs || fail "a window upgrade that works reports a failure"
grep -qx 'Complete!' "$test_tmp/out" || fail "a window upgrade still shows dnf's output" "$(<"$test_tmp/out")"
diff <(printf '%s\n' \
  'frac system 0.25 1/2 · foo-1.0-1.fc44.x86_64' \
  'frac system 0.5 2/2 · bar-2.0-1.fc44.x86_64' \
  'frac system 0.625 1/4 · Verify' \
  'frac system 1 4/4 · Upgrading') "$test_tmp/state-calls" >"$test_tmp/progress" ||
  fail "a window upgrade reports download, then transaction progress" "$(<"$test_tmp/progress")"
pass "a window upgrade reports download, then transaction progress"

# Staged for the next boot, downloading is all the step does.
cat >"$test_tmp/dnf-output" <<'OUT'
[1/2] foo-1.0-1.fc44.x86_64 100% |   1.0 MiB/s |   1.0 MiB |  00m01s
[2/2] bar-2.0-1.fc44.x86_64 100% |   1.0 MiB/s |   1.0 MiB |  00m01s
OUT

OMARCHY_UPDATE_GUI=1 run_system_pkgs --offline || fail "a staged window upgrade that works reports a failure"
diff <(printf '%s\n' \
  'frac download 0.5 1/2 · foo-1.0-1.fc44.x86_64' \
  'frac download 1 2/2 · bar-2.0-1.fc44.x86_64') "$test_tmp/state-calls" >"$test_tmp/progress" ||
  fail "a staged window upgrade fills the download step" "$(<"$test_tmp/progress")"
pass "a staged window upgrade fills the download step"

# The progress reader sits at the end of a pipe, so dnf's status has to be
# carried past it or a failed upgrade would read as done.
if DNF_STATUS=1 OMARCHY_UPDATE_GUI=1 run_system_pkgs; then
  fail "a failed window upgrade passes for a whole one"
fi
pass "a failed window upgrade fails the step"
