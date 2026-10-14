echo "Move the weather from the bar's center to the right of the clock"

# fedora/doxia/apply-bar moves it only along with a clock still in the center
# (a new install); bars whose clock already left the center keep it there.
shell_json="$HOME/.config/omarchy/shell.json"

[[ -f $shell_json ]] || exit 0
jq -e '
  (.bar.layout.center // [] | any(.id == "omarchy.weather"))
  and (.bar.layout.right // [] | any(.id == "omarchy.clock"))
' "$shell_json" >/dev/null || exit 0

jq '
  (.bar.layout.center | map(select(.id == "omarchy.weather"))) as $weather
  | .bar.layout.center |= map(select(.id != "omarchy.weather"))
  | .bar.layout.right |= (
    (map(.id) | index("omarchy.clock")) as $c
    | .[:$c + 1] + $weather + .[$c + 1:])
' "$shell_json" >"$shell_json.tmp" && mv "$shell_json.tmp" "$shell_json"
