"""The product statistics of Loonar Compass (LC-291): reads the D1 database of the feedback service with the author's
Cloudflare API token and writes a self-contained HTML page. Private: nothing here is published.

    python tools/stats/dashboard.py [--out compass_stats.html] [--days 60] [--no-open]

The token: the environment variable CLOUDFLARE_API_TOKEN, or the Windows Credential Manager entry given with
--credential (default cloudflare_api_token.loonar-tools). The account and the database come from feedback/wrangler.toml.
The website part: installer downloads from GitHub Releases (totals now; per day from the snapshots in D1) and, with a
GoatCounter API key (GOATCOUNTER_TOKEN or the entry --goatcounter-credential), visitors, pages, clicks, referrers and
countries.
"""
import argparse
import datetime as dt
import html
import json
import os
import re
import sys
import urllib.request
import webbrowser
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "regression"))

OUTCOMES = ["done", "partial", "rejected", "cannot", "error", "stopped"]
TASK_NAMES = {
    "code": "Source code", "presentation": "Presentation", "document": "Document", "spreadsheet": "Spreadsheet",
    "confluence": "Confluence", "jira": "Jira", "cloud_files": "SharePoint / OneDrive / Google Drive",
    "model_change": "Model change", "requirements": "Requirements", "model_check": "Model check", "impact": "Impact",
    "diagram": "Diagram", "web": "Web", "mcp": "MCP", "model_question": "Model question", "chat": "Conversation only",
}


def wrangler_ids():
    text = (ROOT / "feedback" / "wrangler.toml").read_text(encoding="utf-8")
    account = re.search(r'^account_id\s*=\s*"([^"]+)"', text, re.M).group(1)
    database = re.search(r'database_id\s*=\s*"([^"]+)"', text).group(1)
    return account, database


def token(credential):
    if os.environ.get("CLOUDFLARE_API_TOKEN"):
        return os.environ["CLOUDFLARE_API_TOKEN"]
    from services import secret
    return secret(credential)


def query(auth, account, database, sql, params=()):
    url = f"https://api.cloudflare.com/client/v4/accounts/{account}/d1/database/{database}/query"
    body = json.dumps({"sql": sql, "params": list(params)}).encode()
    req = urllib.request.Request(url, body, {"Authorization": f"Bearer {auth}", "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        answer = json.load(r)
    if not answer.get("success"):
        raise SystemExit(f"D1: {answer.get('errors')}")
    return answer["result"][0]["results"]


# ---------------------------------------------------------------- data

def load(auth, since):
    account, database = wrangler_ids()
    starts = query(auth, account, database, "SELECT install, day, app, os, lang FROM starts WHERE day >= ?1", [since])
    days = query(auth, account, database, "SELECT install, day, app, partial, data FROM days WHERE day >= ?1", [since])
    first = query(auth, account, database, "SELECT install, MIN(day) AS first FROM starts GROUP BY install")
    for d in days:
        d["data"] = json.loads(d["data"])
    return starts, days, {r["install"]: r["first"] for r in first}


def week_of(day):
    d = dt.date.fromisoformat(day)
    return (d - dt.timedelta(days=d.weekday())).isoformat()


def summarize(starts, days, first, today, span):
    active = defaultdict(set)  # day -> installs
    last = {}  # install -> latest start row
    for s in sorted(starts, key=lambda s: s["day"]):
        active[s["day"]].add(s["install"])
        last[s["install"]] = s
    for d in days:
        active[d["day"]].add(d["install"])
        last.setdefault(d["install"], {"app": d["app"], "os": d["data"].get("os", ""), "lang": d["data"].get("lang", "")})

    def installs_between(a, b):
        out = set()
        for day, ids in active.items():
            if a <= day <= b:
                out |= ids
        return out

    t = today.isoformat()
    d7, d30 = (today - dt.timedelta(days=6)).isoformat(), (today - dt.timedelta(days=29)).isoformat()
    recent = [d for d in days if d["day"] >= d30]

    tasks = defaultdict(Counter)
    gaps, ratings, tools, tool_errors, setup = Counter(), Counter(), Counter(), Counter(), defaultdict(Counter)
    turns = 0
    for d in recent:
        c = d["data"]
        turns += c.get("turns", 0)
        for k, n in (c.get("tasks") or {}).items():
            category, _, outcome = k.partition("__")
            tasks[category][outcome] += n
        gaps.update(c.get("gaps") or {})
        ratings.update(c.get("ratings") or {})
        tools.update(c.get("tools") or {})
        tool_errors.update(c.get("tool_errors") or {})
    latest_setup = {}
    for d in sorted(recent, key=lambda d: d["day"]):
        if d["data"].get("setup"):
            latest_setup[d["install"]] = d["data"]["setup"]
    for s in latest_setup.values():
        for k, v in s.items():
            setup[k][("yes" if v else "no") if isinstance(v, (bool, int)) and not isinstance(v, str) else str(v)] += 1

    timeline = []
    for i in range(span - 1, -1, -1):
        day = (today - dt.timedelta(days=i)).isoformat()
        timeline.append((day, len(active.get(day, ())), sum(1 for f in first.values() if f == day)))

    # Weekly cohorts: installations by the week of their first start, and the share active in each later week.
    weeks = sorted({week_of(f) for f in first.values() if f >= (today - dt.timedelta(days=7 * 8)).isoformat()})
    active_weeks = defaultdict(set)
    for day, ids in active.items():
        for i in ids:
            active_weeks[i].add(week_of(day))
    cohorts = []
    for w in weeks:
        members = [i for i, f in first.items() if week_of(f) == w]
        later = []
        for k in range(1, 6):
            wk = (dt.date.fromisoformat(w) + dt.timedelta(weeks=k)).isoformat()
            if wk > week_of(t):
                break
            later.append(sum(1 for i in members if wk in active_weeks[i]))
        cohorts.append((w, len(members), later))

    seven = installs_between(d7, t)
    done = sum(sum(c.values()) for c in tasks.values())
    ok = sum(c["done"] for c in tasks.values())
    up = ratings.get("up", 0)
    rated = sum(ratings.values())
    return {
        "today": t,
        "kpi": [
            ("Active installations, 7 days", len(seven), None),
            ("Active installations, 30 days", len(installs_between(d30, t)), None),
            ("New installations, 7 days", sum(1 for f in first.values() if f >= d7), None),
            ("Installations ever", len(first), None),
            ("Tasks, 30 days", done, f"{ok / done:.0%} ended well" if done else "no tasks yet"),
            ("Answers rated 👍, 30 days", f"{up / rated:.0%}" if rated else "–", f"{rated} rated" if rated else "no ratings yet"),
        ],
        "timeline": timeline,
        "cohorts": cohorts,
        "versions": Counter(last[i]["app"] for i in seven if i in last),
        "languages": Counter(last[i]["lang"] for i in seven if i in last),
        "systems": Counter(re.sub(r"\s*\(.*\)$", "", last[i]["os"]) for i in seven if i in last),
        "tasks": tasks,
        "gaps": gaps,
        "ratings": ratings,
        "tools": tools,
        "tool_errors": tool_errors,
        "setup": setup,
        "turns": turns,
    }


# ---------------------------------------------------------------- website and downloads

def secret_or_none(credential):
    try:
        from services import secret
        return secret(credential)
    except SystemExit:
        return None


def get_json(url, headers=None):
    req = urllib.request.Request(url, headers={"User-Agent": "compass-stats", **(headers or {})})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def kind_of(asset):
    if asset.endswith(".sha256"):
        return None
    return "installer" if asset.lower().endswith(".exe") else "hub" if asset.lower().endswith(".zip") else None


def releases(repo):
    """The download totals of the release files now (GitHub keeps totals only)."""
    out = []
    for r in get_json(f"https://api.github.com/repos/{repo}/releases?per_page=100"):
        c = Counter()
        for a in r.get("assets", []):
            k = kind_of(a["name"])
            if k:
                c[k] += a["download_count"]
        out.append({"tag": r["tag_name"], "published": r["published_at"][:10], **c})
    return out


def download_history(auth):
    """Installer downloads per day: differences of the daily snapshots that the feedback service writes down."""
    account, database = wrangler_ids()
    try:
        rows = query(auth, account, database, "SELECT day, tag, asset, count FROM downloads ORDER BY day")
    except Exception as e:  # the table appears with the first snapshot
        return {}, f"no download history: {e}"
    by_day = defaultdict(dict)
    for r in rows:
        if kind_of(r["asset"]) == "installer":
            by_day[r["day"]][(r["tag"], r["asset"])] = r["count"]
    per_day, previous = {}, None
    for day in sorted(by_day):
        if previous is not None:
            per_day[day] = sum(max(n - previous.get(k, 0), 0) for k, n in by_day[day].items())
        previous = by_day[day]
    note = None if per_day else "downloads per day appear after the second daily snapshot (the service writes one a day)"
    return per_day, note


def goatcounter(site, tok, start, end):
    """Visitors per day, pages, events (clicks), referrers and countries from the GoatCounter API."""
    base = f"https://{site}/api/v0/stats"
    q = f"start={start}T00:00:00Z&end={end}T23:59:59Z"
    head = {"Authorization": f"Bearer {tok}", "Content-Type": "application/json"}
    total = get_json(f"{base}/total?{q}", head)
    hits = get_json(f"{base}/hits?{q}&limit=100", head).get("hits") or []
    refs = get_json(f"{base}/toprefs?{q}&limit=15", head).get("stats") or []
    places = get_json(f"{base}/locations?{q}&limit=15", head).get("stats") or []
    daily = Counter()
    for st in total.get("stats") or []:
        daily[st["day"]] += st.get("daily", 0)
    return {
        "daily": daily,
        "total": total.get("total", 0),
        "events_total": total.get("total_events", 0),
        "pages": Counter({h["path"]: h["count"] for h in hits if not h.get("event")}),
        "events": Counter({h["path"]: h["count"] for h in hits if h.get("event")}),
        "refs": Counter({(r.get("name") or "(direct)"): r["count"] for r in refs}),
        "places": Counter({(r.get("name") or "?"): r["count"] for r in places}),
    }


def web_section(today, span, auth, gc_site, gc_credential, repo):
    """The website and downloads part of the page; every source may fail on its own without stopping the rest."""
    d30 = (today - dt.timedelta(days=29)).isoformat()
    d7 = (today - dt.timedelta(days=6)).isoformat()
    tiles, parts, notes = [], [], []
    try:
        rel = releases(repo)
        installer_total = sum(r.get("installer", 0) for r in rel)
        tiles.append(("Installer downloads, all versions", installer_total, None))
        if rel:
            tiles.append((f"Downloads of {rel[0]['tag']}", rel[0].get("installer", 0), f"published {rel[0]['published']}"))
        by_version = Counter({r["tag"]: r.get("installer", 0) for r in rel})
        hub = Counter({r["tag"]: r.get("hub", 0) for r in rel if r.get("hub")})
    except Exception as e:
        by_version, hub = Counter(), Counter()
        notes.append(f"GitHub releases: {e}")
    per_day, note = download_history(auth)
    if note:
        notes.append(note)
    gc = None
    tok = os.environ.get("GOATCOUNTER_TOKEN") or secret_or_none(gc_credential)
    if tok:
        try:
            gc = goatcounter(gc_site, tok, (today - dt.timedelta(days=span - 1)).isoformat(), today.isoformat())
        except Exception as e:
            notes.append(f"GoatCounter: {e}")
    else:
        notes.append(f"Website visits: create an API key in GoatCounter ({gc_site} → your name at the top → API, "
                     f"permission “Read statistics”) and save it: cmdkey /generic:{gc_credential} /user:token /pass")
    if gc:
        tiles[:0] = [
            ("Website visitors, 7 days", sum(n for d, n in gc["daily"].items() if d >= d7), None),
            ("Website visitors, 30 days", sum(n for d, n in gc["daily"].items() if d >= d30), None),
            ("Download clicks", sum(n for k, n in gc["events"].items() if "download" in k.lower()), f"last {span} days"),
        ]
    rows = [((today - dt.timedelta(days=i)).isoformat(),) for i in range(span - 1, -1, -1)]
    if gc:
        parts.append(daily_bars([(d, gc["daily"].get(d, 0)) for (d,) in rows], 1, f"Website visitors per day, last {span} days", "visitors"))
    if per_day:
        parts.append(daily_bars([(d, per_day.get(d, 0)) for (d,) in rows], 1, f"Installer downloads per day, last {span} days", "downloads"))
    cards = [hbars("Installer downloads by version (all time)", by_version, limit=15)]
    if hub:
        cards.append(hbars("Hub downloads by version (all time)", hub, limit=10))
    if gc:
        cards += [hbars("Pages", gc["pages"]), hbars("Clicks (events)", gc["events"]), hbars("Referrers", gc["refs"]), hbars("Countries", gc["places"])]
    tiles_html = "".join(f'<div class="tile"><div class="k">{esc(k)}</div><div class="v">{esc(v)}</div>'
                         + (f'<div class="sub">{esc(sub)}</div>' if sub else "") + "</div>" for k, v, sub in tiles)
    notes_html = "".join(f'<p class="muted">{esc(n)}</p>' for n in notes)
    return (f'<h2>Website and downloads</h2>{notes_html}<div class="tiles">{tiles_html}</div>'
            + "".join(parts) + f'<div class="cards">{"".join(cards)}</div>')


# ---------------------------------------------------------------- page

def esc(v):
    return html.escape(str(v))


def daily_bars(rows, index, title, label):
    """A column chart of one measure per day; the hover tooltip is the SVG title of each column."""
    w, h, pad = 960, 180, 28
    if not any(r[index] for r in rows):
        return f'<figure><figcaption>{esc(title)}</figcaption><p class="muted">No data yet.</p></figure>'
    peak = max([r[index] for r in rows] + [1])
    step = (w - pad) / len(rows)
    bar = max(step - 2, 1)
    out = [f'<figure><figcaption>{esc(title)}</figcaption><svg viewBox="0 0 {w} {h + 22}" role="img" aria-label="{esc(title)}">']
    for frac in (0.5, 1):
        y = h - frac * (h - 10)
        out.append(f'<line class="grid" x1="{pad}" x2="{w}" y1="{y:.1f}" y2="{y:.1f}"/><text class="axis" x="{pad - 6}" y="{y + 4:.1f}" text-anchor="end">{round(peak * frac)}</text>')
    out.append(f'<line class="base" x1="{pad}" x2="{w}" y1="{h}" y2="{h}"/>')
    for i, r in enumerate(rows):
        v = r[index]
        bh = 0 if not v else max(v / peak * (h - 10), 2)
        x = pad + i * step + 1
        out.append(f'<g class="hit"><rect x="{x - 1:.1f}" y="0" width="{step:.1f}" height="{h}" fill="transparent"/>'
                   f'<path class="mark" d="{column(x, h, bar, bh)}"/><title>{esc(r[0])}: {v} {esc(label)}</title></g>')
        if i % 7 == 0:
            out.append(f'<text class="axis" x="{x + bar / 2:.1f}" y="{h + 16}" text-anchor="middle">{esc(r[0][5:])}</text>')
    out.append("</svg></figure>")
    return "".join(out)


def column(x, base, width, height):
    """A column with 4px rounded top corners, anchored to the baseline."""
    if height <= 0:
        return ""
    r = min(4, width / 2, height)
    top = base - height
    return (f"M{x:.1f},{base} V{top + r:.1f} Q{x:.1f},{top:.1f} {x + r:.1f},{top:.1f} H{x + width - r:.1f} "
            f"Q{x + width:.1f},{top:.1f} {x + width:.1f},{top + r:.1f} V{base} Z")


def hbars(title, counts, names=None, limit=12, note=None):
    """Horizontal bars with the value written at the end (one series: no legend)."""
    if not counts:
        return f'<section class="card"><h3>{esc(title)}</h3><p class="muted">No data yet.</p></section>'
    items = counts.most_common(limit)
    peak = max(n for _, n in items)
    rows = "".join(
        f'<div class="hrow" title="{esc((names or {}).get(k, k))}: {n}"><span class="lab">{esc((names or {}).get(k, k))}</span>'
        f'<span class="track"><span class="fill" style="width:{max(n / peak * 100, 1.5):.1f}%"></span></span><span class="val">{n}</span></div>'
        for k, n in items)
    more = f'<p class="muted">{esc(note)}</p>' if note else ""
    return f'<section class="card"><h3>{esc(title)}</h3>{rows}{more}</section>'


def task_table(tasks):
    if not tasks:
        return '<section class="card wide"><h3>Tasks by kind, 30 days</h3><p class="muted">No tasks yet.</p></section>'
    rows = []
    for cat, c in sorted(tasks.items(), key=lambda kv: -sum(kv[1].values())):
        total = sum(c.values())
        segs = "".join(f'<span class="seg s{j}" style="width:{c[o] / total * 100:.1f}%" title="{o}: {c[o]}"></span>' for j, o in enumerate(OUTCOMES) if c[o])
        rows.append(f'<tr><td>{esc(TASK_NAMES.get(cat, cat))}</td><td class="num">{total}</td>'
                    + "".join(f'<td class="num">{c[o]}</td>' for o in OUTCOMES)
                    + f'<td class="num">{c["done"] / total:.0%}</td><td class="stack"><div class="bar">{segs}</div></td></tr>')
    legend = "".join(f'<span class="key"><i class="sw s{j}"></i>{o}</span>' for j, o in enumerate(OUTCOMES))
    return (f'<section class="card wide"><h3>Tasks by kind, 30 days</h3><div class="legend">{legend}</div><table><thead><tr><th>Kind</th><th class="num">All</th>'
            + "".join(f'<th class="num">{o}</th>' for o in OUTCOMES) + '<th class="num">Ended well</th><th>Share</th></tr></thead><tbody>'
            + "".join(rows) + "</tbody></table></section>")


def tool_table(tools, errors):
    if not tools:
        return ""
    rows = "".join(
        f'<tr><td>{esc(k)}</td><td class="num">{n}</td><td class="num">{errors.get(k, 0)}</td><td class="num">{errors.get(k, 0) / n:.0%}</td></tr>'
        for k, n in sorted(tools.items(), key=lambda kv: (-(errors.get(kv[0], 0) / kv[1]), -kv[1]))[:20])
    return (f'<section class="card wide"><h3>Functions of the assistant, 30 days (most failing first)</h3><table><thead><tr><th>Function</th>'
            f'<th class="num">Used</th><th class="num">Failed</th><th class="num">Failure rate</th></tr></thead><tbody>{rows}</tbody></table></section>')


def cohort_table(cohorts):
    if not cohorts:
        return ""
    head = "".join(f'<th class="num">+{k} wk</th>' for k in range(1, 6))
    rows = []
    for w, n, later in cohorts:
        cells = "".join(
            f'<td class="num heat" style="--a:{(v / n if n else 0):.2f}" title="{v} of {n}">{(v / n if n else 0):.0%}</td>' for v in later)
        cells += '<td></td>' * (5 - len(later))
        rows.append(f'<tr><td>{esc(w)}</td><td class="num">{n}</td>{cells}</tr>')
    return (f'<section class="card wide"><h3>Return by week of the first start (share of the installations active again)</h3>'
            f'<table><thead><tr><th>First week</th><th class="num">New</th>{head}</tr></thead><tbody>{"".join(rows)}</tbody></table></section>')


def page(s, span, web=""):
    kpis = "".join(f'<div class="tile"><div class="k">{esc(k)}</div><div class="v">{esc(v)}</div>'
                   + (f'<div class="sub">{esc(sub)}</div>' if sub else "") + "</div>" for k, v, sub in s["kpi"])
    downs = Counter({k[5:]: n for k, n in s["ratings"].items() if k.startswith("down_")})
    setup_cards = "".join(hbars(f"Setup: {k.replace('_', ' ')}", c, limit=8) for k, c in sorted(s["setup"].items()) if k != "ai_model")
    models = s["setup"].get("ai_model")
    return f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Compass statistics</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
:root {{ color-scheme: light; --surface:#fcfcfb; --card:#ffffff; --line:#e4e3df; --text:#0b0b0b; --text2:#52514e; --muted:#7a7974;
  --s1:#2a78d6; --s2:#eb6834; --s3:#1baf7a; --s4:#eda100; --s5:#e87ba4; --s6:#008300; }}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{ color-scheme: dark; --surface:#1a1a19; --card:#232322; --line:#3a3936;
  --text:#f0efec; --text2:#c3c2b7; --muted:#8f8e88; --s1:#3987e5; --s2:#d95926; --s3:#199e70; --s4:#c98500; --s5:#d55181; --s6:#008300; }} }}
:root[data-theme="dark"] {{ color-scheme: dark; --surface:#1a1a19; --card:#232322; --line:#3a3936; --text:#f0efec; --text2:#c3c2b7; --muted:#8f8e88;
  --s1:#3987e5; --s2:#d95926; --s3:#199e70; --s4:#c98500; --s5:#d55181; --s6:#008300; }}
body {{ margin:0; background:var(--surface); color:var(--text); font:14px/1.45 "Segoe UI", system-ui, sans-serif; }}
main {{ max-width:1180px; margin:0 auto; padding:24px 16px 48px; }}
h1 {{ font-size:22px; margin:0 0 4px; }} h2 {{ font-size:18px; margin:28px 0 4px; }} h3 {{ font-size:14px; margin:0 0 10px; color:var(--text2); font-weight:600; }}
.muted, .sub {{ color:var(--muted); font-size:12px; }}
.tiles {{ display:grid; grid-template-columns:repeat(6, 1fr); gap:12px; margin:18px 0; }}
.tile, .card, figure {{ background:var(--card); border:1px solid var(--line); border-radius:10px; padding:14px 16px; margin:0; }}
.tile .k {{ color:var(--text2); font-size:12px; }} .tile .v {{ font-size:28px; font-weight:600; margin-top:4px; }}
figure {{ margin-bottom:14px; }} figcaption {{ color:var(--text2); font-weight:600; margin-bottom:6px; }}
svg {{ width:100%; height:auto; display:block; }}
.grid {{ stroke:var(--line); stroke-width:1; }} .base {{ stroke:var(--muted); stroke-width:1; }}
.axis {{ fill:var(--muted); font-size:11px; }} .mark {{ fill:var(--s1); }} .hit:hover .mark {{ opacity:.75; }}
.cards {{ display:grid; grid-template-columns:repeat(3, 1fr); gap:12px; margin-bottom:14px; }}
.wide {{ grid-column:1 / -1; }}
.hrow {{ display:grid; grid-template-columns:150px 1fr 48px; align-items:center; gap:8px; margin:5px 0; }}
.lab {{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; color:var(--text2); }}
.track {{ height:10px; }} .fill {{ display:block; height:10px; background:var(--s1); border-radius:0 4px 4px 0; }}
.val {{ text-align:right; font-variant-numeric:tabular-nums; }}
table {{ width:100%; border-collapse:collapse; }} th, td {{ padding:6px 8px; border-bottom:1px solid var(--line); text-align:left; }}
th {{ color:var(--text2); font-weight:600; font-size:12px; }} .num {{ text-align:right; font-variant-numeric:tabular-nums; }}
.stack {{ width:32%; }} .bar {{ display:flex; }} .seg {{ height:10px; border-right:2px solid var(--card); box-sizing:border-box; flex:none; }}
.seg:last-child {{ border-right:0; }}
.s0 {{ background:var(--s1); }} .s1 {{ background:var(--s2); }} .s2 {{ background:var(--s3); }}
.s3 {{ background:var(--s4); }} .s4 {{ background:var(--s5); }} .s5 {{ background:var(--s6); }}
.legend {{ display:flex; gap:16px; margin-bottom:6px; color:var(--text2); font-size:12px; }}
.key {{ display:inline-flex; align-items:center; gap:6px; }} .sw {{ width:10px; height:10px; border-radius:2px; display:inline-block; }}
.heat {{ background:color-mix(in srgb, var(--s1) calc(var(--a) * 70%), transparent); }}
</style></head><body><main>
<h1>Loonar Compass – usage statistics</h1>
<div class="muted">Up to {esc(s["today"])} (UTC days); {s["turns"]} answers in the last 30 days. Private page generated from the D1 database.</div>
{web}
<h2>The application</h2>
<div class="tiles">{kpis}</div>
{daily_bars(s["timeline"], 1, f"Active installations per day, last {span} days", "active installations")}
{daily_bars(s["timeline"], 2, f"New installations per day, last {span} days", "new installations")}
<div class="cards">
{cohort_table(s["cohorts"])}
{hbars("Versions (active, 7 days)", s["versions"])}
{hbars("Languages (active, 7 days)", s["languages"])}
{hbars("Windows (active, 7 days)", s["systems"])}
{task_table(s["tasks"])}
{hbars("What the assistant could not do, 30 days", s["gaps"])}
{hbars("👎 reasons, 30 days", downs, note=f'👍 {s["ratings"].get("up", 0)} · 👎 {sum(downs.values())}')}
{hbars("AI models (active, 30 days)", models or Counter(), limit=10)}
{tool_table(s["tools"], s["tool_errors"])}
{setup_cards}
</div>
</main></body></html>"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="compass_stats.html")
    ap.add_argument("--days", type=int, default=60, help="days on the daily charts")
    ap.add_argument("--credential", default="cloudflare_api_token.loonar-tools")
    ap.add_argument("--no-open", action="store_true")
    ap.add_argument("--goatcounter-site", default="loonar.goatcounter.com")
    ap.add_argument("--goatcounter-credential", default="goatcounter_api_token.loonar-tools")
    ap.add_argument("--repo", default="loonar-eu/loonar-compass")
    a = ap.parse_args()
    today = dt.datetime.now(dt.timezone.utc).date()
    since = (today - dt.timedelta(days=max(a.days, 7 * 9) + 1)).isoformat()
    auth = token(a.credential)
    starts, days, first = load(auth, since)
    web = web_section(today, a.days, auth, a.goatcounter_site, a.goatcounter_credential, a.repo)
    out = Path(a.out).resolve()
    out.write_text(page(summarize(starts, days, first, today, a.days), a.days, web), encoding="utf-8")
    print(f"{out}  ({len(first)} installations, {len(days)} daily summaries since {since})")
    if not a.no_open:
        webbrowser.open(out.as_uri())


if __name__ == "__main__":
    main()
