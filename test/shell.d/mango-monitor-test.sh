#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

tmpdir=$(mktemp -d)
cleanup() {
  [[ -f $tmpdir/runtime/omarchy-monitor-mirror.pid ]] &&
    kill "$(<"$tmpdir/runtime/omarchy-monitor-mirror.pid")" 2>/dev/null
  rm -rf "$tmpdir"
  return 0
}
trap cleanup EXIT

mock_bin="$tmpdir/bin"
calls="$tmpdir/calls"
outputs="$tmpdir/outputs.json"
mkdir -p "$mock_bin" "$tmpdir/home" "$tmpdir/runtime"

# The scripts run for real against stand-ins for every tool that reaches the
# live session: wlr-randr and mmsg answer from $outputs and log what they were
# asked to change, so nothing here reconfigures the running compositor.
cat >"$mock_bin/wlr-randr" <<'SH'
#!/bin/bash
if [[ $* == "--json" ]]; then
  cat "$FAKE_OUTPUTS"
else
  echo "wlr-randr $*" >>"$CALLS"
fi
SH
cat >"$mock_bin/mmsg" <<'SH'
#!/bin/bash
case "$1 $2" in
  "get all-monitors")
    jq '{monitors: [.[] | select(.enabled) | {name, active: (.name == env.FAKE_FOCUSED), scale}]}' "$FAKE_OUTPUTS"
    ;;
  *) echo "mmsg $*" >>"$CALLS" ;;
esac
SH
cat >"$mock_bin/omarchy-notification-send" <<'SH'
#!/bin/bash
echo "notify ${*: -1}" >>"$CALLS"
SH
cat >"$mock_bin/omarchy-hw-clamshell" <<'SH'
#!/bin/bash
[[ $FAKE_LID == "closed" ]]
SH
cat >"$mock_bin/wl-mirror" <<'SH'
#!/bin/bash
echo "wl-mirror $*" >>"$CALLS"
exec sleep 60
SH
chmod +x "$mock_bin"/*

toggles="$tmpdir/home/.local/state/omarchy/toggles/mango"
rules="$toggles/monitors.conf"

# $1 outputs: "laptop", "docked" (laptop + external) or "desktop" (external only).
set_outputs() {
  local laptop='{"name":"eDP-1","enabled":true,"make":"BOE","model":"0x0BCA","scale":1}'
  local external='{"name":"DP-1","enabled":true,"make":"Apple Computer Inc","model":"StudioDisplay","scale":2}'
  case "$1" in
    laptop) printf '[%s]\n' "$laptop" >"$outputs" ;;
    docked) printf '[%s,%s]\n' "$laptop" "$external" >"$outputs" ;;
    desktop) printf '[%s]\n' "$external" >"$outputs" ;;
  esac
}

run() {
  HOME="$tmpdir/home" XDG_RUNTIME_DIR="$tmpdir/runtime" PATH="$mock_bin:$ROOT/bin:$PATH" \
    CALLS="$calls" FAKE_OUTPUTS="$outputs" FAKE_FOCUSED="${FAKE_FOCUSED:-eDP-1}" FAKE_LID="${FAKE_LID:-open}" \
    "$@"
}

reset() {
  rm -rf "$tmpdir/home/.local" "$tmpdir/home/.config"
  : >"$calls"
}

reloads() {
  grep -c '^mmsg dispatch reload_config$' "$calls" || true
}

# Output lookups.
set_outputs docked
[[ $(run omarchy-mango-monitor-laptop) == "eDP-1" ]] || fail "laptop prints the built-in display"
run omarchy-mango-monitor-external-active || fail "external-active sees the enabled external monitor"
FAKE_FOCUSED=DP-1 run omarchy-mango-monitor-focused-apple || fail "focused-apple recognizes a Studio Display"
run omarchy-mango-monitor-focused-apple eDP-1 && fail "focused-apple rejects the laptop panel"
set_outputs laptop
run omarchy-mango-monitor-external-active && fail "external-active is false with only the laptop display"
set_outputs desktop
[[ -z $(run omarchy-mango-monitor-laptop) ]] || fail "laptop prints nothing on a desktop"
pass "monitor lookups read wlr-randr"

# apply writes toggle rules and reloads only on a change.
reset
set_outputs docked
run omarchy-mango-monitor-apply
[[ -f $rules && ! -s $rules ]] || fail "apply writes an empty rule file with no toggles"
(( $(reloads) == 1 )) || fail "apply reloads when the rule file first appears"
run omarchy-mango-monitor-apply
(( $(reloads) == 1 )) || fail "apply skips the reload when nothing changed"
pass "apply reloads mango only when the rules change"

# Turning the laptop display off and back on.
reset
set_outputs laptop
run omarchy-mango-monitor-internal off && fail "internal off refuses to disable the only display"
[[ ! -f $toggles/internal-monitor-disable ]] || fail "a refused internal off leaves no flag"
grep -q '^notify Não é possível desativar a única tela ativa$' "$calls" || fail "a refused internal off says why"

set_outputs docked
run omarchy-mango-monitor-internal off
[[ -f $toggles/internal-monitor-disable ]] || fail "internal off sets the flag"
grep -Fxq 'monitor_rule=name:^eDP-1$,disable:1' "$rules" || fail "internal off writes a disable rule for the laptop"
(( $(reloads) == 1 )) || fail "internal off reloads mango"

run omarchy-mango-monitor-internal toggle
[[ ! -f $toggles/internal-monitor-disable ]] || fail "internal toggle turns a disabled laptop back on"
[[ ! -s $rules ]] || fail "internal on drops the disable rule"
pass "internal display turns off only with an external monitor and back on"

# recover brings the laptop back once the external monitor is gone.
reset
set_outputs docked
run omarchy-mango-monitor-internal off
run omarchy-mango-monitor-internal recover
[[ -f $toggles/internal-monitor-disable ]] || fail "recover keeps the laptop off while docked"
set_outputs laptop
run omarchy-mango-monitor-internal recover
[[ ! -f $toggles/internal-monitor-disable ]] || fail "recover clears the flag once undocked"
[[ ! -s $rules ]] || fail "recover drops the disable rule once undocked"
pass "internal recover restores the laptop when the external monitor goes away"

# Clamshell follows the lid only while docked.
reset
set_outputs docked
FAKE_LID=closed run omarchy-mango-monitor-clamshell
[[ -f $toggles/internal-monitor-clamshell ]] || fail "a closed lid while docked sets the clamshell flag"
grep -Fxq 'monitor_rule=name:^eDP-1$,disable:1' "$rules" || fail "clamshell disables the laptop display"
FAKE_LID=closed run omarchy-mango-monitor-clamshell
(( $(reloads) == 1 )) || fail "a repeated clamshell sync does not reload again"
FAKE_LID=open run omarchy-mango-monitor-clamshell
[[ ! -f $toggles/internal-monitor-clamshell ]] || fail "opening the lid clears the clamshell flag"
[[ ! -s $rules ]] || fail "opening the lid re-enables the laptop display"

reset
set_outputs laptop
FAKE_LID=closed run omarchy-mango-monitor-clamshell
[[ ! -f $toggles/internal-monitor-clamshell ]] || fail "a closed lid without an external monitor is not clamshell"

reset
set_outputs desktop
FAKE_LID=closed run omarchy-mango-monitor-clamshell
[[ ! -e $rules ]] || fail "clamshell does nothing on a desktop"
pass "clamshell disables the laptop display only with the lid shut while docked"

# Scaling: steps between presets and persists the choice.
reset
set_outputs docked
[[ $(run omarchy-mango-monitor-scaling) == "1" ]] || fail "scaling prints the focused output's scale"
run omarchy-mango-monitor-scaling up
grep -Fxq 'wlr-randr --output eDP-1 --scale 1.25' "$calls" || fail "scaling up moves to the next preset"
[[ $(<"$toggles/scale-eDP-1") == "1.25" ]] || fail "scaling without a user rule persists as a toggle"
grep -Fxq 'monitor_rule=name:^eDP-1$,scale:1.25' "$rules" || fail "apply carries the toggle scale as a rule"

: >"$calls"
FAKE_FOCUSED=DP-1 run omarchy-mango-monitor-scaling down
grep -Fxq 'wlr-randr --output DP-1 --scale 1.6' "$calls" || fail "scaling down moves to the previous preset"

mkdir -p "$tmpdir/home/.config/mango"
printf '%s\n' 'monitor_rule=name:^eDP-1$,width:2880,height:1800' >"$tmpdir/home/.config/mango/monitors.conf"
run omarchy-mango-monitor-scaling 2
grep -Fxq 'monitor_rule=name:^eDP-1$,width:2880,height:1800,scale:2' "$tmpdir/home/.config/mango/monitors.conf" ||
  fail "scaling adds the scale to the user's own rule"
[[ ! -f $toggles/scale-eDP-1 ]] || fail "a user rule replaces the scale toggle"
run omarchy-mango-monitor-scaling 1.6
grep -Fxq 'monitor_rule=name:^eDP-1$,width:2880,height:1800,scale:1.6' "$tmpdir/home/.config/mango/monitors.conf" ||
  fail "scaling rewrites the scale already in the user's rule"

: >"$calls"
run omarchy-mango-monitor-scaling 5 2>/dev/null && fail "scaling rejects a scale above 4"
run omarchy-mango-monitor-scaling abc 2>/dev/null && fail "scaling rejects a non-number"
! grep -q '^wlr-randr --output' "$calls" || fail "a rejected scale changes nothing"
pass "scaling steps presets and persists to the user rule or a toggle"

# A disabled laptop display drops its scale rule: mango would re-enable it.
reset
set_outputs docked
run omarchy-mango-monitor-scaling 2
run omarchy-mango-monitor-internal off
! grep -q 'scale:' "$rules" || fail "apply drops the scale rule of a disabled laptop display"
pass "apply never re-enables a disabled laptop display through its scale rule"

# Mirroring runs wl-mirror while docked and stops when undocked.
reset
set_outputs docked
run omarchy-mango-monitor-internal-mirror on
pid_file="$tmpdir/runtime/omarchy-monitor-mirror.pid"
[[ -f $pid_file ]] && kill -0 "$(<"$pid_file")" 2>/dev/null || fail "mirror on starts wl-mirror"
for _ in {1..20}; do grep -q '^wl-mirror' "$calls" && break; sleep 0.05; done
grep -Fxq 'wl-mirror --fullscreen-output DP-1 eDP-1' "$calls" || fail "mirror shows the laptop display on the external monitor"
mirror_pid=$(<"$pid_file")
set_outputs laptop
run omarchy-mango-monitor-internal-mirror recover
[[ ! -f $pid_file ]] || fail "mirror recover clears the pid file once undocked"
for _ in {1..20}; do kill -0 "$mirror_pid" 2>/dev/null || break; sleep 0.05; done
kill -0 "$mirror_pid" 2>/dev/null && fail "mirror recover stops wl-mirror once undocked"
run omarchy-mango-monitor-internal-mirror on && fail "mirror on refuses without an external monitor"
pass "mirroring starts wl-mirror while docked and stops once undocked"

# Nothing above may reach the real compositor.
! grep -v '^mmsg dispatch reload_config$' "$calls" | grep -q '^mmsg' || fail "only reload_config is dispatched"
pass "monitor scripts only dispatch reload_config"
