"""Creates the 0.9.2 test cycle with the cases of cases_092.json (once; existing cases are reused by name)."""
import json
import sys

from add_cases import post
from services import CONFIG, Zephyr


def main(file="cases_092.json", cycle_name="Compass 0.9.2 – nowe funkcje (LC-303…LC-311)"):
    z = Zephyr()
    project = CONFIG["project"]
    cycles = z.get(f"/testcycles?projectKey={project}&maxResults=200")["values"]
    cycle = next((c["key"] for c in cycles if c["name"] == cycle_name), None) or post(z, "/testcycles", {"projectKey": project, "name": cycle_name, "description": "Testy zadań wersji 0.9.2."})["key"]
    folders = {f["name"]: f["id"] for f in z.get(f"/folders?projectKey={project}&folderType=TEST_CASE&maxResults=200")["values"]}
    existing = {z.case(k)["name"] for _, k in z.cycle_executions(cycle)}
    for case in json.load(open(file, encoding="utf-8")):
        if case["name"] in existing:
            continue
        if case["folder"] not in folders:
            folders[case["folder"]] = post(z, "/folders", {"projectKey": project, "name": case["folder"], "folderType": "TEST_CASE"})["id"]
        key = post(z, "/testcases", {"projectKey": project, "name": case["name"], "folderId": folders[case["folder"]], "estimatedTime": case["minutes"] * 60000,
                                     "objective": f"Weryfikacja zadań {case['task']} (wersja 0.9.2).", "precondition": case["pre"], "labels": ["0.9.2"], "statusName": "Approved"})["key"]
        post(z, f"/testcases/{key}/teststeps", {"mode": "OVERWRITE", "items": [{"inline": {"description": a, "expectedResult": e, "testData": d}} for a, e, d in case["steps"]]})
        post(z, "/testexecutions", {"projectKey": project, "testCaseKey": key, "testCycleKey": cycle, "statusName": "Not Executed"})
        print("added", key, case["name"])
    print("cycle", cycle)


if __name__ == "__main__":
    main(*sys.argv[1:])
