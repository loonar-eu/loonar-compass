/* Loonar Compass – the published usage statistics (stats/ pages).
   Reads the weekly summary over all installations from the feedback service and shows it as tables.
   Values shared by fewer than 3 installations are already counted as "other" by the service. */
(function () {
  "use strict";
  var API = "https://compass-feedback.loonar-eu.workers.dev/v1/summary";
  var box = document.getElementById("stats");
  if (!box) return;

  var T = {
    en: { loading: "Loading…", error: "The statistics could not be loaded. Try again later.", none: "No summary has been published yet.",
      week: "Week", installs: "Active installations", fresh: "New installations", returning: "Used on 2 or more days", days: "Active days",
      versions: "Application versions", systems: "Windows versions", languages: "Interface languages", setup: "Setup", counters: "Usage",
      value: "Value", count: "Installations", key: "Item", total: "Total", empty: "No data in this week.", raw: "Data (JSON)", other: "other (fewer than 3 installations)" },
    pl: { loading: "Wczytywanie…", error: "Nie udało się wczytać statystyk. Spróbuj później.", none: "Nie opublikowano jeszcze żadnego podsumowania.",
      week: "Tydzień", installs: "Aktywne instalacje", fresh: "Nowe instalacje", returning: "Używane w 2 lub więcej dni", days: "Dni aktywności",
      versions: "Wersje aplikacji", systems: "Wersje Windows", languages: "Języki interfejsu", setup: "Konfiguracja", counters: "Użycie",
      value: "Wartość", count: "Instalacje", key: "Pozycja", total: "Razem", empty: "Brak danych w tym tygodniu.", raw: "Dane (JSON)", other: "inne (mniej niż 3 instalacje)" },
    de: { loading: "Wird geladen…", error: "Die Statistik konnte nicht geladen werden. Bitte später erneut versuchen.", none: "Es wurde noch keine Zusammenfassung veröffentlicht.",
      week: "Woche", installs: "Aktive Installationen", fresh: "Neue Installationen", returning: "An 2 oder mehr Tagen genutzt", days: "Aktive Tage",
      versions: "Anwendungsversionen", systems: "Windows-Versionen", languages: "Sprachen der Oberfläche", setup: "Einrichtung", counters: "Nutzung",
      value: "Wert", count: "Installationen", key: "Eintrag", total: "Summe", empty: "Keine Daten in dieser Woche.", raw: "Daten (JSON)", other: "andere (weniger als 3 Installationen)" },
    fr: { loading: "Chargement…", error: "Impossible de charger les statistiques. Réessayez plus tard.", none: "Aucun résumé n’a encore été publié.",
      week: "Semaine", installs: "Installations actives", fresh: "Nouvelles installations", returning: "Utilisées 2 jours ou plus", days: "Jours d’activité",
      versions: "Versions de l’application", systems: "Versions de Windows", languages: "Langues de l’interface", setup: "Configuration", counters: "Utilisation",
      value: "Valeur", count: "Installations", key: "Élément", total: "Total", empty: "Aucune donnée cette semaine.", raw: "Données (JSON)", other: "autres (moins de 3 installations)" }
  };
  var t = T[(document.documentElement.lang || "en").slice(0, 2)] || T.en;
  var num = function (n) { return Number(n || 0).toLocaleString(document.documentElement.lang || "en"); };

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function table(title, rows, head) {
    var sec = el("section", "stats-sec");
    sec.appendChild(el("h2", "", title));
    if (!rows.length) { sec.appendChild(el("p", "stats-empty", t.empty)); return sec; }
    var wrap = el("div", "stats-table");
    var tb = el("table");
    var tr = el("tr");
    tr.appendChild(el("th", "", head[0]));
    tr.appendChild(el("th", "num", head[1]));
    var thead = el("thead"); thead.appendChild(tr); tb.appendChild(thead);
    var body = el("tbody");
    rows.forEach(function (r) {
      var row = el("tr");
      var k = el("td");
      if (r[0] === "other") k.textContent = t.other; else { var c = el("code", "", r[0]); k.appendChild(c); }
      row.appendChild(k);
      row.appendChild(el("td", "num", num(r[1])));
      body.appendChild(row);
    });
    tb.appendChild(body);
    wrap.appendChild(tb);
    sec.appendChild(wrap);
    return sec;
  }

  var sorted = function (obj) {
    return Object.keys(obj || {}).map(function (k) { return [k, obj[k]]; })
      .filter(function (r) { return typeof r[1] === "number"; })
      .sort(function (a, b) { return b[1] - a[1] || a[0].localeCompare(b[0]); });
  };

  /* Setup and the usage counters are nested one level ("setup.ai_provider" → { anthropic: 5 }); flatten to "group · key". */
  var flat = function (obj) {
    var out = {};
    Object.keys(obj || {}).forEach(function (g) {
      var v = obj[g];
      if (typeof v === "number") out[g] = v;
      else if (v && typeof v === "object") Object.keys(v).forEach(function (k) { if (typeof v[k] === "number") out[g + " · " + k] = v[k]; });
    });
    return out;
  };

  function render(s) {
    box.textContent = "";
    var bar = el("div", "stats-bar");
    var label = el("label", "", t.week + " ");
    var sel = el("select");
    (s.weeks && s.weeks.length ? s.weeks : [s.week]).forEach(function (w) {
      var o = el("option", "", w); o.value = w; if (w === s.week) o.selected = true; sel.appendChild(o);
    });
    sel.addEventListener("change", function () { load(sel.value); });
    label.appendChild(sel);
    bar.appendChild(label);
    bar.appendChild(el("span", "stats-range", s.week + " – " + s.until));
    box.appendChild(bar);

    var kpi = el("div", "stats-kpi");
    [[t.installs, s.installations], [t.fresh, s.new_installations], [t.returning, s.returning], [t.days, s.active_days]].forEach(function (k) {
      var d = el("div", "stats-tile");
      d.appendChild(el("b", "", num(k[1])));
      d.appendChild(el("span", "", k[0]));
      kpi.appendChild(d);
    });
    box.appendChild(kpi);

    box.appendChild(table(t.versions, sorted(s.versions), [t.value, t.count]));
    box.appendChild(table(t.systems, sorted(s.systems), [t.value, t.count]));
    box.appendChild(table(t.languages, sorted(s.languages), [t.value, t.count]));
    box.appendChild(table(t.setup, sorted(flat(s.setup)), [t.value, t.count]));
    box.appendChild(table(t.counters, sorted(flat(s.counters)), [t.key, t.total]));
  }

  function load(week) {
    box.setAttribute("aria-busy", "true");
    if (!box.firstChild) box.appendChild(el("p", "stats-empty", t.loading));
    fetch(API + (week ? "?week=" + encodeURIComponent(week) : ""), { credentials: "omit", referrerPolicy: "no-referrer" })
      .then(function (r) {
        if (r.status === 404) { box.textContent = ""; box.appendChild(el("p", "stats-empty", t.none)); return null; }
        if (!r.ok) throw new Error(String(r.status));
        return r.json();
      })
      .then(function (s) { if (s) render(s); })
      .catch(function () { box.textContent = ""; box.appendChild(el("p", "stats-empty", t.error)); })
      .then(function () { box.removeAttribute("aria-busy"); });
  }

  load(null);
})();
