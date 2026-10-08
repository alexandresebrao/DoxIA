hl.on("hyprland.start", function()
  -- Slow app launch fix -- set systemd vars before starting session services.
  hl.exec_cmd("systemctl --user import-environment $(env | cut -d'=' -f 1)")
  hl.exec_cmd("dbus-update-activation-environment --systemd --all")

  -- Finish starting the keyring the login unlocked (pam_gnome_keyring): without
  -- --start it quits after 120s, and the first app to want a secret then gets a
  -- fresh, locked one asking for the password. GNOME's own autostart for this is
  -- OnlyShowIn=GNOME, so it is skipped under Hyprland.
  hl.exec_cmd("gnome-keyring-daemon --start --components=secrets")

  hl.exec_cmd("omarchy-launch-shell")
  hl.exec_cmd("omarchy-provision-first-run")
  hl.exec_cmd("omarchy-powerprofiles-init")
  hl.exec_cmd(o.launch("omarchy-hyprland-monitor-watch"))
  hl.exec_cmd(o.launch("udiskie --automount --no-notify --no-tray"))

  -- Run post-boot hooks after startup config has loaded.
  hl.exec_cmd("sleep 2 && omarchy-hook post-boot")
end)
