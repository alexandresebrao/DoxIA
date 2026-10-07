#!/bin/bash

# Prints the openfortivpn state as one JSON object for the FortiVPN panel.
# Usage: status.sh [focus-profile]
#
# The focus profile is the one the panel last tried to connect; its unit state
# and latest journal error are reported even when it is not running.

state_file="${XDG_STATE_HOME:-$HOME/.local/state}/omarchy-fortivpn/last"
focus="${1:-}"
[[ -n $focus ]] || focus=$(cat "$state_file" 2>/dev/null)

# The running (or starting) profile, if any.
active_profile=""
active_state=""
while read -r unit _ active _; do
  [[ $unit == openfortivpn@*.service ]] || continue
  if [[ $active == "active" || $active == "activating" || $active == "deactivating" ]]; then
    active_profile=${unit#openfortivpn@}
    active_profile=${active_profile%.service}
    active_state=$active
    break
  fi
done < <(systemctl list-units 'openfortivpn@*' --all --plain --no-legend --no-pager 2>/dev/null)

[[ -n $active_profile ]] && focus=$active_profile
focus_unit="openfortivpn@$focus.service"
focus_state=""
since_epoch=0
error=""
cert=""
if [[ -n $focus ]]; then
  focus_state=$(systemctl show "$focus_unit" -p ActiveState --value 2>/dev/null)
  since=$(systemctl show "$focus_unit" -p ActiveEnterTimestamp --value 2>/dev/null)
  [[ $focus_state == "active" && -n $since ]] && since_epoch=$(date -d "$since" +%s 2>/dev/null || echo 0)

  if [[ $focus_state == "failed" || $focus_state == "activating" || ( $focus_state == "active" && -z $(ip -br link show type ppp 2>/dev/null) ) ]]; then
    log=$(journalctl -u "$focus_unit" -n 40 -o cat --no-pager 2>/dev/null)
    error=$(grep -E "ERROR" <<<"$log" | grep -v -- "trusted-cert" | tail -n 1 | sed -E 's/^ERROR:[[:space:]]*//')
    cert=$(grep -oE "trusted-cert = [0-9a-fA-F]{64}" <<<"$log" | tail -n 1 | awk '{print $3}')
  fi
fi

# An openfortivpn started by hand in a terminal, outside the systemd units.
main_pid=0
[[ -n $active_profile ]] && main_pid=$(systemctl show "openfortivpn@$active_profile.service" -p MainPID --value)
external_pid=0
for pid in $(pgrep -x openfortivpn); do
  if [[ $pid != "$main_pid" ]]; then
    external_pid=$pid
    (( since_epoch == 0 )) && since_epoch=$(( $(date +%s) - $(ps -o etimes= -p "$pid" | tr -d ' ') ))
    break
  fi
done

iface=$(ip -j link show type ppp 2>/dev/null | jq -r '.[0].ifname // empty')
ip_addr=""
peer=""
routes=0
if [[ -n $iface ]]; then
  ip_addr=$(ip -j -4 addr show dev "$iface" | jq -r '.[0].addr_info[0].local // empty')
  peer=$(ip -j -4 addr show dev "$iface" | jq -r '.[0].addr_info[0].address // empty')
  routes=$(ip -j -4 route show dev "$iface" | jq 'length')
fi

helper_installed=false
[[ -x /usr/local/libexec/omarchy-fortivpn-helper ]] && helper_installed=true

jq -cn \
  --argjson helperInstalled "$helper_installed" \
  --arg activeProfile "$active_profile" \
  --arg activeState "$active_state" \
  --arg focus "$focus" \
  --arg focusState "${focus_state:-}" \
  --argjson externalPid "${external_pid:-0}" \
  --argjson since "${since_epoch:-0}" \
  --arg iface "$iface" \
  --arg ip "$ip_addr" \
  --arg peer "$peer" \
  --argjson routes "${routes:-0}" \
  --arg error "$error" \
  --arg cert "$cert" \
  '{helperInstalled: $helperInstalled, activeProfile: $activeProfile, activeState: $activeState, focus: $focus, focusState: $focusState,
    externalPid: $externalPid, since: $since, iface: $iface, ip: $ip, peer: $peer, routes: $routes,
    error: $error, cert: $cert}'
