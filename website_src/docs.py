"""User documentation on the website: /docs/<version>/<lang>/ (user guide) and .../install.html (installation),
plus /docs/latest/ with the newest version. The source is the user guide in docs/*.html (the same file the PDF is
printed from); older versions come from website_src/docs_archive/<version>/. Screenshots are the English ones in
every language; captions are translated in the guides themselves.

Called by build.py: generate(build) writes the pages through build's own head/header/footer and returns the URLs
for the sitemap.
"""
import html as _html
import os
import re
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ARCHIVE = os.path.join(ROOT, "website_src", "docs_archive")
SOURCE = {"pl": "Instrukcja_uzytkownika", "en": "Manual.en", "de": "Manual.de", "fr": "Manual.fr"}
# Chapters that make up the installation page are marked <h2 ... data-page="install">; all the others form the user
# guide. Guides of versions before 0.6 have no marks: there these chapters were 2, 3 and 17.
INSTALL_BEFORE_0_6 = {"s2", "s3", "s17"}
PAGES = ("index.html", "install.html")
# The hub's installation guide for IT (docs/<name>.html and .pdf), a page of its own (hub.html) from 0.9.1; a
# language without its own guide shows the English one.
HUB_SOURCE = {"pl": "Instalacja_huba", "en": "Hub_Installation_Guide.en", "de": "Hub_Installation_Guide.de", "fr": "Hub_Installation_Guide.fr"}
HUB_T = {
    "en": dict(tab="Compass Hub (IT)", title="Compass Hub – installation guide for IT", pdf="Download as PDF (hub guide)",
               desc="Installing Compass Hub {v}, the optional team server of Loonar Compass: Windows service, IIS, company sign-in (OpenID Connect), policies."),
    "pl": dict(tab="Compass Hub (IT)", title="Compass Hub – instrukcja instalacji dla działu IT", pdf="Pobierz jako PDF (instrukcja huba)",
               desc="Instalacja Compass Hub {v}, opcjonalnego serwera zespołu Loonar Compass: usługa Windows, IIS, logowanie kontem firmowym (OpenID Connect), zasady."),
    "de": dict(tab="Compass Hub (IT)", title="Compass Hub – Installationsanleitung für die IT", pdf="Als PDF herunterladen (Hub-Anleitung)",
               desc="Installation von Compass Hub {v}, dem optionalen Team-Server von Loonar Compass: Windows-Dienst, IIS, Firmenanmeldung (OpenID Connect), Richtlinien."),
    "fr": dict(tab="Compass Hub (IT)", title="Compass Hub – guide d’installation pour l’IT", pdf="Télécharger en PDF (guide du hub)",
               desc="Installation de Compass Hub {v}, le serveur d’équipe optionnel de Loonar Compass : service Windows, IIS, connexion d’entreprise (OpenID Connect), stratégies."),
}


def hub_source(lang):
    """The hub guide of a language (or the English one) as (html path, pdf file name)."""
    name = HUB_SOURCE[lang] if os.path.isfile(os.path.join(ROOT, "docs", HUB_SOURCE[lang] + ".html")) else HUB_SOURCE["en"]
    return os.path.join(ROOT, "docs", name + ".html"), name + ".pdf"

T = {
    "en": dict(docs="Documentation", guide="User guide", install="Installation", version="Version", latest="latest",
               contents="On this page", pdf="Download as PDF (whole guide)",
               old="This is the documentation of version {v}. The newest version is {l}:", old_link="go to the newest documentation",
               title_guide="User guide", title_install="Installation guide",
               desc_guide="User guide of Loonar Compass {v}, an AI assistant for Sparx Enterprise Architect.",
               desc_install="Installing, updating and uninstalling Loonar Compass {v}, an AI assistant for Sparx Enterprise Architect.",
               choose="Choose the language of the documentation:", versions="Older versions"),
    "pl": dict(docs="Dokumentacja", guide="Instrukcja użytkownika", install="Instalacja", version="Wersja", latest="najnowsza",
               contents="Na tej stronie", pdf="Pobierz jako PDF (cała instrukcja)",
               old="To dokumentacja wersji {v}. Najnowsza wersja to {l}:", old_link="przejdź do najnowszej dokumentacji",
               title_guide="Instrukcja użytkownika", title_install="Instrukcja instalacji",
               desc_guide="Instrukcja użytkownika Loonar Compass {v}, asystenta AI do Sparx Enterprise Architect.",
               desc_install="Instalacja, aktualizacja i odinstalowanie Loonar Compass {v}, asystenta AI do Sparx Enterprise Architect.",
               choose="Wybierz język dokumentacji:", versions="Starsze wersje"),
    "de": dict(docs="Dokumentation", guide="Benutzerhandbuch", install="Installation", version="Version", latest="aktuell",
               contents="Auf dieser Seite", pdf="Als PDF herunterladen (ganzes Handbuch)",
               old="Dies ist die Dokumentation der Version {v}. Die neueste Version ist {l}:", old_link="zur neuesten Dokumentation",
               title_guide="Benutzerhandbuch", title_install="Installationsanleitung",
               desc_guide="Benutzerhandbuch von Loonar Compass {v}, einem KI-Assistenten für Sparx Enterprise Architect.",
               desc_install="Installation, Aktualisierung und Deinstallation von Loonar Compass {v}, einem KI-Assistenten für Sparx Enterprise Architect.",
               choose="Wählen Sie die Sprache der Dokumentation:", versions="Ältere Versionen"),
    "fr": dict(docs="Documentation", guide="Guide de l'utilisateur", install="Installation", version="Version", latest="la plus récente",
               contents="Sur cette page", pdf="Télécharger en PDF (guide complet)",
               old="Ceci est la documentation de la version {v}. La version la plus récente est la {l} :", old_link="voir la documentation la plus récente",
               title_guide="Guide de l'utilisateur", title_install="Guide d'installation",
               desc_guide="Guide de l'utilisateur de Loonar Compass {v}, un assistant IA pour Sparx Enterprise Architect.",
               desc_install="Installation, mise à jour et désinstallation de Loonar Compass {v}, un assistant IA pour Sparx Enterprise Architect.",
               choose="Choisissez la langue de la documentation :", versions="Versions précédentes"),
}
# Inline colours of the guide that the site's Content-Security-Policy would block (no inline styles): mapped to classes.
COLOURS = {"#d97706": "c-orange", "#1e7a3e": "c-green"}


def versions(current):
    """Versions with documentation, newest first: the current one from docs/ and the archived ones."""
    archived = [v for v in os.listdir(ARCHIVE) if os.path.isdir(os.path.join(ARCHIVE, v))] if os.path.isdir(ARCHIVE) else []
    key = lambda v: [int(p) for p in v.split(".")]
    return [current] + sorted((v for v in archived if v != current), key=key, reverse=True)


def source(ver, current):
    """Folder with the guides, the images and the PDFs of a version."""
    if ver == current:
        return {"html": os.path.join(ROOT, "docs"), "img": os.path.join(ROOT, "docs", "img", "en"),
                "logo": os.path.join(ROOT, "docs", "img", "logo.png"), "pdf": os.path.join(ROOT, "docs")}
    d = os.path.join(ARCHIVE, ver)
    return {"html": d, "img": os.path.join(d, "img"), "logo": os.path.join(d, "img", "logo.png"), "pdf": os.path.join(d, "pdf")}


def sections(path):
    """Chapters of a guide: [(id, title, html)], from the first chapter to the end of the body (cover and the printed
    table of contents are left out)."""
    text = open(path, encoding="utf-8").read()
    start = text.index('<h2 id="s1"')
    body = text[start:text.rindex("</body>")]
    out = []
    for part in re.split(r'(?=<h2 id="s\d+")', body):
        if not part.strip():
            continue
        m = re.match(r'<h2 id="(s\d+)"[^>]*>(.*?)</h2>', part, re.S)
        out.append((m.group(1), re.sub(r"<[^>]+>", "", m.group(2)).strip(), part.strip()))
    return out


def clean(html, here, other_page):
    """Adapts a chapter for the web page: shared English images, classes instead of inline styles, links to chapters
    that are on the other page."""
    html = re.sub(r'src="img/(?:[a-z]{2}/)?', 'src="../img/', html)
    for colour, cls in COLOURS.items():
        html = re.sub(rf'style="color:\s*{colour};?"', f'class="{cls}"', html)
    html = re.sub(r'\sstyle="[^"]*"', "", html)
    html = html.replace(' class="break"', "")
    html = re.sub(r'\sdata-page="[^"]*"', "", html)

    def fix(m):
        target = m.group(1)
        return m.group(0) if target in here else f'href="{other_page}#{target}"'
    return re.sub(r'href="#(s\d+)"', fix, html)


def generate(b):
    """Writes the documentation pages through the build module `b`; returns the sitemap URLs (absolute)."""
    current = b.VER
    vers = versions(current)
    urls = []
    for ver in vers:
        src = source(ver, current)
        for label in ([ver, "latest"] if ver == current else [ver]):
            base = f"docs/{label}/"
            img_out = os.path.join(b.OUT, "docs", label, "img")
            os.makedirs(img_out, exist_ok=True)
            for f in os.listdir(src["img"]):
                if f.endswith(".png"):
                    shutil.copyfile(os.path.join(src["img"], f), os.path.join(img_out, f))
            shutil.copyfile(src["logo"], os.path.join(img_out, "logo.png"))
            for lang in b.LANGS:
                guide = os.path.join(src["html"], SOURCE[lang] + ".html")
                if ver == current and f"Loonar Compass {current} " not in open(guide, encoding="utf-8").read():
                    raise SystemExit(f"ERROR: {SOURCE[lang]}.html does not say version {current} (title, cover, footer)")
                chapters = sections(guide)
                pdf = os.path.join(src["pdf"], SOURCE[lang] + ".pdf")
                has_pdf = os.path.isfile(pdf)
                # "latest" links to the PDF of its version instead of keeping a second copy.
                if has_pdf and label != "latest":
                    os.makedirs(os.path.join(b.OUT, "docs", label, lang), exist_ok=True)
                    shutil.copyfile(pdf, os.path.join(b.OUT, "docs", label, lang, SOURCE[lang] + ".pdf"))
                marked = {c[0] for c in chapters if re.match(r'<h2 [^>]*data-page="install"', c[2])}
                install = marked or INSTALL_BEFORE_0_6
                for page in PAGES:
                    picked = [c for c in chapters if (c[0] in install) == (page == "install.html")]
                    write_page(b, ver, label, lang, page, picked, has_pdf, vers, current, chapters)
                    if label == "latest":
                        urls.append(b.SITE + base + lang + "/" + ("" if page == "index.html" else page))
    # The installation guide of the hub for IT: a page of the current version (and its PDF in docs/<ver>/hub/).
    hub_out = os.path.join(b.OUT, "docs", current, "hub")
    os.makedirs(hub_out, exist_ok=True)
    for lang in b.LANGS:
        guide, pdf_name = hub_source(lang)
        pdf = os.path.join(ROOT, "docs", pdf_name)
        if os.path.isfile(pdf):
            shutil.copyfile(pdf, os.path.join(hub_out, pdf_name))
        chapters = sections(guide)
        for label in (current, "latest"):
            write_page(b, current, label, lang, "hub.html", chapters, os.path.isfile(pdf), [current], current, chapters)
        urls.append(b.SITE + f"docs/latest/{lang}/hub.html")
    index_page(b, vers, current)
    return urls


def write_page(b, ver, label, lang, page, chapters, has_pdf, vers, current, all_chapters):
    d = f"docs/{label}/{lang}/"
    P = b.rel(d, "")
    t, dt = b.texts(lang), T[lang]
    install = page == "install.html"
    hub = page == "hub.html"
    ht = HUB_T[lang]
    here = {c[0] for c in chapters}
    other = "index.html" if install else "install.html"
    body = "\n".join(clean(c[2], here, other) for c in chapters)
    title = ht["title"] if hub else dt["title_install"] if install else dt["title_guide"]
    desc = (ht["desc"] if hub else dt["desc_install"] if install else dt["desc_guide"]).format(v=ver)
    canonical = b.SITE + f"docs/latest/{lang}/" + ("" if page == "index.html" else page) if ver == current else b.SITE + d + ("" if page == "index.html" else page)
    alternates = [(l, b.SITE + f"docs/{label}/{l}/" + ("" if page == "index.html" else page)) for l in b.LANGS]
    links = {l: f"../{l}/" + ("" if page == "index.html" else page) for l in b.LANGS}
    switch = {"P": P, "html": b.lang_switch(t, links, lang)}
    home = b.rel(d, b.PATH[lang])
    # The table of contents lists every chapter, so the numbers run without gaps; chapters on the other page
    # (installation or user guide) link there.
    toc = "".join(
        f'<li><a href="#{cid}">{_html.escape(name)}</a></li>' if cid in here
        else f'<li class="docs-toc-other"><a href="{other}#{cid}">{_html.escape(name)} <span aria-hidden="true">↗</span></a></li>'
        for cid, name, _ in all_chapters)
    tabs = (f'<a href="index.html"{" aria-current=\"page\"" if page == "index.html" else ""}>{dt["guide"]}</a>'
            f'<a href="install.html"{" aria-current=\"page\"" if install else ""}>{dt["install"]}</a>')
    if ver == current:  # the hub guide is a page of the current version only
        tabs += f'<a href="hub.html"{" aria-current=\"page\"" if hub else ""}>{ht["tab"]}</a>'
    if hub:  # one version and no chapters on other pages
        vers = [current]
    ver_links = "".join(
        f'<li><a href="{b.rel(d, f"docs/{v}/{lang}/")}{"" if page == "index.html" else page}"'
        f'{" aria-current=\"page\"" if v == ver else ""}>{v}{" (" + dt["latest"] + ")" if v == current else ""}</a></li>'
        for v in vers)
    pdf_href = f"{SOURCE[lang]}.pdf" if label != "latest" else b.rel(d, f"docs/{ver}/{lang}/") + f"{SOURCE[lang]}.pdf"
    if hub:
        pdf_href = b.rel(d, f"docs/{ver}/hub/") + hub_source(lang)[1]
    pdf = f'<p class="docs-pdf"><a href="{pdf_href}" download>{ht["pdf"] if hub else dt["pdf"]}</a></p>' if has_pdf else ""
    old = ""
    if ver != current:
        newest = b.rel(d, f"docs/latest/{lang}/") + ("" if page == "index.html" else page)
        old = f'<p class="docs-old">{dt["old"].format(v=ver, l=current)} <a href="{newest}">{dt["old_link"]}</a></p>'
    html = b.head(lang, f"{title} {ver} – {b.NAME}", desc, canonical, alternates, P, f"{title} – {b.NAME}", desc,
                  robots=None if ver == current else "noindex") + f'''
<body id="top">
{b.header(t, lang, home, switch, nav=False)}

<main id="main" class="docs-main">
  <div class="wrap docs-grid">
    <aside class="docs-side" aria-label="{dt["docs"]}">
      <p class="docs-kicker">{dt["docs"]} · {b.NAME}</p>
      <nav class="docs-tabs">{tabs}</nav>
      <details class="docs-ver"><summary>{dt["version"]}: {ver}{" (" + dt["latest"] + ")" if ver == current else ""}</summary><ul>{ver_links}</ul></details>
      <p class="docs-toc-h">{dt["contents"]}</p>
      <ol class="docs-toc">{toc}</ol>
      {pdf}
    </aside>
    <article class="docs-body">
      {old}
      <h1>{title}</h1>
      {body}
    </article>
  </div>
</main>

{b.footer(t, lang, P, b.rel(d, b.page_dir("privacy", lang)), b.rel(d, b.page_dir("legal", lang)), switch["html"])}
</body>
</html>
'''
    b.write(d + page, html)


def index_page(b, vers, current):
    """/docs/: the language choice for the newest documentation and the list of older versions."""
    d = "docs/"
    P = b.rel(d, "")
    t = b.texts("en")
    langs = "".join(f'<li><a class="btn{"" if l == "en" else " btn-ghost"}" href="latest/{l}/" hreflang="{l}" lang="{l}">{T[l]["guide"]}</a></li>' for l in b.LANGS)
    older = "".join(f'<li><a href="{v}/en/">{v}</a> · ' + " · ".join(f'<a href="{v}/{l}/" lang="{l}">{l.upper()}</a>' for l in b.LANGS if l != "en") + '</li>'
                    for v in vers if v != current)
    switch = {"P": P, "html": b.lang_switch(t, {l: f"latest/{l}/" for l in b.LANGS}, None)}
    html = b.head("en", f"{T['en']['docs']} – {b.NAME}", T["en"]["desc_guide"].format(v=current), b.SITE + d,
                  [], P, f"{T['en']['docs']} – {b.NAME}", T["en"]["desc_guide"].format(v=current)) + f'''
<body id="top">
{b.header(t, "en", P, switch, nav=False)}

<main id="main" class="doc-main">
  <article class="wrap doc">
    <h1>{T["en"]["docs"]}</h1>
    <p>{" / ".join(T[l]["choose"] for l in b.LANGS)}</p>
    <ul class="docs-langs">{langs}</ul>
    <h2>{" / ".join(dict.fromkeys(T[l]["versions"] for l in b.LANGS))}</h2>
    <ul class="docs-older">{older}</ul>
  </article>
</main>

{b.footer(t, "en", P, "../privacy/", "../legal/", switch["html"])}
</body>
</html>
'''
    b.write(d + "index.html", html)
    # /docs/latest/ without a language: to the language choice.
    b.write(d + "latest/index.html", '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="robots" content="noindex">'
            '<meta http-equiv="refresh" content="0; url=../"><title>' + T["en"]["docs"] + '</title></head>'
            '<body><a href="../">' + T["en"]["docs"] + '</a></body></html>\n')
