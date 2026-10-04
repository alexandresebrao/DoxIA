echo "Run fedora/update.sh after every update (DoxIA post-update hook)"

# The Atualizar menu (omarchy update) only pulls, upgrades packages and migrates,
# so the DoxIA changes in the repo (bar widgets, Kitty, chafa, branding) never
# reached updated machines. The hook it runs right after the migrations now
# calls fedora/update.sh, starting with this very update. Rewriting the hook is
# idempotent.
bash "$OMARCHY_PATH/fedora/doxia/install-update-hook"
