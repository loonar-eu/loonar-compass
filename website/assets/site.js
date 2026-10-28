/* Loonar Compass - loonar.eu. No libraries, no external scripts, no cookies.
   1) scroll-reveal animations, 2) remembers the chosen language,
   3) on the first visit to the English root page only, suggests (never forces)
      the visitor's browser language in a small dismissible banner,
   4) anonymous visit statistics with GoatCounter (page views + a few link clicks), sent directly to its
      documented /count endpoint (https://www.goatcounter.com/help/pixel) instead of loading count.js,
   5) the "do not count my visits" switch on the privacy page. */
(function () {
  "use strict";
  var doc = document.documentElement;
  var KEY = "loonar-lang";        // language choice
  var NOCOUNT = "loonar-nocount"; // "1" = this browser opted out of the statistics
  var LANGS = ["en", "pl", "de", "fr"];

  function getItem(k) { try { return window.localStorage.getItem(k); } catch (e) { return null; } }
  function setItem(k, v) { try { window.localStorage.setItem(k, v); return true; } catch (e) { return false; } }
  function removeItem(k) { try { window.localStorage.removeItem(k); } catch (e) { /* storage blocked: ignore */ } }

  /* 1. scroll reveal */
  var items = document.querySelectorAll(".reveal");
  var reduced = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if ("IntersectionObserver" in window && !reduced && items.length) {
    doc.classList.add("rv");
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
    items.forEach(function (el) { io.observe(el); });
  }

  /* 2. remember an explicit language choice */
  document.querySelectorAll("[data-lang-link]").forEach(function (a) {
    a.addEventListener("click", function () { setItem(KEY, a.getAttribute("data-lang-link")); });
  });

  /* 4. statistics. The endpoint comes from build.py (constant GOATCOUNTER); no attribute = statistics off.
     GoatCounter's documentation does not ask sites to honour Do Not Track; set HONOR_DNT to true to skip
     counting when the browser sends Do Not Track or Global Privacy Control (and say so in the privacy policy). */
  var GC = doc.getAttribute("data-goatcounter");
  var HONOR_DNT = false;

  function counting() {
    if (!GC || getItem(NOCOUNT) === "1") return false;
    if (location.protocol === "file:") return false;
    if (/(^localhost$|\.localhost$|\.test$|^127\.|^10\.|^192\.168\.|^172\.(1[6-9]|2\d|3[01])\.|^\[?::1\]?$|^0\.0\.0\.0$)/.test(location.hostname)) return false;
    if (document.visibilityState === "prerender") return false;
    try { if (window.top !== window.self) return false; } catch (e) { return false; } // inside a frame
    if (HONOR_DNT && (navigator.globalPrivacyControl === true || navigator.doNotTrack === "1" || window.doNotTrack === "1")) return false;
    return true;
  }

  // Headless browsers, as in GoatCounter's own count.js (the server filters these hits out).
  function bot() {
    var w = window, d = document;
    if (w.callPhantom || w._phantom || w.phantom) return 150;
    if (w.__nightmare) return 151;
    if (d.__selenium_unwrapped || d.__webdriver_evaluate || d.__driver_evaluate) return 152;
    if (navigator.webdriver) return 153;
    return 0;
  }

  // Fire and forget: sendBeacon does not delay navigation; the image request is the fallback.
  function send(data) {
    var q = [];
    for (var k in data) {
      if (Object.prototype.hasOwnProperty.call(data, k) && data[k] !== "" && data[k] !== null && data[k] !== undefined) {
        q.push(encodeURIComponent(k) + "=" + encodeURIComponent(data[k]));
      }
    }
    q.push("rnd=" + Math.random().toString(36).slice(2, 7));
    var url = GC + "?" + q.join("&");
    try { if (navigator.sendBeacon && navigator.sendBeacon(url)) return; } catch (e) { /* fall back */ }
    var img = new Image();
    img.src = url;
  }

  if (counting()) {
    var fixed = doc.getAttribute("data-gc-path"); // the 404 page counts as "/404", with the missing address as title
    send({
      p: fixed || location.pathname || "/",
      t: fixed ? location.pathname : document.title,
      r: document.referrer,
      s: window.screen ? window.screen.width : "",
      q: location.search,
      b: bot()
    });
  }

  // Click events: data-gc="download|github|support|privacy|legal" set by build.py, plus any other link to
  // github.com ("github") or suppi.pl ("support"). The title is the page on which the link was clicked.
  function eventName(a) {
    var n = a.getAttribute("data-gc");
    if (n) return n;
    var h = (a.hostname || "").toLowerCase();
    if (h === "github.com" || h === "www.github.com") return "github";
    if (h === "suppi.pl" || h === "www.suppi.pl") return "support";
    return "";
  }
  function onClick(ev) {
    if (ev.type === "auxclick" && ev.button !== 1) return;
    var el = ev.target;
    var a = el && el.closest ? el.closest("a[href]") : null;
    if (!a) return;
    var name = eventName(a);
    if (name && counting()) send({ p: name, t: location.pathname, e: "true", b: bot() });
  }
  document.addEventListener("click", onClick, true);
  document.addEventListener("auxclick", onClick, true);

  /* 5. opt-out switch (privacy page; hidden without JavaScript or without local storage) */
  var box = document.querySelector("[data-gc-optout]");
  // the switch needs working local storage: test it with its own key, restored at once
  var storageWorks = function () {
    var stored = getItem(NOCOUNT);
    if (stored !== null) return setItem(NOCOUNT, stored);
    var ok = setItem(NOCOUNT, "0");
    removeItem(NOCOUNT);
    return ok;
  };
  if (box && GC && storageWorks()) {
    var btn = box.querySelector("button");
    var state = box.querySelector(".gc-state");
    var render = function () {
      var off = getItem(NOCOUNT) === "1";
      state.textContent = state.getAttribute(off ? "data-off" : "data-on");
      btn.textContent = btn.getAttribute(off ? "data-on" : "data-off");
    };
    btn.addEventListener("click", function () {
      if (getItem(NOCOUNT) === "1") removeItem(NOCOUNT); else setItem(NOCOUNT, "1");
      render();
    });
    render();
    box.hidden = false;
  }

  /* 3. suggestion banner - root (English) page, first visit, no stored choice */
  if (doc.getAttribute("data-root") !== "1" || getItem(KEY)) return;
  var prefs = (navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || ""]);
  var target = null;
  for (var i = 0; i < prefs.length; i++) {
    var code = String(prefs[i] || "").toLowerCase().slice(0, 2);
    if (LANGS.indexOf(code) !== -1) { target = code; break; }
  }
  if (!target || target === "en") return;

  var MSG = {
    pl: { text: "Ta strona jest dostępna także po polsku.", go: "Przejdź do wersji polskiej", stay: "Zostań przy angielskiej", close: "Zamknij" },
    de: { text: "Diese Seite gibt es auch auf Deutsch.", go: "Zur deutschen Version", stay: "Auf Englisch bleiben", close: "Schließen" },
    fr: { text: "Cette page existe aussi en français.", go: "Voir la version française", stay: "Rester en anglais", close: "Fermer" }
  };
  var m = MSG[target];
  var banner = document.createElement("div");
  banner.className = "lang-banner";
  banner.setAttribute("role", "region");
  banner.setAttribute("lang", target);
  banner.setAttribute("aria-label", m.text);

  var p = document.createElement("p"); p.textContent = m.text;
  var row = document.createElement("div"); row.className = "row";
  var go = document.createElement("a"); go.className = "btn"; go.href = target + "/"; go.textContent = m.go;
  var stay = document.createElement("a"); stay.className = "btn btn-ghost"; stay.href = "#"; stay.textContent = m.stay;
  var x = document.createElement("button"); x.type = "button"; x.className = "x"; x.setAttribute("aria-label", m.close); x.textContent = "×";

  function dismiss(ev) { if (ev) ev.preventDefault(); setItem(KEY, "en"); banner.remove(); }
  go.addEventListener("click", function () { setItem(KEY, target); });
  stay.addEventListener("click", dismiss);
  x.addEventListener("click", dismiss);
  document.addEventListener("keydown", function (ev) { if (ev.key === "Escape" && banner.isConnected) dismiss(); });

  row.appendChild(go); row.appendChild(stay);
  banner.appendChild(p); banner.appendChild(row); banner.appendChild(x);
  document.body.appendChild(banner);
})();
