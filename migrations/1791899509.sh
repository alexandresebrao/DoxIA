echo "Move the Hyper-V 1280x720 monitor rule out of ~/.config/mango/monitors.conf"

# The installer and update.sh appended it to the user's monitors.conf, where it
# also forced 1280x720 after the VM moved to another hypervisor. The session now
# applies it only on Hyper-V (default/mango/hyperv.conf). Only that exact rule
# and the Hyper-V comment right above it go.
monitors="$HOME/.config/mango/monitors.conf"
rule='monitor_rule=name:.*,width:1280,height:720,refresh:60,x:0,y:0,scale:1'

[[ -f $monitors ]] && grep -Fxq "$rule" "$monitors" || exit 0

awk -v rule="$rule" '
  { lines[NR] = $0 }
  END {
    for (i = 1; i <= NR; i++) {
      if (lines[i] == rule) { drop[i] = 1; if (i > 1 && lines[i - 1] ~ /^#.*Hyper-V/) drop[i - 1] = 1 }
    }
    # The blank line the installer put before the comment.
    for (i = 2; i <= NR; i++) if (drop[i] && !drop[i - 1] && lines[i - 1] == "") drop[i - 1] = 1
    for (i = 1; i <= NR; i++) if (!drop[i]) print lines[i]
  }' "$monitors" >"$monitors.tmp" && mv "$monitors.tmp" "$monitors"
