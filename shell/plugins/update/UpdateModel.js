.pragma library

// Shared by the Atualizar DoxIA window (UpdateWindow.qml) and the bar icon
// (bar/widgets/SystemUpdate.qml). The data comes from three JSON files:
//   status  ~/.cache/omarchy/update-status.json       omarchy-update-check
//   run     $XDG_RUNTIME_DIR/omarchy-update/state.json  omarchy-update-state (run in progress)
//   last    ~/.local/state/omarchy/update-last.json    last finished run (or boot finish)

function parse(text) {
  try {
    var v = JSON.parse(String(text || ""))
    return v && typeof v === "object" ? v : null
  } catch (e) {
    return null
  }
}

function systemCount(status) {
  return status && status.system ? status.system.length : 0
}

function flatpakApps(status) {
  return status && status.flatpak && status.flatpak.apps ? status.flatpak.apps : []
}

function flatpakCount(status) {
  return flatpakApps(status).length + (status && status.flatpak ? (status.flatpak.runtimes || 0) : 0)
}

function doxiaBehind(status) {
  return status && status.doxia ? (status.doxia.behind || 0) : 0
}

function stagedPending(status) {
  return !!(status && status.staged && status.staged.pending)
}

function stagedPackages(status) {
  return status && status.staged && status.staged.packages ? status.staged.packages : []
}

function anything(status) {
  return systemCount(status) > 0 || flatpakCount(status) > 0 || doxiaBehind(status) > 0 || stagedPending(status)
}

function onlyFlatpak(status) {
  return flatpakCount(status) > 0 && systemCount(status) === 0 && doxiaBehind(status) === 0
}

function plural(n, one, many) {
  return n + " " + (n === 1 ? one : many)
}

// "flatpak, nautilus, rsync +6": subpackages of the same name collapse.
function systemNames(status, max) {
  var seen = {}
  var names = []
  var list = status && status.system ? status.system : []
  for (var i = 0; i < list.length; i++) {
    var base = list[i].name.replace(/-(libs|selinux|session-helper|extensions|common|devel|data|core|modules|tools)$/, "")
    if (!seen[base]) { seen[base] = true; names.push(base) }
  }
  var shown = names.slice(0, max || 4).join(", ")
  return names.length > (max || 4) ? shown + " +" + (names.length - (max || 4)) : shown
}

function flatpakNames(status, max) {
  var apps = flatpakApps(status).map(function(a) { return a.name })
  var text = apps.slice(0, max || 3).join(", ")
  if (apps.length > (max || 3)) text += " +" + (apps.length - (max || 3))
  var runtimes = status && status.flatpak ? (status.flatpak.runtimes || 0) : 0
  if (runtimes > 0) text += (text ? " · " : "") + plural(runtimes, "runtime", "runtimes")
  return text
}

function updatesCount(status) {
  return systemCount(status) + flatpakCount(status) + (doxiaBehind(status) > 0 ? 1 : 0)
}

function clock(epoch) {
  if (!epoch) return ""
  var d = new Date(epoch * 1000)
  return String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0")
}

function ago(epoch) {
  if (!epoch) return "nunca"
  var s = Math.max(0, Math.floor(Date.now() / 1000 - epoch))
  if (s < 60) return "agora"
  if (s < 3600) return "há " + Math.floor(s / 60) + " min"
  if (s < 86400) return "há " + Math.floor(s / 3600) + " h"
  return "há " + Math.floor(s / 86400) + " d"
}

function duration(seconds) {
  seconds = Math.max(0, Math.round(seconds || 0))
  if (seconds < 60) return seconds + " s"
  var m = Math.floor(seconds / 60)
  return m + " min" + (seconds % 60 ? " " + (seconds % 60) + " s" : "")
}

function currentStep(run) {
  if (!run || !run.steps) return null
  for (var i = 0; i < run.steps.length; i++)
    if (run.steps[i].id === run.current) return run.steps[i]
  return null
}

function stepLabel(run, id) {
  if (!run || !run.steps) return id
  for (var i = 0; i < run.steps.length; i++)
    if (run.steps[i].id === id) return run.steps[i].label
  return id
}

// A finished run's failure message: omarchy-update records the id of the step
// that failed; the boot finish writes a sentence.
function failureText(last) {
  if (!last) return ""
  var msg = last.message || ""
  if (msg && msg.indexOf(" ") < 0) return "A atualização parou no passo “" + stepLabel(last, msg) + "”."
  return msg || "A atualização parou antes de terminar."
}

// Last lines of the update transcript (script(1) output): drop ANSI codes and
// keep what each carriage return left on screen.
function logLines(text, count) {
  var clean = String(text || "")
    .replace(/\x1b\[[0-9;?]*[ -\/]*[@-~]/g, "")
    .replace(/\x1b[()][0-9A-Za-z]/g, "")
    .replace(/\x1b[=>]/g, "")
  var lines = clean.split("\n").map(function(l) {
    var parts = l.split("\r").filter(function(p) { return p.length > 0 })
    return parts.length ? parts[parts.length - 1] : ""
  }).filter(function(l) { return l.trim().length > 0 && l.indexOf("Script started") !== 0 })
  return lines.slice(-(count || 8))
}
