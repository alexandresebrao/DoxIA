#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

stub_dir="$tmpdir/bin"
home_dir="$tmpdir/home"
xdg_decoy="$tmpdir/xdg-decoy"
log_file="$tmpdir/hyprctl.log"
mmsg_log="$tmpdir/mmsg.log"
marker="$tmpdir/marker"
mkdir -p "$stub_dir" "$home_dir" "$xdg_decoy"

toggles_file="$home_dir/.local/state/omarchy/toggles/mango/input.conf"

# Legacy Hyprland state, which migrations/1787618700.sh still cleans up.
state_dir="$home_dir/.local/state/omarchy/toggles/hypr"
name_file="$state_dir/touchpad-disabled-name"
state_lua="$state_dir/touchpad-disabled.lua"

cat >"$stub_dir/hyprctl" <<'EOF'
#!/bin/bash
case $1 in
  eval) printf '%s\n' "$2" >>"$HYPRCTL_LOG" ;;
  reload) printf 'reload\n' >>"$HYPRCTL_LOG" ;;
esac
EOF

# Never let the toggle reach the live mango session.
cat >"$stub_dir/mmsg" <<'EOF'
#!/bin/bash
printf '%s\n' "$*" >>"$MMSG_LOG"
EOF

cat >"$stub_dir/omarchy-osd" <<'EOF'
#!/bin/bash
:
EOF
chmod +x "$stub_dir"/*

stub_device() {
  local kind=$1
  local name=$2
  cat >"$stub_dir/omarchy-hw-$kind" <<EOF
#!/bin/bash
printf '%s\n' '$name'
EOF
  chmod +x "$stub_dir/omarchy-hw-$kind"
}

no_device() {
  printf '#!/bin/bash\n:\n' >"$stub_dir/omarchy-hw-$1"
  chmod +x "$stub_dir/omarchy-hw-$1"
}

# XDG_STATE_HOME deliberately points away from HOME everywhere below: the
# input-device state is hardcoded to ~/.local/state, which is the path
# config/mango/config.conf sources, so nothing may read or write the XDG
# directory.
run_toggle() {
  HOME="$home_dir" \
    XDG_STATE_HOME="$xdg_decoy" \
    HYPRCTL_LOG="$log_file" \
    MMSG_LOG="$mmsg_log" \
    PATH="$stub_dir:$ROOT/bin:$PATH" \
    "$ROOT/bin/omarchy-toggle-input-device" "$@"
}

assert_decoy_untouched() {
  [[ -z $(find "$xdg_decoy" -mindepth 1 -print -quit 2>/dev/null) ]] ||
    fail "input-device state must ignore XDG_STATE_HOME"
}

toggles() {
  cat "$toggles_file" 2>/dev/null || true
}

grep -Fx 'source_optional=~/.local/state/omarchy/toggles/mango/input.conf' "$ROOT/config/mango/config.conf" >/dev/null ||
  fail "mango config sources the input-device toggles"
pass "mango config sources the input-device toggles"

stub_device touchpad 'elan-touchpad'
stub_device touchscreen 'wacom-hid-52eb-finger'

: >"$mmsg_log"
run_toggle touchpad off
[[ $(toggles) == "disable_trackpad=1" ]] || fail "touchpad disable writes disable_trackpad" "$(toggles)"
grep -Fx 'dispatch reload_config' "$mmsg_log" >/dev/null || fail "touchpad disable reloads mango"
assert_decoy_untouched
pass "touchpad disable persists disable_trackpad and reloads mango"

run_toggle touchpad off
[[ $(toggles) == "disable_trackpad=1" ]] || fail "disabling twice does not repeat the option" "$(toggles)"
pass "disabling twice keeps a single option line"

: >"$mmsg_log"
run_toggle touchpad on
[[ -z $(toggles) ]] || fail "touchpad enable clears disable_trackpad" "$(toggles)"
grep -Fx 'dispatch reload_config' "$mmsg_log" >/dev/null || fail "touchpad enable reloads mango"
pass "touchpad enable clears the option and reloads mango"

run_toggle touchpad
[[ $(toggles) == "disable_trackpad=1" ]] || fail "default toggle action disables an enabled touchpad"
run_toggle touchpad
[[ -z $(toggles) ]] || fail "default toggle action enables a disabled touchpad"
pass "default toggle action flips the persisted state"

run_toggle touchscreen off
[[ $(toggles) == "touch_enable=0" ]] || fail "touchscreen disable writes touch_enable=0" "$(toggles)"
run_toggle touchpad off
run_toggle touchscreen on
[[ $(toggles) == "disable_trackpad=1" ]] || fail "touchscreen enable keeps the touchpad disabled" "$(toggles)"
run_toggle touchpad on
[[ -z $(toggles) ]] || fail "both kinds can be enabled again" "$(toggles)"
pass "touchpad and touchscreen toggle independently"

rm -f "$toggles_file" "$marker"
stub_device touchpad 'touchpad"; touch '"$marker"'; echo "'
run_toggle touchpad off
[[ ! -e $marker ]] || fail "touchpad disable does not execute metacharacters in the device name"
[[ $(toggles) == "disable_trackpad=1" ]] || fail "the device name never reaches the mango config" "$(toggles)"
run_toggle touchpad on
pass "a hostile device name is neither executed nor persisted"

rm -f "$toggles_file"
: >"$mmsg_log"
run_toggle touchpad on
[[ ! -e $toggles_file ]] || fail "enable with no saved state writes nothing"
grep -Fx 'dispatch reload_config' "$mmsg_log" >/dev/null || fail "enable with no saved state still reloads"
pass "enable without saved state is a no-op reload"

no_device touchpad
set +e
run_toggle touchpad off >/dev/null 2>&1
status=$?
set -e
(( status != 0 )) || fail "disable errors when no device is found"
[[ ! -e $toggles_file ]] || fail "no state is written when no device is found"
pass "disable errors when no device is found"

for args in "" "keyboard" "touchpad sideways"; do
  set +e
  # shellcheck disable=SC2086
  run_toggle $args >/dev/null 2>&1
  status=$?
  set -e
  (( status == 1 )) || fail "rejects invalid arguments: '$args'"
done
[[ ! -e $toggles_file ]] || fail "invalid arguments write no state"
pass "invalid kinds and actions are rejected"

# The migration runs with the same XDG decoy: legacy files were written to
# ~/.local/state, so that is where it must look no matter what XDG says.
run_migration() {
  HOME="$home_dir" XDG_STATE_HOME="$xdg_decoy" HYPRCTL_LOG="$log_file" \
    PATH="$stub_dir:$ROOT/bin:$PATH" \
    bash -euo pipefail "$ROOT/migrations/1787618700.sh" >/dev/null
}

mkdir -p "$state_dir"
rm -f "$state_dir"/*-disabled-name
printf 'hl.device({ name = "synps/2-synaptics-touchpad", enabled = false })\n' >"$state_lua"
printf 'hl.device({ name = "hostile\\"")", enabled = false })\n' >"$state_dir/touchscreen-disabled.lua"

: >"$log_file"
run_migration
[[ $(<"$name_file") == "synps/2-synaptics-touchpad" ]] ||
  fail "migration recovers a device name containing a slash"
[[ ! -e $state_lua ]] || fail "migration deletes the generated touchpad Lua"
[[ ! -e $state_dir/touchscreen-disabled-name ]] ||
  fail "migration does not copy a hostile name out of generated Lua"
[[ ! -e $state_dir/touchscreen-disabled.lua ]] ||
  fail "migration deletes hostile generated Lua even when no name is recovered"
assert_decoy_untouched
# The package hook reloads Hyprland before migrations run, so the disable was
# already dropped for this session; the migration has to put it back.
grep -Fx 'reload' "$log_file" >/dev/null ||
  fail "migration reloads so the recovered disable applies to this session"
pass "migration recovers plain names and discards hostile generated Lua"

printf 'kept-name\n' >"$name_file"
printf 'hl.device({ name = "other-touchpad", enabled = false })\n' >"$state_lua"
run_migration
[[ $(<"$name_file") == "kept-name" ]] || fail "migration keeps an existing device-name file"
[[ ! -e $state_lua ]] || fail "migration still deletes the generated Lua"
pass "migration is idempotent over an existing device-name file"

rm -f "$name_file"
printf 'garbage\n' >"$state_lua"
chmod 000 "$state_lua"
run_migration
[[ ! -e $state_lua ]] || fail "migration removes an unreadable generated Lua"
[[ ! -e $name_file ]] || fail "no name is recovered from an unreadable file"
pass "an unreadable state file does not wedge the migration"

: >"$log_file"
run_migration
[[ ! -s $log_file ]] || fail "migration with nothing to migrate does not reload"
pass "migration no-ops with nothing left to migrate"
