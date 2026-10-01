#!/bin/bash

# One-time system setup for the FortiVPN bar widget. Run it with sudo:
#
#   sudo ~/.config/omarchy/plugins/klab.fortivpn/setup-system.sh [import-name]
#
# - Installs a root-owned copy of helper.sh as /usr/local/libexec/omarchy-fortivpn-helper,
#   which is the only thing that reads or writes /etc/openfortivpn/<name>.conf.
# - Installs polkit rules that let only your user run that helper and
#   start/stop/restart openfortivpn@<name>.service without a password.
# - Imports an existing /etc/openfortivpn/config as a profile (default "principal").
#
# Re-run it after changing helper.sh so the installed copy is updated.

set -euo pipefail

plugin_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
import_name="${1:-principal}"
target_user="${SUDO_USER:-}"
[[ -z $target_user && -n ${PKEXEC_UID:-} ]] && target_user=$(id -nu "$PKEXEC_UID")
helper="/usr/local/libexec/omarchy-fortivpn-helper"
rule="/etc/polkit-1/rules.d/50-omarchy-fortivpn-$target_user.rules"

(( EUID == 0 )) || { echo "Run with sudo." >&2; exit 1; }
[[ -n $target_user && $target_user != "root" ]] || { echo "Run with sudo from your own user." >&2; exit 1; }
[[ $import_name =~ ^[A-Za-z0-9_-]{1,32}$ ]] || { echo "Invalid profile name: $import_name" >&2; exit 1; }

install -D -o root -g root -m 755 "$plugin_dir/helper.sh" "$helper"
restorecon "$helper" 2>/dev/null || true
echo "Installed $helper"

cat >"$rule" <<RULE
// Installed by the klab.fortivpn Omarchy plugin.
polkit.addRule(function(action, subject) {
  if (subject.user != "$target_user" || !subject.local || !subject.active) return;

  if (action.id == "org.freedesktop.policykit.exec" &&
      action.lookup("program") == "$helper") {
    return polkit.Result.YES;
  }

  if (action.id == "org.freedesktop.systemd1.manage-units" &&
      /^openfortivpn@[A-Za-z0-9_-]+\.service\$/.test(action.lookup("unit") || "")) {
    var verb = action.lookup("verb");
    if (verb == "start" || verb == "stop" || verb == "restart") return polkit.Result.YES;
  }
});
RULE
chmod 644 "$rule"
echo "Installed $rule"

# Rules from the first version of this plugin.
rm -f "/etc/polkit-1/rules.d/50-openfortivpn-$target_user.rules"
[[ -L /etc/openfortivpn/default.conf ]] && rm -f /etc/openfortivpn/default.conf

legacy="/etc/openfortivpn/config"
imported="/etc/openfortivpn/$import_name.conf"
if [[ -f $legacy && ! -e $imported ]] && grep -qE '^[[:space:]]*host[[:space:]]*=' "$legacy"; then
  install -o root -g root -m 600 "$legacy" "$imported"
  restorecon "$imported" 2>/dev/null || true
  echo "Imported $legacy as profile '$import_name'"
fi

# Fedora builds openfortivpn without sd_notify support, so the packaged
# Type=notify unit never reports ready and systemd kills the tunnel on timeout.
dropin="/etc/systemd/system/openfortivpn@.service.d/10-omarchy.conf"
install -d -m 755 "$(dirname "$dropin")"
cat >"$dropin" <<DROPIN
# Installed by the klab.fortivpn Omarchy plugin.
[Service]
Type=simple
RestartSec=5
DROPIN
chmod 644 "$dropin"
echo "Installed $dropin"
systemctl daemon-reload

# Started by systemd, openfortivpn runs confined as openfortivpn_t, which may
# not add routes or run resolvconf/resolvectl. Make only that domain permissive
# (denials are still logged); the rest of the system stays enforcing.
if command -v semanage &>/dev/null && command -v getenforce &>/dev/null && [[ $(getenforce) != "Disabled" ]]; then
  if ! semanage permissive -l 2>/dev/null | grep -qx "openfortivpn_t"; then
    semanage permissive -a openfortivpn_t
    echo "SELinux: openfortivpn_t is now permissive"
  fi
fi

systemctl reset-failed 'openfortivpn@*' 2>/dev/null || true
echo "Done."
