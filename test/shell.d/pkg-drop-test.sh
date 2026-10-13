#!/bin/bash

set -euo pipefail

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/base-test.sh"

test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT

mock_path="$test_tmp/pkg-drop-bin"
mkdir -p "$mock_path"

# rpm -q matches installed package names, not what a package provides, so a
# virtual name counts as not installed.
cat >"$mock_path/rpm" <<'EOF'
#!/bin/bash
[[ $1 == "-q" ]] || exit 2
case $2 in
  exact-package | provider-package) exit 0 ;;
  *) exit 1 ;;
esac
EOF

cat >"$mock_path/sudo" <<'EOF'
#!/bin/bash
printf '%s\n' "$*" >"$TEST_TMP/pkg-drop-command"
EOF

chmod +x "$mock_path/rpm" "$mock_path/sudo"

PATH="$mock_path:$PATH" TEST_TMP="$test_tmp" \
  "$ROOT/bin/omarchy-pkg-drop" exact-package virtual-package provider-package

[[ $(<"$test_tmp/pkg-drop-command") == "dnf remove -y exact-package provider-package" ]] ||
  fail "package removal targets exact installed names only" "$(<"$test_tmp/pkg-drop-command")"
pass "package removal ignores names that are not installed packages"

rm -f "$test_tmp/pkg-drop-command"
PATH="$mock_path:$PATH" TEST_TMP="$test_tmp" \
  "$ROOT/bin/omarchy-pkg-drop" virtual-package
[[ ! -e $test_tmp/pkg-drop-command ]] || fail "package removal does nothing when nothing is installed"
pass "package removal does nothing when nothing is installed"
