.pragma library

// Pastas de Programas: a primeira categoria XDG do app que casar, nesta ordem
// (Settings antes de System, que costumam vir juntas). O resto vai pra Outros.
var CATEGORIES = [
  { keys: ["Game"], label: "Jogos", icon: "applications-games" },
  { keys: ["Development"], label: "Desenvolvimento", icon: "applications-development" },
  { keys: ["Office"], label: "Escritório", icon: "applications-office" },
  { keys: ["Graphics"], label: "Gráficos", icon: "applications-graphics" },
  { keys: ["AudioVideo", "Audio", "Video"], label: "Multimídia", icon: "applications-multimedia" },
  { keys: ["Network"], label: "Internet", icon: "applications-internet" },
  { keys: ["Education"], label: "Educação", icon: "applications-education" },
  { keys: ["Science"], label: "Ciência", icon: "applications-science" },
  { keys: ["Settings"], label: "Configurações", icon: "preferences-system" },
  { keys: ["System"], label: "Sistema", icon: "applications-system" },
  { keys: ["Utility"], label: "Acessórios", icon: "applications-utilities" }
]
var OTHER = { label: "Outros", icon: "applications-other" }

function listOf(value) {
  var out = []
  try {
    if (value && value.length !== undefined)
      for (var i = 0; i < value.length; i++) out.push(String(value[i]))
  } catch (e) {
  }
  return out
}

function categoryOf(entry) {
  var cats = listOf(entry && entry.categories)
  for (var i = 0; i < CATEGORIES.length; i++)
    for (var k = 0; k < CATEGORIES[i].keys.length; k++)
      if (cats.indexOf(CATEGORIES[i].keys[k]) !== -1) return CATEGORIES[i]
  return OTHER
}

// [{ label, icon, apps: [entry] }] em ordem alfabética, sem pastas vazias.
function groupByCategory(entries) {
  var groups = ({})
  for (var i = 0; i < entries.length; i++) {
    var cat = categoryOf(entries[i])
    if (!groups[cat.label]) groups[cat.label] = { label: cat.label, icon: cat.icon, apps: [] }
    groups[cat.label].apps.push(entries[i])
  }
  var out = []
  for (var label in groups) out.push(groups[label])
  out.sort(function(a, b) {
    if (a.label === OTHER.label) return 1
    if (b.label === OTHER.label) return -1
    return a.label.localeCompare(b.label, "pt-BR")
  })
  return out
}

function xmlUnescape(value) {
  return String(value || "").replace(/&quot;/g, "\"").replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&")
}

// Arquivos recentes do ~/.local/share/recently-used.xbel (o mesmo que GTK e
// Nautilus usam), mais novos primeiro: [{ href, name, icon }].
function recentDocuments(xml, limit) {
  var out = []
  var re = /<bookmark\b([^>]*)>([\s\S]*?)<\/bookmark>/g
  var match
  while ((match = re.exec(String(xml || ""))) !== null) {
    var attrs = match[1]
    var href = (/\bhref="([^"]*)"/.exec(attrs) || [])[1]
    if (!href || href.indexOf("file://") !== 0) continue
    href = xmlUnescape(href)
    var stamp = (/\bmodified="([^"]*)"/.exec(attrs) || [])[1] || (/\bvisited="([^"]*)"/.exec(attrs) || [])[1] || ""
    var mime = (/mime-type\s+type="([^"]*)"/.exec(match[2]) || [])[1] || ""
    var name = href.slice(href.lastIndexOf("/") + 1)
    try { name = decodeURIComponent(name) } catch (e) {}
    out.push({ href: href, name: name, stamp: stamp, icon: mime ? mime.replace("/", "-") : "text-x-generic" })
  }
  out.sort(function(a, b) { return a.stamp < b.stamp ? 1 : a.stamp > b.stamp ? -1 : 0 })
  return out.slice(0, limit || 15)
}

// Busca simples para quando a biblioteca de apps do shell não está disponível:
// visíveis, por nome; com texto, nome que começa com ele vem primeiro.
function searchEntries(values, query, hidden) {
  var q = String(query || "").trim().toLowerCase()
  var out = []
  for (var i = 0; i < values.length; i++) {
    var entry = values[i]
    if (!entry || entry.noDisplay || !entry.name) continue
    if (hidden && hidden[String(entry.id)] === true) continue
    var name = String(entry.name).toLowerCase()
    var score = 0
    if (q) {
      var haystack = [entry.name, entry.genericName, entry.comment, listOf(entry.keywords).join(" "), entry.id].join(" ").toLowerCase()
      if (name.indexOf(q) === 0) score = 3
      else if (name.indexOf(q) !== -1) score = 2
      else if (haystack.indexOf(q) !== -1) score = 1
      else continue
    }
    out.push({ entry: entry, score: score, name: name })
  }
  out.sort(function(a, b) { return b.score - a.score || a.name.localeCompare(b.name, "pt-BR") })
  return out.map(function(row) { return row.entry })
}

// launcher.hides: um id de .desktop por linha (com ou sem o sufixo).
function hiddenIds(text) {
  var out = ({})
  var lines = String(text || "").split(/\n/)
  for (var i = 0; i < lines.length; i++) {
    var id = lines[i].trim().replace(/\.desktop$/, "")
    if (id) out[id] = true
  }
  return out
}
