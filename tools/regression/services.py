"""Zephyr Scale and Jira access for the regression run. Site, account and project come from config.local.json
(not in the repository); API keys are read from Windows Credential Manager and never printed."""
import ctypes
import ctypes.wintypes as wt
import base64
import json
import mimetypes
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

CONFIG = json.loads((Path(__file__).with_name("config.local.json")).read_text(encoding="utf-8"))


class _Credential(ctypes.Structure):
    _fields_ = [("Flags", wt.DWORD), ("Type", wt.DWORD), ("TargetName", wt.LPWSTR), ("Comment", wt.LPWSTR),
                ("LastWritten", wt.FILETIME), ("CredentialBlobSize", wt.DWORD), ("CredentialBlob", ctypes.POINTER(ctypes.c_byte)),
                ("Persist", wt.DWORD), ("AttributeCount", wt.DWORD), ("Attributes", ctypes.c_void_p), ("TargetAlias", wt.LPWSTR), ("UserName", wt.LPWSTR)]


def secret(target):
    advapi = ctypes.WinDLL("advapi32", use_last_error=True)
    ptr = ctypes.POINTER(_Credential)()
    if not advapi.CredReadW(target, 1, 0, ctypes.byref(ptr)):
        raise SystemExit(f"no credential {target} in Windows Credential Manager")
    c = ptr.contents
    if not c.CredentialBlobSize:
        advapi.CredFree(ptr)
        raise SystemExit(f"credential {target} is empty")
    blob = bytes(ctypes.cast(c.CredentialBlob, ctypes.POINTER(ctypes.c_byte * c.CredentialBlobSize)).contents)
    advapi.CredFree(ptr)
    return (blob.decode("utf-16-le") if b"\x00" in blob else blob.decode("utf-8")).strip()


def _call(url, method="GET", body=None, headers=None, raw=None):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    h = {"Accept": "application/json", **(headers or {})}
    if body is not None:
        h["Content-Type"] = "application/json"
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=data, method=method, headers=h), timeout=60) as r:
            text = r.read().decode("utf-8")
            return json.loads(text) if text.strip() else {}
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {url}: {e.code} {e.read().decode('utf-8', 'replace')[:400]}") from None


class Zephyr:
    def __init__(self):
        self.base = CONFIG["zephyr_api"]
        self.auth = {"Authorization": "Bearer " + secret(CONFIG["zephyr_credential"])}
        self.statuses = {s["name"]: s["id"] for s in self.get(f"/statuses?projectKey={CONFIG['project']}&statusType=TEST_EXECUTION&maxResults=50")["values"]}

    def get(self, path):
        return _call(self.base + path, headers=self.auth)

    def put(self, path, body):
        return _call(self.base + path, "PUT", body, self.auth)

    def cycle_executions(self, cycle):
        """[(execution, test case key)] of a test cycle."""
        out, start = [], 0
        while True:
            page = self.get(f"/testexecutions?projectKey={CONFIG['project']}&testCycle={cycle}&maxResults=100&startAt={start}")
            for e in page["values"]:
                out.append((e, e["testCase"]["self"].rstrip("/").split("/")[-3]))
            if page.get("isLast", True):
                return out
            start += len(page["values"])

    def case(self, key):
        c = self.get(f"/testcases/{key}")
        steps = self.get(f"/testcases/{key}/teststeps?maxResults=100")["values"]
        return {"key": key, "name": c["name"], "objective": c.get("objective") or "", "precondition": c.get("precondition") or "",
                "steps": [{"action": s["inline"]["description"], "data": s["inline"].get("testData") or "", "expected": s["inline"]["expectedResult"]} for s in steps]}

    def record(self, execution_id, verdict, comment, step_results, seconds):
        status = {"PASS": "Pass", "FAIL": "Fail", "BLOCKED": "Blocked"}[verdict]
        self.put(f"/testexecutions/{execution_id}", {"statusName": status, "comment": comment, "executionTime": int(seconds * 1000)})
        if step_results:
            self.put(f"/testexecutions/{execution_id}/teststeps", {"steps": [
                {"statusName": {"PASS": "Pass", "FAIL": "Fail", "BLOCKED": "Blocked"}[s["verdict"]], "actualResult": s["note"]} for s in step_results]})


class Jira:
    def __init__(self):
        token = secret(CONFIG["jira_credential"])
        self.site = CONFIG["site"]
        self.auth = {"Authorization": "Basic " + base64.b64encode(f"{CONFIG['account']}:{token}".encode()).decode()}

    def call(self, method, path, body=None):
        return _call(self.site + path, method, body, self.auth)

    def evidence_issue(self, summary, parent):
        """The issue that holds the evidence files of a run: the newest one whose summary starts with this text (also when
        closed: it is only a container, kept out of the release), created once if there is none."""
        jql = urllib.parse.quote(f'project = {CONFIG["project"]} AND summary ~ "\\"{summary}\\"" ORDER BY created DESC')
        found = self.call("GET", f"/rest/api/3/search/jql?jql={jql}&fields=summary")["issues"]
        if found:
            return found[0]["key"]
        return self.call("POST", "/rest/api/3/issue", {"fields": {"project": {"key": CONFIG["project"]}, "issuetype": {"name": "Task"}, "summary": summary, "parent": {"key": parent},
                                                               "labels": ["regression-evidence"]}})["key"]

    def attach(self, key, path):
        """Uploads one file; returns its URL in Jira."""
        boundary = uuid.uuid4().hex
        media = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        data = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{path.name}"\r\nContent-Type: {media}\r\n\r\n').encode() + path.read_bytes() + f"\r\n--{boundary}--\r\n".encode()
        result = _call(f"{self.site}/rest/api/3/issue/{key}/attachments", "POST", raw=data,
                       headers={**self.auth, "Content-Type": f"multipart/form-data; boundary={boundary}", "X-Atlassian-Token": "no-check"})
        return f"{self.site}/rest/api/3/attachment/content/{result[0]['id']}"
