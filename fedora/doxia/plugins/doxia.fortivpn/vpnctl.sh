#!/bin/bash

# Connects/disconnects openfortivpn profiles for the FortiVPN panel.
#   vpnctl.sh connect <name>      stops any other profile, then starts <name>
#   vpnctl.sh disconnect [name]   stops <name>, or whatever profile is running
#   vpnctl.sh kill <pid>          stops an openfortivpn started by hand (asks for a password)

set -euo pipefail

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/omarchy-fortivpn"

running_units() {
  systemctl list-units 'openfortivpn@*' --state=active,activating --plain --no-legend --no-pager | awk '{print $1}'
}

case "${1:-}" in
connect)
  name="${2:-}"
  [[ $name =~ ^[A-Za-z0-9_-]{1,32}$ ]] || { echo "Nome inválido." >&2; exit 1; }
  mkdir -p "$state_dir"
  echo "$name" >"$state_dir/last"
  for unit in $(running_units); do
    [[ $unit == "openfortivpn@$name.service" ]] || systemctl stop "$unit"
  done
  systemctl reset-failed "openfortivpn@$name.service" 2>/dev/null || true
  systemctl start --no-block "openfortivpn@$name.service"
  ;;
disconnect)
  if [[ -n ${2:-} ]]; then
    systemctl stop --no-block "openfortivpn@$2.service"
  else
    for unit in $(running_units); do systemctl stop --no-block "$unit"; done
  fi
  ;;
kill)
  [[ ${2:-} =~ ^[0-9]+$ ]] || { echo "PID inválido." >&2; exit 1; }
  pkexec kill -TERM "$2"
  ;;
*)
  echo "Usage: ${0##*/} connect <name> | disconnect [name] | kill <pid>" >&2
  exit 1
  ;;
esac
