# Atualizar DoxIA

The update window, as a GTK 4 app in Rust. It used to be the `omarchy.update`
shell panel (`shell/plugins/update`), which closed and flickered while the
update's "Configuração do DoxIA" step reloaded the shell. Same views, texts and
actions; the work stays in `bin/` (`omarchy-update-run`, `omarchy-update-pref`,
`omarchy-update-check`), and the window follows the JSON files described in
`src/model.rs`. The look comes from the current theme (`src/theme.rs`).

`omarchy-launch-update` opens it (`--prefs` opens on the preferences); the
panel stays as the fallback when the app is not installed.

## Shipping a new build

```bash
sudo dnf install cargo gtk4-devel
cd fedora/doxia/atualizar
cargo test && cargo build --release
cp target/release/doxia-atualizar "/tmp/doxia-atualizar-$(uname -m)"
gh release create doxia-atualizar-<version> "/tmp/doxia-atualizar-$(uname -m)" \
  --target fedora --latest=false
sha256sum target/release/doxia-atualizar
```

Then set `version` and `sha256` in `fedora/doxia/install-atualizar`, which
`fedora/install-system.sh` and `fedora/update.sh` run.
