# -*- coding: utf-8 -*-
"""Generator of the loonar.eu website (see website/README.md).

    python -I -B website_src\\build.py   (from the repository root)

Writes into website/: the home page in four languages, the privacy policy and the legal notice per language,
404.html, robots.txt, sitemap.xml, CNAME, .nojekyll and copies of the licence; then checks the generated HTML.
"""
import os, re, json, sys, hashlib, shutil, posixpath
from html.parser import HTMLParser
from urllib.parse import urlsplit
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from content import T, ICONS, MARKS, FEATURE_ICONS, SEC_ICONS, FAQ_IDS
import legal

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "website")
DOMAIN = "loonar.eu"
SITE = f"https://{DOMAIN}/"
LANGS = ["en", "pl", "de", "fr"]
PATH = {"en": "", "pl": "pl/", "de": "de/", "fr": "fr/"}
NAME = "Loonar Compass"
OWNER = "Loonar Mariusz Głowacki"
EMAIL = "mariusz@loonar.eu"

# ---- current release: change these lines for a new version (see website/README.md) ----
VER = "0.3"
# The only link to the installer: a GitHub release asset. The file name and the repository are taken from it.
SETUP_URL = "https://github.com/loonar-eu/loonar-compass/releases/download/v0.3.0/LoonarCompass-Setup-0.3.exe"
RELEASE = "2026-10-17"
DATE = {"en": "17 October 2026", "pl": "17 października 2026", "de": "17. Oktober 2026", "fr": "17 octobre 2026"}
# SHA-256 of the installer, shown next to the download button (PowerShell: Get-FileHash <installer>).
# When the installer is in DIST, the build checks this value against the file and takes the size from it;
# without the file it uses SHA256 and SIZE_MB as written here and prints a warning.
SHA256 = "b55e2061ca8069fc5f8f8e7e99d2b3b3406e48874840d5240f9358d89b37ccc0"
SIZE_MB = 28
DIST = os.path.join(ROOT, "dist")
# ---- services used by the website --------------------------------------------------------
GOATCOUNTER = "loonar"   # GoatCounter site code (https://loonar.goatcounter.com); "" switches the statistics off
SUPPORT_URL = "https://suppi.pl/loonar"
LINKEDIN_URL = "https://pl.linkedin.com/in/glowackimariusz"
# -----------------------------------------------------------------------------------------
ASSET_V = "3"  # raise after every change of assets/style.css or assets/site.js (cache busting)

SETUP = SETUP_URL.rsplit("/", 1)[1]
REPO = SETUP_URL.split("/releases/")[0]
RELEASES_URL = REPO + "/releases"
GC_ENDPOINT = f"https://{GOATCOUNTER}.goatcounter.com/count" if GOATCOUNTER else ""
GH_PRIVACY = "https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement"
GH_PAGES_DOC = "https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages"
GC_PRIVACY = "https://www.goatcounter.com/help/privacy"
PORTRAIT = "assets/img/author.jpg"   # optional; without the file the About section shows a neutral placeholder
LICENSES = {"pl": "LICENSE.txt", "en": "LICENSE.en.txt", "de": "LICENSE.de.txt", "fr": "LICENSE.fr.txt"}
OLD_NAMES = ("EA Assistant", "Loonar-ea-assistant", "Sextant")

MAINNAV = {"en": "Main", "pl": "Menu główne", "de": "Hauptmenü", "fr": "Menu principal"}
FOOTNAV = {"en": "Legal information", "pl": "Informacje prawne", "de": "Rechtliches", "fr": "Informations légales"}
NAV_IDS = ["features", "how", "security", "requirements", "faq", "support"]
WARNINGS = []


def installer_size():
    """Size of the installer in MB. Stops the build when SHA256 does not match the file in DIST."""
    path = os.path.join(DIST, SETUP)
    if not os.path.isfile(path):
        WARNINGS.append(f"{path} not found: the page shows SIZE_MB = {SIZE_MB} and the SHA256 from build.py, "
                        "unchecked. Build the installer and run build.py again before publishing.")
        return SIZE_MB
    with open(path, "rb") as f:
        actual = hashlib.sha256(f.read()).hexdigest()
    if actual != SHA256.lower():
        sys.exit(f"SHA256 in build.py does not match {path}:\n  build.py: {SHA256}\n  file:     {actual}\n"
                 "Upload the installer you mean to publish and put its checksum into SHA256.")
    return round(os.path.getsize(path) / 1048576)


SIZE = installer_size()


def fill(s, lang, P, absolute=False):
    """Placeholders in texts: {ver}, {setup}, {size}, {date}, {repo}, {releases}, {lic} (licence link)."""
    lic = (SITE if absolute else P) + f"assets/license-{lang}.txt"
    return (s.replace("{ver}", VER).replace("{setup}", SETUP).replace("{size}", str(SIZE))
             .replace("{date}", f'<time datetime="{RELEASE}">{DATE[lang]}</time>')
             .replace("{repo}", REPO).replace("{releases}", RELEASES_URL).replace("{lic}", lic))


def ld_json(obj):
    # "</" escaped so that no text can ever close the <script> data block
    return json.dumps(obj, ensure_ascii=False).replace("</", "<\\/")


def ld_answer(html, lang):
    # Answer HTML for FAQPage: absolute links, <code> reduced to plain text (not in Google's allowed tag list)
    return re.sub(r'</?code>', '', fill(html, lang, "", absolute=True))


def nbsp_fr(s):
    # French typography: narrow no-break space before : ; ? ! and inside guillemets (text only)
    parts = re.split(r'(<[^>]+>)', s)
    out = []
    for p in parts:
        if p.startswith('<'):
            out.append(p); continue
        p = re.sub(r' ([:;?!»])', '\u202f\\1', p)
        p = p.replace('« ', '«\u202f')
        out.append(p)
    return ''.join(out)


def fr_fix(obj):
    if isinstance(obj, str): return nbsp_fr(obj)
    if isinstance(obj, list): return [fr_fix(x) for x in obj]
    if isinstance(obj, tuple): return tuple(fr_fix(x) for x in obj)
    if isinstance(obj, dict): return {k: (v if k in ("locale","title","desc","og_desc","hero_alt") else fr_fix(v)) for k, v in obj.items()}
    return obj


def texts(lang):
    return fr_fix(T[lang]) if lang == "fr" else T[lang]


def icon(name, cls="ico"):
    return f'<svg class="{cls}" viewBox="0 0 24 24" aria-hidden="true" focusable="false">{ICONS[name]}</svg>'


def mark(name, cls="mark"):
    vb, body = MARKS[name]
    return f'<svg class="{cls}" viewBox="{vb}" aria-hidden="true" focusable="false">{body}</svg>'


def esc_attr(s):
    return s.replace('&', '&amp;').replace('"', '&quot;').replace('<', '&lt;')


def rel(frm, to):
    """Relative URL from the page folder `frm` to the folder `to` (both like '', 'pl/' or 'pl/privacy/')."""
    r = posixpath.relpath("/" + to, "/" + frm)
    return "./" if r == "." else r.rstrip("/") + "/"


def page_dir(kind, lang):
    return PATH[lang] + ("" if kind == "home" else kind + "/")


def csp():
    """Content-Security-Policy for the <meta> tag (GitHub Pages sends no custom headers; frame-ancestors
    cannot be set in a meta tag). Only the site itself, data: images (CSS ticks) and the GoatCounter endpoint."""
    img = "'self' data:" + (" " + GC_ENDPOINT if GC_ENDPOINT else "")
    connect = GC_ENDPOINT or "'none'"
    return (f"default-src 'self'; img-src {img}; connect-src {connect}; style-src 'self'; script-src 'self'; "
            "font-src 'self'; object-src 'none'; base-uri 'self'; form-action 'none'")

# ---------------------------------------------------------------------------- mock-ups (home page)

def svg_lines(lines, x, y, lh=12, cls="dg-t"):
    n = len(lines); y0 = y - (n - 1) * lh / 2
    return f'<text class="{cls}" x="{x}" y="{y0:.0f}" text-anchor="middle" dominant-baseline="middle">' + ''.join(
        f'<tspan x="{x}" y="{y0 + i*lh:.0f}">{l}</tspan>' for i, l in enumerate(lines)) + '</text>'


def diagram(t):
    s = []
    s.append('<svg class="dg" viewBox="0 0 630 240" aria-hidden="true" focusable="false">')
    s.append('<defs><marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="dg-ah"/></marker></defs>')
    # start event
    s.append('<circle class="dg-ev" cx="22" cy="95" r="12"/>')
    s.append('<path class="dg-fl" d="M34 95H48" marker-end="url(#ah)"/>')
    s.append('<rect class="dg-task" x="48" y="72" width="100" height="46" rx="8"/>')
    s.append(svg_lines(t["d_recv"], 98, 95))
    s.append('<path class="dg-fl" d="M148 95H162" marker-end="url(#ah)"/>')
    s.append('<path class="dg-gw" d="M180 77L198 95L180 113L162 95z"/>')
    s.append('<path class="dg-x" d="M174 89l12 12M186 89l-12 12"/>')
    s.append(f'<text class="dg-s" x="180" y="66" text-anchor="middle">{t["d_gw"]}</text>')
    s.append('<path class="dg-fl" d="M198 95H214" marker-end="url(#ah)"/>')
    s.append(f'<text class="dg-s" x="206" y="88" text-anchor="middle">{t["d_yes"]}</text>')
    # subprocess
    s.append('<rect class="dg-sub" x="214" y="36" width="240" height="118" rx="10"/>')
    s.append(f'<text class="dg-h" x="226" y="54">{t["d_pay"]}</text>')
    xs = [224, 300, 376]
    for i, x in enumerate(xs):
        s.append(f'<rect class="dg-task sm" x="{x}" y="74" width="66" height="44" rx="7"/>')
        s.append(svg_lines(t["dg_sub"][i], x + 33, 96, 11, "dg-t sm"))
        if i < 2:
            s.append(f'<path class="dg-fl" d="M{x+66} 96H{x+76}" marker-end="url(#ah)"/>')
    s.append('<path class="dg-fl" d="M454 95H474" marker-end="url(#ah)"/>')
    s.append('<rect class="dg-task" x="474" y="72" width="104" height="46" rx="8"/>')
    s.append(svg_lines(t["d_ship"], 526, 95))
    s.append('<path class="dg-fl" d="M578 95H592" marker-end="url(#ah)"/>')
    s.append('<circle class="dg-ev end" cx="605" cy="95" r="12"/>')
    # no branch
    s.append('<path class="dg-fl" d="M180 113V196H232" marker-end="url(#ah)"/>')
    s.append(f'<text class="dg-s" x="168" y="150" text-anchor="end">{t["d_no"]}</text>')
    s.append('<rect class="dg-task" x="232" y="174" width="112" height="44" rx="8"/>')
    s.append(svg_lines(t["d_notify"], 288, 196))
    s.append('<path class="dg-fl" d="M344 196H362" marker-end="url(#ah)"/>')
    s.append('<circle class="dg-ev end" cx="375" cy="196" r="12"/>')
    s.append('</svg>')
    return ''.join(s)


RAIL = ["ask", "memory", "doc", "check", "local"]


def app_mock(t, P):
    convs = ''.join(f'<li{" class=\"on\"" if i == 0 else ""}>{c}</li>' for i, c in enumerate(t["m_convs"]))
    steps = ''.join(f'<li><span class="ok" aria-hidden="true">✓</span>{s}<span class="tm">{d}</span></li>'
                    for s, d in zip(t["m_steps"], ["4 s", "0 s", "6 s"]))
    rail = ''.join(f'<span class="am-ri{" on" if i == 0 else ""}">{icon(n)}</span>' for i, n in enumerate(RAIL))
    return f'''<div class="app-mock" role="img" aria-label="{esc_attr(t["hero_alt"])}">
  <div class="am-bar"><img src="{P}assets/img/logo.svg" alt="" width="14" height="14"><span>{NAME}</span><i></i><i></i><i></i></div>
  <div class="am-body">
    <div class="am-rail"><img src="{P}assets/img/logo-white.svg" alt="" width="22" height="22">{rail}</div>
    <div class="am-list"><div class="am-new">+</div><div class="am-grp">{t["m_today"]}</div><ul>{convs}</ul></div>
    <div class="am-chat">
      <div class="am-head">{t["m_convs"][0]}</div>
      <div class="am-msg user">{t["m_user"]}</div>
      <div class="am-steps"><div class="am-sh">{t["m_steps_h"]}</div><ul>{steps}</ul></div>
      <div class="am-msg bot"><img src="{P}assets/img/logo.svg" alt="" width="20" height="20"><p>{t["m_bot"]}</p></div>
      <div class="am-comp"><div class="am-ph">{t["m_ph"]}</div><div class="am-cf"><span>{t["m_mode"]}</span><b aria-hidden="true">↑</b></div></div>
    </div>
  </div>
</div>'''


def approval_mock(t):
    items = ''.join(f'<li>{x}</li>' for x in t["m_appr"])
    return f'''<div class="mock-card appr" aria-hidden="true">
  <div class="mc-h"><span class="dot"></span>{t["m_appr_h"]}</div>
  <ul class="appr-l">{items}</ul>
  <div class="appr-b"><span class="b pri">{t["m_approve"]}</span><span class="b">{t["m_reject"]}</span></div>
</div>'''


def diagram_mock(t):
    return f'''<div class="mock-card dgm" aria-hidden="true">
  <div class="dg-wrap">{diagram(t)}</div>
  <div class="dg-b"><span class="b">{t["m_saveas"]}</span><span class="b">{t["m_full"]}</span><span class="fn">{t["m_png"]}</span></div>
</div>'''


def log_mock(t):
    rows = ''.join(
        f'<tr><td>{a}</td><td><span class="res{" err" if r != "OK" else ""}">{r}</span></td><td>assistant</td><td>{c}</td></tr>'
        for a, r, c in t["m_log"])
    cols = ''.join(f'<th>{c}</th>' for c in t["m_log_cols"])
    return f'''<div class="mock-card log" aria-hidden="true">
  <div class="mc-h row"><span>{t["m_log_h"]}</span><span class="b">{t["m_log_btn"]}</span></div>
  <div class="log-sc"><table><thead><tr>{cols}</tr></thead><tbody>{rows}</tbody></table></div>
</div>'''


def portrait(P):
    if os.path.isfile(os.path.join(OUT, *PORTRAIT.split("/"))):
        return f'<img class="portrait" src="{P}{PORTRAIT}" alt="Mariusz Głowacki" width="168" height="168">'
    # neutral placeholder until website/assets/img/author.jpg exists
    return ('<div class="portrait ph" aria-hidden="true"><svg viewBox="0 0 64 64" focusable="false">'
            '<circle cx="32" cy="24" r="12"/><path d="M10 58c2-12 11-19 22-19s20 7 22 19z"/></svg></div>')

# ---------------------------------------------------------------------------- shared page parts

def head(lang, title, desc, canonical, alternates, P, og_title, og_desc, extra="", robots=None, gc_path=None, root=False):
    alts = ''.join(f'<link rel="alternate" hreflang="{l}" href="{u}">\n' for l, u in alternates)
    oglocs = ''.join(f'<meta property="og:locale:alternate" content="{T[l]["locale"]}">\n' for l in LANGS if l != lang)
    attrs = ' data-root="1"' if root else ''
    if GC_ENDPOINT:
        attrs += f' data-goatcounter="{GC_ENDPOINT}"'
    if gc_path:
        attrs += f' data-gc-path="{gc_path}"'
    canon = f'<link rel="canonical" href="{canonical}">\n' if canonical else ''
    rob = f'<meta name="robots" content="{robots}">\n' if robots else ''
    og_url = f'<meta property="og:url" content="{canonical}">\n' if canonical else ''
    return f'''<!doctype html>
<html lang="{lang}"{attrs}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="{csp()}">
<meta name="referrer" content="strict-origin-when-cross-origin">
<title>{title}</title>
<meta name="description" content="{esc_attr(desc)}">
{rob}{canon}{alts}<meta name="theme-color" content="#0E4A6B">
<meta name="color-scheme" content="light">
<meta property="og:type" content="website">
<meta property="og:site_name" content="{NAME}">
<meta property="og:title" content="{esc_attr(og_title)}">
<meta property="og:description" content="{esc_attr(og_desc)}">
{og_url}<meta property="og:image" content="{SITE}assets/img/og-image.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="{NAME} – {esc_attr(T[lang]["title"].split(" – ", 1)[1])}">
<meta property="og:locale" content="{T[lang]["locale"]}">
{oglocs}<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="{P}favicon.svg" type="image/svg+xml">
<link rel="icon" href="{P}favicon-32.png" type="image/png" sizes="32x32">
<link rel="apple-touch-icon" href="{P}apple-touch-icon.png">
<link rel="stylesheet" href="{P}assets/style.css?v={ASSET_V}">
<script src="{P}assets/site.js?v={ASSET_V}" defer></script>
{extra}</head>'''


def lang_switch(t, links, current):
    """links: {lang: href}; current: the language of this page (None: no current page, e.g. 404)."""
    out = []
    for l in LANGS:
        cur = ' aria-current="page"' if l == current else ''
        out.append(f'<li><a href="{links[l]}" hreflang="{l}" lang="{l}" data-lang-link="{l}" title="{T[l]["name_lang"]}"{cur}>{l.upper()}</a></li>')
    return f'<nav class="lang" aria-label="{t["lang_label"]}"><ul>{"".join(out)}</ul></nav>'


def header(t, lang, home, switch, nav=True):
    """home: link to this language's home page ('' on the home page itself, so anchors stay in-page)."""
    brand = home or "#top"
    navhtml = ''
    if nav:
        items = ''.join(f'<li><a href="{home}#{i}">{n}</a></li>' for i, n in zip(NAV_IDS, t["nav"]))
        navhtml = f'\n    <nav class="main-nav" aria-label="{MAINNAV[lang]}"><ul>{items}</ul></nav>'
    return f'''<a class="skip" href="#main">{t["skip"]}</a>
<header class="site-header">
  <div class="wrap hdr{'' if nav else ' nonav'}">
    <a class="brand" href="{brand}"><img src="{switch["P"]}assets/img/logo.svg" alt="" width="34" height="34"><span>Loonar <b>Compass</b></span></a>{navhtml}
    {switch["html"]}
    <a class="hdr-gh" href="{REPO}" aria-label="{esc_attr(t["gh_label"])}" title="{esc_attr(t["gh_label"])}">{mark("github")}</a>
    <a class="btn btn-sm" href="{home}#download">{t["nav_dl"]}</a>
  </div>
</header>'''


def footer(t, lang, P, privacy, legal_href, switch_html, top=True):
    back = f'<a class="ftr-top" href="#top">{t["f_top"]} ↑</a>' if top else ''
    return f'''<footer class="site-footer">
  <div class="wrap ftr">
    <div class="ftr-brand"><img src="{P}assets/img/logo.svg" alt="" width="40" height="40"><div><strong>{NAME}</strong><br>© 2026 {OWNER}<br>{t["f_addr"]}<br><a href="mailto:{EMAIL}">{EMAIL}</a></div></div>
    <div class="ftr-txt"><p>{t["f_tm"]}</p><p>{t["f_stats"]}</p></div>
    {switch_html.replace('class="lang"', 'class="lang ftr-lang"', 1)}
  </div>
  <div class="wrap ftr-bottom">
    <nav aria-label="{FOOTNAV[lang]}"><ul class="ftr-links">
      <li><a href="{privacy}" data-gc="privacy">{t["f_privacy_link"]}</a></li>
      <li><a href="{legal_href}" data-gc="legal">{t["f_legal_link"]}</a></li>
      <li><a class="ftr-gh" href="{REPO}">{mark("github")}{t["gh_label"]}</a></li>
    </ul></nav>
    {back}
  </div>
</footer>'''


def write(rel_path, text):
    path = os.path.join(OUT, *rel_path.split("/"))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print("wrote", rel_path, len(text))


def common(kind, lang):
    """Prefix, links and language switch for a page of `kind` ('home', 'privacy', 'legal') in `lang`."""
    d = page_dir(kind, lang)
    P = "" if d == "" else rel(d, "")
    links = {l: rel(d, page_dir(kind, l)) for l in LANGS}
    t = texts(lang)
    return d, P, t, {"P": P, "html": lang_switch(t, links, lang)}

# ---------------------------------------------------------------------------- home page

def home_page(lang):
    d, P, t, sw = common("home", lang)
    url = SITE + d
    assert len(NAV_IDS) == len(t["nav"]), lang
    feats = ''.join(f'<li class="card reveal">{icon(ic)}<h3>{h}</h3><p>{p}</p></li>'
                    for (h, p), ic in zip(t["features"], FEATURE_ICONS))
    steps = ''.join(f'<li class="step reveal"><span class="num" aria-hidden="true">{i+1}</span><h3>{h}</h3><p>{p}</p></li>'
                    for i, (h, p) in enumerate(t["steps"]))
    assert len(t["sec"]) == len(SEC_ICONS), lang
    sec = ''.join(f'<li class="sec-item reveal">{icon(ic)}<div><h3>{h}</h3><p>{p}</p></div></li>'
                  for (h, p), ic in zip(t["sec"], SEC_ICONS))
    req = ''.join(f'<li>{x}</li>' for x in t["req"])
    opt = ''.join(f'<li>{x}</li>' for x in t["opt"])
    assert len(t["faq"]) == len(FAQ_IDS), lang
    # Native <details>/<summary>: keyboard accessible and working without JavaScript.
    faq = ''.join(
        f'<details class="qa" id="{fid}"><summary><span class="q">{q}</span><span class="qa-ico" aria-hidden="true"></span></summary>'
        f'<div class="a">{fill(a, lang, P)}</div></details>'
        for fid, (q, a) in zip(FAQ_IDS, t["faq"]))
    ld = {
        "@context": "https://schema.org", "@type": "SoftwareApplication",
        "name": NAME, "softwareVersion": VER, "datePublished": RELEASE,
        "operatingSystem": "Windows 10, Windows 11", "applicationCategory": "BusinessApplication",
        "description": T[lang]["desc"], "url": url, "inLanguage": lang,
        "downloadUrl": SETUP_URL, "fileSize": f"{SIZE}MB",
        "license": SITE + f"assets/license-{lang}.txt",
        "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
        "author": {"@type": "Organization", "name": OWNER, "email": EMAIL, "url": SITE},
    }
    # FAQPage: the same questions and answers as the visible FAQ section (required for structured data).
    faq_ld = {
        "@context": "https://schema.org", "@type": "FAQPage", "url": url, "inLanguage": lang,
        "mainEntity": [{"@type": "Question", "name": re.sub(r'<[^>]+>', '', q),
                        "acceptedAnswer": {"@type": "Answer", "text": ld_answer(a, lang)}}
                       for q, a in t["faq"]],
    }
    extra = (f'<script type="application/ld+json">{ld_json(ld)}</script>\n'
             f'<script type="application/ld+json">{ld_json(faq_ld)}</script>\n')
    alternates = [(l, SITE + page_dir("home", l)) for l in LANGS] + [("x-default", SITE)]
    html = head(lang, t["title"], T[lang]["desc"], url, alternates, P, T[lang]["title"], T[lang]["og_desc"],
                extra=extra, root=(lang == "en")) + f'''
<body id="top">
{header(t, lang, "", sw)}

<main id="main">
<section class="hero">
  <div class="wrap hero-grid">
    <div class="hero-txt">
      <p class="eyebrow"><img src="{P}assets/img/logo-navy.svg" alt="" width="22" height="22">{t["eyebrow"]}</p>
      <h1>{t["h1"]}</h1>
      <p class="lead">{t["lead"]}</p>
      <div class="cta">
        <a class="btn" href="#download">{icon("dl","ico-btn")}{t["cta1"]}</a>
        <a class="btn btn-ghost" href="#features">{t["cta2"]}</a>
      </div>
      <p class="hero-meta">{fill(t["meta"], lang, P)}</p>
    </div>
    <div class="hero-vis">{app_mock(t, P)}</div>
  </div>
</section>

<section id="features" class="sec">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["f_h"]}</h2><p>{t["f_sub"]}</p></header>
    <ul class="grid">{feats}</ul>
  </div>
</section>

<section id="how" class="sec alt">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["h_h"]}</h2><p>{t["h_sub"]}</p></header>
    <ol class="steps">{steps}</ol>
  </div>
</section>

<section id="screens" class="sec">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["s_h"]}</h2><p>{t["s_sub"]}</p></header>
    <div class="shots">
      <figure class="shot reveal">{approval_mock(t)}<figcaption>{t["caps"][0]}</figcaption></figure>
      <figure class="shot wide reveal">{diagram_mock(t)}<figcaption>{t["caps"][1]}</figcaption></figure>
      <figure class="shot full reveal">{log_mock(t)}<figcaption>{t["caps"][2]}</figcaption></figure>
    </div>
  </div>
</section>

<section id="security" class="sec navy">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["sec_h"]}</h2><p>{t["sec_sub"]}</p></header>
    <ul class="sec-list">{sec}</ul>
  </div>
</section>

<section id="requirements" class="sec">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["r_h"]}</h2></header>
    <div class="req reveal">
      <div class="req-col"><h3>{t["r_need"]}</h3><ul class="ticks">{req}</ul></div>
      <div class="req-col opt"><h3>{t["r_opt"]}</h3><ul class="ticks">{opt}</ul><p class="note">{t["r_note"]}</p></div>
    </div>
  </div>
</section>

<section id="faq" class="sec alt">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["faq_h"]}</h2><p>{t["faq_sub"]}</p></header>
    <div class="faq reveal">{faq}</div>
  </div>
</section>

<section id="download" class="sec">
  <div class="wrap">
    <header class="sec-h center reveal"><h2>{t["d_h"]}</h2><p>{t["d_sub"]}</p></header>
    <div class="dl">
      <div class="dl-card reveal">
        <img src="{P}assets/img/logo.svg" alt="" width="64" height="64">
        <h3>{NAME} {VER}</h3>
        <p class="dl-meta">{fill(t["d_meta"], lang, P)}</p>
        <!-- DOWNLOAD LINK: the only link to the installer; generated from SETUP_URL in website_src/build.py (see README.md) -->
        <a class="btn btn-lg" id="download-link" href="{SETUP_URL}" data-gc="download">{icon("dl","ico-btn")}{t["d_btn"]}</a>
        <p class="hash">{t["d_hash"]} <code translate="no">{SHA256.lower()}</code></p>
        <p class="small">{t["d_admin"]}</p>
        <p class="lic">{t["d_lic"]} <a href="{P}assets/license-{lang}.txt">{t["d_lic_link"]}</a></p>
        <p class="small">{t["d_lang"]}</p>
        <p class="small">{t["d_update"]}</p>
        <ul class="dl-links">
          <li><a href="{REPO}">{mark("github")}{t["gh_label"]}</a></li>
          <li><a href="{RELEASES_URL}">{t["d_all"]}</a></li>
        </ul>
      </div>
    </div>
  </div>
</section>

<section id="support" class="sec alt support">
  <div class="wrap sup reveal">
    <span class="sup-ico" aria-hidden="true">{icon("heart")}</span>
    <div class="sup-txt"><h2>{t["sup_h"]}</h2><p>{t["sup_text"]}</p><p class="sup-more">{fill(t["sup_more"], lang, P)}</p></div>
    <div class="sup-cta"><a class="btn" href="{SUPPORT_URL}" data-gc="support">{icon("heart","ico-btn")}{t["sup_btn"]}</a></div>
  </div>
</section>

<section id="about" class="sec about">
  <div class="wrap">
    <header class="sec-h reveal"><h2>{t["ab_h"]}</h2></header>
    <div class="ab reveal">
      <div class="ab-person">
        {portrait(P)}
        <p class="ab-name">Mariusz Głowacki</p>
        <p class="ab-role">{t["ab_role"]}</p>
        <a class="ab-li" href="{LINKEDIN_URL}" aria-label="{esc_attr(t["ab_li"])}" title="{esc_attr(t["ab_li"])}">{mark("linkedin")}</a>
      </div>
      <div class="ab-txt">
        <h3>{t["ab_me_h"]}</h3>
        <p>{t["ab_me"]}</p>
        <h3>{t["ab_why_h"]}</h3>
        <p>{t["ab_why"]}</p>
        <h3>{t["ab_name_h"]}</h3>
        <p>{t["ab_name"]}</p>
      </div>
    </div>
  </div>
</section>

<section id="contact" class="sec alt contact">
  <div class="wrap reveal">
    <h2>{t["c_h"]}</h2>
    <p>{t["c_text"]}</p>
  </div>
</section>
</main>

{footer(t, lang, P, rel(d, page_dir("privacy", lang)), rel(d, page_dir("legal", lang)), sw["html"])}
</body>
</html>
'''
    write(d + "index.html", html)

# ---------------------------------------------------------------------------- privacy policy / legal notice

def optout(t):
    if not GC_ENDPOINT:
        return ''
    return (f'<p class="gc-optout" data-gc-optout hidden><span class="gc-state" role="status" '
            f'data-on="{esc_attr(t["gc_state_on"])}" data-off="{esc_attr(t["gc_state_off"])}"></span> '
            f'<button type="button" class="btn btn-ghost btn-sm" data-on="{esc_attr(t["gc_on"])}" '
            f'data-off="{esc_attr(t["gc_off"])}">{t["gc_off"]}</button></p>')


def doc_page(kind, lang):
    d, P, t, sw = common(kind, lang)
    title, desc, body, updated = legal.page(kind, lang, gh_privacy=GH_PRIVACY, gh_pages=GH_PAGES_DOC,
                                            gc_privacy=GC_PRIVACY, optout=optout(t))
    if lang == "fr":
        body, updated = nbsp_fr(body), nbsp_fr(updated)
    home = rel(d, PATH[lang])
    alternates = [(l, SITE + page_dir(kind, l)) for l in LANGS] + [("x-default", SITE + page_dir(kind, "en"))]
    html = head(lang, f"{title} – {NAME}", desc, SITE + d, alternates, P, f"{title} – {NAME}", desc) + f'''
<body id="top">
{header(t, lang, home, sw)}

<main id="main" class="doc-main">
  <article class="wrap doc">
    <h1>{title}</h1>
    <p class="doc-upd">{t["doc_updated"]} <time datetime="{legal.UPDATED_ISO}">{updated}</time></p>
    {body}
    <p class="doc-back"><a href="{home}">← {t["doc_home"]}</a></p>
  </article>
</main>

{footer(t, lang, P, rel(d, page_dir("privacy", lang)), rel(d, page_dir("legal", lang)), sw["html"])}
</body>
</html>
'''
    write(d + "index.html", html)

# ---------------------------------------------------------------------------- 404 page

def not_found():
    """GitHub Pages serves /404.html for every missing address at any depth, so all links are root-relative."""
    t = texts("en")
    blocks = ''.join(
        f'<section class="nf-l" lang="{l}"><h2>{texts(l)["nf_title"]}</h2><p>{texts(l)["nf_text"]}</p>'
        f'<p><a class="btn{" btn-ghost" if l != "en" else ""}" href="/{PATH[l]}">{texts(l)["nf_link"]}</a></p></section>'
        for l in LANGS)
    switch = lang_switch(t, {l: "/" + PATH[l] for l in LANGS}, None)
    html = head("en", f"{t['nf_title']} – {NAME}", T["en"]["desc"], None, [], "/", f"{t['nf_title']} – {NAME}",
                T["en"]["og_desc"], robots="noindex", gc_path="/404") + f'''
<body id="top">
{header(t, "en", "/", {"P": "/", "html": switch}, nav=False)}

<main id="main" class="nf">
  <div class="wrap">
    <h1 class="nf-code">404</h1>
    <div class="nf-grid">{blocks}</div>
  </div>
</main>

{footer(t, "en", "/", "/privacy/", "/legal/", switch, top=False)}
</body>
</html>
'''
    write("404.html", html)

# ---------------------------------------------------------------------------- other files

def sitemap():
    urls = []
    for kind in ("home", "privacy", "legal"):
        for l in LANGS:
            alts = ''.join(f'\n    <xhtml:link rel="alternate" hreflang="{a}" href="{SITE + page_dir(kind, a)}"/>' for a in LANGS)
            alts += f'\n    <xhtml:link rel="alternate" hreflang="x-default" href="{SITE + page_dir(kind, "en")}"/>'
            urls.append(f'  <url>\n    <loc>{SITE + page_dir(kind, l)}</loc>{alts}\n  </url>')
    write("sitemap.xml", '<?xml version="1.0" encoding="UTF-8"?>\n'
          '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n'
          + "\n".join(urls) + "\n</urlset>\n")


def static_files():
    write("robots.txt", f"User-agent: *\nAllow: /\n\nSitemap: {SITE}sitemap.xml\n")
    write("CNAME", DOMAIN + "\n")
    write(".nojekyll", "")
    for lang, src in LICENSES.items():
        path = os.path.join(ROOT, src)
        text = open(path, encoding="utf-8").read()
        old = [n for n in OLD_NAMES if n in text]
        if old:
            WARNINGS.append(f"{src} still names {', '.join(old)} - update the licence, then build again.")
        shutil.copyfile(path, os.path.join(OUT, "assets", f"license-{lang}.txt"))

# ---------------------------------------------------------------------------- checks of the generated pages

VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}
SVG_VOID_OK = {"path", "circle", "rect", "stop", "use", "line", "polyline", "polygon", "ellipse"}


class Checker(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack, self.ids, self.hrefs, self.srcs, self.errors, self.ld = [], [], [], [], [], []
        self._ld = None

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a: self.ids.append(a["id"])
        if tag == "a" and "href" in a: self.hrefs.append(a["href"])
        for k in ("src", "href"):
            if k in a and tag in ("img", "link", "script"): self.srcs.append(a[k])
        if tag == "script" and a.get("type") == "application/ld+json": self._ld = []
        if tag not in VOID: self.stack.append((tag, self.getpos()))

    def handle_startendtag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a: self.ids.append(a["id"])
        if tag not in VOID and tag not in SVG_VOID_OK:
            self.errors.append(f"self-closed <{tag}/> at {self.getpos()}")

    def handle_data(self, data):
        if self._ld is not None: self._ld.append(data)

    def handle_endtag(self, tag):
        if tag == "script" and self._ld is not None:
            self.ld.append("".join(self._ld)); self._ld = None
        if tag in VOID: return
        if not self.stack or self.stack[-1][0] != tag:
            self.errors.append(f"unexpected </{tag}> at {self.getpos()}, open: {self.stack[-1] if self.stack else None}")
            return
        self.stack.pop()


def local_target(page_rel, href):
    """File path in OUT for a relative/root-relative link, or None for external links."""
    u = urlsplit(href)
    if u.scheme or u.netloc or href.startswith(("mailto:", "#")):
        return None
    base = "/" + posixpath.dirname(page_rel) + "/"
    path = posixpath.normpath(posixpath.join("/" if u.path.startswith("/") else base, u.path or "."))
    if u.path.endswith("/") or u.path in ("", ".", "./") or path.endswith("/"):
        path = path.rstrip("/") + "/index.html"
    return path.lstrip("/"), u.fragment


def check():
    pages = {}
    for dirpath, _, files in os.walk(OUT):
        for f in files:
            if f.endswith(".html"):
                rp = os.path.relpath(os.path.join(dirpath, f), OUT).replace(os.sep, "/")
                c = Checker(); c.feed(open(os.path.join(dirpath, f), encoding="utf-8").read()); c.close()
                pages[rp] = c
    problems = []
    for rp, c in pages.items():
        problems += [f"{rp}: {e}" for e in c.errors]
        if c.stack: problems.append(f"{rp}: unclosed {c.stack}")
        dup = {i for i in c.ids if c.ids.count(i) > 1}
        if dup: problems.append(f"{rp}: duplicate ids {sorted(dup)}")
        for block in c.ld:
            try: json.loads(block)
            except ValueError as e: problems.append(f"{rp}: JSON-LD does not parse: {e}")
        for href in c.hrefs + c.srcs:
            if href.startswith("#"):
                if href[1:] and href[1:] not in c.ids: problems.append(f"{rp}: anchor {href} missing")
                continue
            tgt = local_target(rp, href)
            if not tgt: continue
            path, frag = tgt
            if not os.path.isfile(os.path.join(OUT, *path.split("/"))):
                problems.append(f"{rp}: link {href} -> {path} missing")
                continue
            if frag and path in pages and frag not in pages[path].ids:
                problems.append(f"{rp}: link {href} -> #{frag} missing in {path}")
    return problems


if __name__ == "__main__":
    for l in LANGS:
        home_page(l)
        doc_page("privacy", l)
        doc_page("legal", l)
    not_found()
    sitemap()
    static_files()
    stale = os.path.join(OUT, ".htaccess")
    if os.path.exists(stale):
        WARNINGS.append("website/.htaccess exists but GitHub Pages ignores it - delete it.")
    problems = check()
    for w in WARNINGS:
        print("WARNING:", w)
    if problems:
        print("\n".join("ERROR: " + p for p in problems))
        sys.exit(1)
    print("check: all pages OK")
