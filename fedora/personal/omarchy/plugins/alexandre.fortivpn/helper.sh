#!/bin/bash

# Privileged profile manager for the alexandre.fortivpn plugin. setup-system.sh
# installs a root-owned copy at /usr/local/libexec/omarchy-fortivpn-helper and
# a polkit rule so the panel can run it through pkexec without a password.
#
#   list                    JSON array of profiles (never includes passwords)
#   save <name> [old-name]  reads one JSON object on stdin, writes <name>.conf
#   delete <name>           stops the tunnel and removes <name>.conf
#   trust <name> <digest>   adds a trusted-cert digest to <name>.conf
#
# Only whitelisted openfortivpn keys are ever written, so a caller cannot slip
# in pppd-plugin / pppd-call (arbitrary code as root).

set -euo pipefail

CONF_DIR="${FORTIVPN_TEST_CONF_DIR:-/etc/openfortivpn}"
NAME_RE='^[A-Za-z0-9_-]{1,32}$'
DIGEST_RE='^[0-9a-fA-F]{64}$'

fail() {
  echo "$*" >&2
  exit 1
}

valid_name() {
  [[ $1 =~ $NAME_RE ]] || fail "Nome inválido: use letras, números, - ou _ (até 32)."
}

conf_path() {
  echo "$CONF_DIR/$1.conf"
}

# Reads "key = value" pairs from a config, one "key<TAB>value" per line.
read_conf() {
  sed -nE 's/^[[:space:]]*([A-Za-z0-9-]+)[[:space:]]*=[[:space:]]*(.*[^[:space:]])[[:space:]]*$/\1\t\2/p' "$1"
}

conf_value() {
  read_conf "$1" | awk -F'\t' -v k="$2" '$1 == k { print $2; exit }'
}

list_profiles() {
  local file name out="[]"
  shopt -s nullglob
  for file in "$CONF_DIR"/*.conf; do
    name=$(basename "$file" .conf)
    [[ $name =~ $NAME_RE ]] || continue
    out=$(read_conf "$file" | jq -R -s --arg name "$name" --argjson acc "$out" '
      [split("\n")[] | select(length > 0) | split("\t") | {key: .[0], value: .[1]}] as $kv
      | def get($k): ([$kv[] | select(.key == $k) | .value] | first) // "";
        $acc + [{
          name: $name,
          host: get("host"),
          port: get("port"),
          username: get("username"),
          realm: get("realm"),
          trustedCerts: [$kv[] | select(.key == "trusted-cert") | .value],
          setDns: (get("set-dns") != "0"),
          setRoutes: (get("set-routes") != "0"),
          acceptRemote: (get("pppd-accept-remote") == "1"),
          hasPassword: (get("password") != "")
        }]')
  done
  echo "$out"
}

# Rejects control characters (newlines would inject extra config lines).
clean() {
  local value="$1" label="$2"
  [[ $value != *[[:cntrl:]]* ]] || fail "$label contém caracteres inválidos."
  echo "$value"
}

save_profile() {
  local name="$1" old="${2:-}" input host port username password realm certs set_dns set_routes accept_remote
  valid_name "$name"
  [[ -z $old ]] || valid_name "$old"

  IFS= read -r input || true
  jq -e 'type == "object"' <<<"$input" >/dev/null 2>&1 || fail "Dados inválidos."
  field() { jq -r --arg k "$1" 'if has($k) and .[$k] != null then .[$k] | tostring else "" end' <<<"$input"; }

  host=$(clean "$(field host)" "Host")
  port=$(clean "$(field port)" "Porta")
  username=$(clean "$(field username)" "Usuário")
  password=$(clean "$(field password)" "Senha")
  realm=$(clean "$(field realm)" "Realm")
  certs=$(clean "$(field trustedCerts)" "Certificado")
  set_dns=$(field setDns)
  set_routes=$(field setRoutes)
  accept_remote=$(field acceptRemote)

  [[ $host =~ ^[A-Za-z0-9.:-]+$ ]] || fail "Host inválido."
  [[ -z $port ]] && port=443
  [[ $port =~ ^[0-9]{1,5}$ ]] && (( port > 0 && port < 65536 )) || fail "Porta inválida."
  [[ -n $username ]] || fail "Usuário é obrigatório."
  [[ -z $realm || $realm =~ ^[A-Za-z0-9._-]+$ ]] || fail "Realm inválido."

  local target source existing_password=""
  target=$(conf_path "$name")
  source=$(conf_path "${old:-$name}")
  if [[ -n $old && $old != "$name" ]]; then
    [[ ! -e $target ]] || fail "Já existe uma conexão chamada $name."
    systemctl is-active --quiet "openfortivpn@$old.service" && fail "Desconecte $old antes de renomear."
  fi
  [[ -f $source ]] && existing_password=$(conf_value "$source" password)
  [[ -n $password ]] || password="$existing_password"
  [[ -n $password ]] || fail "Senha é obrigatória."

  local tmp cert
  tmp=$(mktemp "$CONF_DIR/.$name.XXXXXX")
  chmod 600 "$tmp"
  {
    echo "# Managed by the alexandre.fortivpn Omarchy plugin."
    echo "host = $host"
    echo "port = $port"
    echo "username = $username"
    echo "password = $password"
    [[ -n $realm ]] && echo "realm = $realm"
    for cert in ${certs//,/ }; do
      [[ $cert =~ $DIGEST_RE ]] || { rm -f "$tmp"; fail "Certificado inválido: $cert"; }
      echo "trusted-cert = ${cert,,}"
    done
    [[ $set_dns == "false" ]] && echo "set-dns = 0" || echo "set-dns = 1"
    [[ $set_routes == "false" ]] && echo "set-routes = 0" || echo "set-routes = 1"
    [[ $accept_remote == "true" ]] && echo "pppd-accept-remote = 1" || echo "pppd-accept-remote = 0"
  } >"$tmp"
  mv -f "$tmp" "$target"
  restorecon "$target" &>/dev/null || true

  if [[ -n $old && $old != "$name" ]]; then
    rm -f "$source"
    systemctl reset-failed "openfortivpn@$old.service" 2>/dev/null || true
  fi
}

delete_profile() {
  local name="$1"
  valid_name "$name"
  systemctl stop "openfortivpn@$name.service" 2>/dev/null || true
  systemctl reset-failed "openfortivpn@$name.service" 2>/dev/null || true
  rm -f "$(conf_path "$name")"
}

trust_cert() {
  local name="$1" digest="${2,,}" file
  valid_name "$name"
  [[ $digest =~ $DIGEST_RE ]] || fail "Certificado inválido."
  file=$(conf_path "$name")
  [[ -f $file ]] || fail "Conexão não encontrada: $name"
  read_conf "$file" | grep -qxF "trusted-cert	$digest" || echo "trusted-cert = $digest" >>"$file"
  systemctl reset-failed "openfortivpn@$name.service" 2>/dev/null || true
}

case "${1:-}" in
list) list_profiles ;;
save) save_profile "${2:-}" "${3:-}" ;;
delete) delete_profile "${2:-}" ;;
trust) trust_cert "${2:-}" "${3:-}" ;;
*) fail "Usage: ${0##*/} list | save <name> [old-name] | delete <name> | trust <name> <digest>" ;;
esac
