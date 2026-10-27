"""Regression run of a Zephyr test cycle: executes every test case that has an executor, lets the judge decide
from the evidence, attaches the evidence to a Jira issue and records the result in Zephyr.

  python run.py LC-R1                      every automatable test of the cycle
  python run.py LC-R1 "Raporty Word i Excel pakietu w hubie"   only the named test cases
  python run.py LC-R1 --dry-run            judge, but do not write to Zephyr or Jira"""
import shutil
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path

import executors
import executors_window  # noqa: F401  (registers the window tests)
import judge
from services import CONFIG, Jira, Zephyr, secret

PRICE_IN, PRICE_OUT = 0.10, 0.50  # USD per million tokens of the judge model (prompts under 100K tokens)


def main(argv):
    cycle, names = argv[0], [a for a in argv[1:] if not a.startswith("--")]
    dry = "--dry-run" in argv
    zephyr, jira = Zephyr(), (None if dry else Jira())
    api_key = secret(CONFIG["claude_credential"])
    run_dir = Path(__file__).with_name("out") / f"{cycle}-{datetime.now():%Y%m%d-%H%M}"
    evidence_issue = None
    tokens_in = tokens_out = 0
    summary = []
    for execution, key in zephyr.cycle_executions(cycle):
        case = zephyr.case(key)
        fn = executors.EXECUTORS.get(case["name"])
        if not fn or (names and case["name"] not in names):
            continue
        out = run_dir / key
        out.mkdir(parents=True, exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix=f"regression-{key}-"))
        print(f"{key} {case['name']}: running…", flush=True)
        started = time.time()
        try:
            fn(out, work)
        except Exception as e:  # a broken executor is evidence too: the judge sees it and blocks the test
            (out / "executor_error.txt").write_text(f"{type(e).__name__}: {e}", encoding="utf-8")
        finally:
            shutil.rmtree(work, ignore_errors=True)
        result, usage = judge.judge(case, out, api_key)
        tokens_in, tokens_out = tokens_in + usage.input_tokens, tokens_out + usage.output_tokens
        print(f"  {result['verdict']}: {result['reason']}", flush=True)
        links = []
        if not dry:
            evidence_issue = evidence_issue or jira.evidence_issue(f"Regression {cycle} – evidence", CONFIG["evidence_parent"])
            links = [f"{p.name}: {jira.attach(evidence_issue, p)}" for p in sorted(out.iterdir()) if p.is_file()]
            comment = (f"Executed by: Claude (automated). Judged by: {judge.MODEL} (independent judge).<br>{result['reason']}<br>"
                       f"Evidence: {CONFIG['site']}/browse/{evidence_issue}<br>" + "<br>".join(links))
            zephyr.record(execution["id"], result["verdict"], comment, result["steps"], time.time() - started)
        summary.append((key, case["name"], result["verdict"]))
    cost = tokens_in / 1e6 * PRICE_IN + tokens_out / 1e6 * PRICE_OUT
    print(f"\n{len(summary)} tests: " + ", ".join(f"{v} {sum(1 for s in summary if s[2] == v)}" for v in ("PASS", "FAIL", "BLOCKED")))
    print(f"Judge: {tokens_in} input tokens, {tokens_out} output tokens, about {cost:.4f} USD. Evidence: {run_dir}")


if __name__ == "__main__":
    main(sys.argv[1:])
