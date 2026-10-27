"""Adds test cases (a folder under the cycle's root folder) and their executions to a Zephyr test cycle.
python add_cases.py <cycle> <cases.json>   (a list of {"folder", "name", "minutes", "pre", "steps": [[action, expected, data]]})"""
import json
import sys
import urllib.request

from services import CONFIG, Zephyr


def post(z, path, body):
    request = urllib.request.Request(z.base + path, data=json.dumps(body).encode(), method="POST", headers={**z.auth, "Content-Type": "application/json"})
    with urllib.request.urlopen(request) as r:
        text = r.read().decode()
        return json.loads(text) if text.strip() else {}


def main(cycle, file):
    z = Zephyr()
    cases = json.load(open(file, encoding="utf-8"))
    folders = {f["name"]: f["id"] for f in z.get(f"/folders?projectKey={CONFIG['project']}&folderType=TEST_CASE&maxResults=200")["values"]}
    root = folders.get("Compass 0.9 – testy ręczne")
    existing = {z.case(k)["name"] for _, k in z.cycle_executions(cycle)}
    for case in cases:
        if case["name"] in existing:
            print("exists:", case["name"])
            continue
        if case["folder"] not in folders:
            folders[case["folder"]] = post(z, "/folders", {"projectKey": CONFIG["project"], "name": case["folder"], "folderType": "TEST_CASE", "parentId": root})["id"]
        key = post(z, "/testcases", {"projectKey": CONFIG["project"], "name": case["name"], "folderId": folders[case["folder"]], "estimatedTime": case["minutes"] * 60000,
                                     "objective": f"Weryfikacja wersji 0.9 – {case['folder']}.", "precondition": case["pre"], "labels": ["0.9", "reczne"], "statusName": "Approved"})["key"]
        post(z, f"/testcases/{key}/teststeps", {"mode": "OVERWRITE", "items": [{"inline": {"description": a, "expectedResult": e, "testData": d}} for a, e, d in case["steps"]]})
        post(z, "/testexecutions", {"projectKey": CONFIG["project"], "testCaseKey": key, "testCycleKey": cycle, "statusName": "Not Executed"})
        print("added", key, case["name"])


if __name__ == "__main__":
    main(*sys.argv[1:])
