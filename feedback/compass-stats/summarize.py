"""Builds SUMMARY.md and summary.json from the daily summaries in data/YYYY/MM/<day>/<installation>.json.

python summarize.py            (the GitHub workflow runs it every day)
"""
import collections
import datetime
import json
import pathlib

ROOT = pathlib.Path(__file__).parent
DATA = ROOT / "data"


def load():
    for path in sorted(DATA.glob("*/*/*/*.json")):
        try:
            yield json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue


def pct(part, whole):
    return f"{100 * part / whole:.0f}%" if whole else "–"


def table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out)


def main():
    days = collections.defaultdict(list)
    for s in load():
        days[s["day"]].append(s)
    today = datetime.date.today()
    recent = [d for d in sorted(days) if (today - datetime.date.fromisoformat(d)).days <= 30]
    window = [s for d in recent for s in days[d]]

    installs = {s["install"] for s in window}
    tools, errors = collections.Counter(), collections.Counter()
    versions, providers, models, modes, sources, langs = (collections.Counter() for _ in range(6))
    approvals, views, events = collections.Counter(), collections.Counter(), collections.Counter()
    turns = ai_errors = 0
    latest = {}
    for s in window:
        tools.update(s.get("tools", {}))
        errors.update(s.get("tool_errors", {}))
        approvals.update(s.get("approvals", {}))
        views.update(s.get("views", {}))
        events.update(s.get("events", {}))
        turns += s.get("turns", 0)
        ai_errors += s.get("ai_errors", 0)
        if s["day"] >= latest.get(s["install"], {}).get("day", ""):
            latest[s["install"]] = s
    for s in latest.values():
        setup = s.get("setup", {})
        versions[s["app"]] += 1
        langs[s["lang"]] += 1
        providers[setup.get("ai_provider", "?")] += 1
        models[setup.get("ai_model", "?")] += 1
        modes[setup.get("mode", "?")] += 1
        sources[setup.get("model_source", "?")] += 1

    lines = [
        "# Loonar Compass – usage in the last 30 days",
        "",
        f"Updated {today.isoformat()}. Anonymous daily summaries sent by the application (see README.md for what is collected).",
        "",
        f"- Active installations: **{len(installs)}**",
        f"- Answers of the assistant: **{turns}**, AI service errors: {ai_errors} ({pct(ai_errors, turns)})",
        f"- Approvals asked: {approvals['asked']}, rejected: {approvals['rejected']} ({pct(approvals['rejected'], approvals['asked'])})",
        "",
        "## Active installations per day",
        "",
        table(["day", "installations", "answers"], [(d, len({s['install'] for s in days[d]}), sum(s.get('turns', 0) for s in days[d])) for d in reversed(recent)]),
        "",
        "## Functions (tools) used",
        "",
        table(["tool", "calls", "failed", "failure rate"], [(t, n, errors[t], pct(errors[t], n)) for t, n in tools.most_common()]),
        "",
        "## Setup (latest day of each installation)",
        "",
        table(["version", "installations"], versions.most_common()),
        "",
        table(["where", "installations"], modes.most_common()),
        "",
        table(["model source", "installations"], sources.most_common()),
        "",
        table(["AI provider", "installations"], providers.most_common()),
        "",
        table(["AI model", "installations"], models.most_common()),
        "",
        table(["language", "installations"], langs.most_common()),
        "",
        "## Views opened",
        "",
        table(["view", "times"], views.most_common()),
        "",
        "## Other events",
        "",
        table(["event", "times"], events.most_common()),
        "",
    ]
    (ROOT / "SUMMARY.md").write_text("\n".join(lines), encoding="utf-8")
    summary = {
        "updated": today.isoformat(),
        "installations": len(installs),
        "turns": turns,
        "ai_errors": ai_errors,
        "tools": dict(tools.most_common()),
        "tool_errors": dict(errors.most_common()),
        "versions": dict(versions),
        "providers": dict(providers),
        "models": dict(models),
        "modes": dict(modes),
        "model_sources": dict(sources),
        "approvals": dict(approvals),
        "views": dict(views),
        "events": dict(events),
        "per_day": {d: {"installations": len({s["install"] for s in days[d]}), "turns": sum(s.get("turns", 0) for s in days[d])} for d in recent},
    }
    (ROOT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"{len(window)} summaries, {len(installs)} installations")


if __name__ == "__main__":
    main()
