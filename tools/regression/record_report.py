"""Records a test done by someone else (a person or another session, e.g. on the lab server) from its written
report: the judge reads the report like any other evidence, the report goes to the evidence issue, the result to
Zephyr. python record_report.py <cycle> <test case key> <report.txt> [<executor name>]"""
import sys
import tempfile
import time
from pathlib import Path

import judge
from services import CONFIG, Jira, Zephyr, secret


def main(cycle, key, report, who="Compass Lab (serwer LAB-DC1)"):
    zephyr, jira = Zephyr(), Jira()
    execution = next(e for e, k in zephyr.cycle_executions(cycle) if k == key)
    case = zephyr.case(key)
    folder = Path(tempfile.mkdtemp(prefix=f"report-{key}-"))
    (folder / Path(report).name).write_bytes(Path(report).read_bytes())
    result, _ = judge.judge(case, folder, secret(CONFIG["claude_credential"]))
    issue = jira.evidence_issue(f"Regression {cycle} – evidence", CONFIG["evidence_parent"])
    link = jira.attach(issue, folder / Path(report).name)
    comment = (f"Executed by: {who}. Judged by: {judge.MODEL} (independent judge) from the execution report.<br>{result['reason']}<br>"
               f"Evidence: {CONFIG['site']}/browse/{issue}<br>{Path(report).name}: {link}")
    zephyr.record(execution["id"], result["verdict"], comment, result["steps"], 0)
    print(key, result["verdict"], result["reason"])


if __name__ == "__main__":
    main(*sys.argv[1:])
