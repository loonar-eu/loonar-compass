"""Sends development data to Jira (project LC) through the Jira DevOps APIs, with the OAuth credentials of the
"Loonar Compass" integration (secrets JIRA_CLIENT_ID / JIRA_CLIENT_SECRET, variable JIRA_CLOUD_ID).

  python jira_sync.py commits                      commits of this push with Jira keys (LC-123) in the message
  python jira_sync.py build <state> <test log>     build with test results for the commits of this push
  python jira_sync.py release                      deployment to production of the issues released in this tag

Only commits that name a key are sent; the rest of the history stays out of Jira."""
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

KEY = re.compile(r"\b(LC-\d+)\b")
REPO_URL = f"{os.environ.get('GITHUB_SERVER_URL', 'https://github.com')}/{os.environ.get('GITHUB_REPOSITORY', 'loonar-eu/loonar-compass')}"
CLOUD = os.environ.get("JIRA_CLOUD_ID", "")
SEQ = int(time.time() * 1000)


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, encoding="utf-8", check=True).stdout


def token():
    body = {"audience": "api.atlassian.com", "grant_type": "client_credentials",
            "client_id": os.environ["JIRA_CLIENT_ID"], "client_secret": os.environ["JIRA_CLIENT_SECRET"]}
    req = urllib.request.Request("https://api.atlassian.com/oauth/token", data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())["access_token"]


def send(api, body):
    if not (CLOUD and os.environ.get("JIRA_CLIENT_ID") and os.environ.get("JIRA_CLIENT_SECRET")):
        print("::warning::Jira is not configured (secrets JIRA_CLIENT_ID / JIRA_CLIENT_SECRET, variable JIRA_CLOUD_ID)")
        return
    try:
        req = urllib.request.Request(f"https://api.atlassian.com/jira/{api}/0.1/cloud/{CLOUD}/bulk", data=json.dumps(body).encode(), method="POST",
                                     headers={"Authorization": "Bearer " + token(), "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            print(api, r.status, r.read().decode()[:300])
    except urllib.error.HTTPError as e:
        # A Jira problem must not fail the build of the application.
        print(f"::warning::Jira {api}: {e.code} {e.read().decode()[:300]}")


def pushed_commits():
    """Commits of this push (event payload), or the last commit when run by hand."""
    event = json.load(open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8")) if os.environ.get("GITHUB_EVENT_PATH") else {}
    shas = [c["id"] for c in event.get("commits", [])] or [git("rev-parse", "HEAD").strip()]
    return shas


def commit_info(sha):
    name, email, ts, msg = git("show", "-s", "--format=%an%x1f%ae%x1f%aI%x1f%B", sha).split("\x1f", 3)
    files = [l for l in git("show", "--name-only", "--format=", sha).splitlines() if l.strip()]
    return {"id": sha, "issueKeys": sorted(set(KEY.findall(msg))), "updateSequenceId": SEQ, "hash": sha, "displayId": sha[:7], "flags": [],
            "message": msg.strip()[:1000], "author": {"name": name, "email": email}, "fileCount": len(files), "url": f"{REPO_URL}/commit/{sha}",
            "authorTimestamp": ts, "files": [{"path": f, "url": f"{REPO_URL}/blob/{sha}/{f}", "changeType": "MODIFIED", "linesAdded": 0, "linesRemoved": 0} for f in files[:10]]}


def commits():
    found = [c for c in map(commit_info, pushed_commits()) if c["issueKeys"]]
    if not found:
        print("no commit names a Jira key")
        return
    branch = os.environ.get("GITHUB_REF_NAME", "")
    keys = sorted({k for c in found for k in c["issueKeys"]})
    repo = {"id": "loonar-compass", "name": "loonar-compass", "url": REPO_URL, "updateSequenceId": SEQ, "commits": found}
    if branch:
        repo["branches"] = [{"id": branch, "issueKeys": keys, "name": branch, "lastCommit": found[-1], "url": f"{REPO_URL}/tree/{branch}",
                             "createPullRequestUrl": f"{REPO_URL}/compare/{branch}?expand=1", "updateSequenceId": SEQ}]
    send("devinfo", {"repositories": [repo]})


def build(state, log_path):
    keys = sorted({k for sha in pushed_commits() for k in KEY.findall(git("show", "-s", "--format=%B", sha))})
    if not keys:
        print("no commit names a Jira key")
        return
    passed = failed = ignored = 0
    if os.path.exists(log_path):
        for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", open(log_path, encoding="utf-8", errors="replace").read()):
            passed, failed, ignored = passed + int(m[1]), failed + int(m[2]), ignored + int(m[3])
    run = f"{REPO_URL}/actions/runs/{os.environ.get('GITHUB_RUN_ID', '')}"
    send("builds", {"builds": [{"pipelineId": "tests", "buildNumber": int(os.environ.get("GITHUB_RUN_NUMBER", "1")), "updateSequenceNumber": SEQ,
                                "displayName": f"Testy ({os.environ.get('GITHUB_REF_NAME', '')})", "url": run, "state": state,
                                "lastUpdated": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "issueKeys": keys[:500],
                                "testInfo": {"totalNumber": passed + failed + ignored, "numberPassed": passed, "numberFailed": failed, "numberSkipped": ignored},
                                "references": [{"commit": {"id": git("rev-parse", "HEAD").strip(), "repositoryUri": REPO_URL},
                                                "ref": {"name": os.environ.get("GITHUB_REF_NAME", ""), "uri": f"{REPO_URL}/tree/{os.environ.get('GITHUB_REF_NAME', '')}"}}]}]})


def release():
    tag = os.environ["GITHUB_REF_NAME"]
    tags = [t for t in git("tag", "--sort=-creatordate").split() if t.startswith("v")]
    previous = next((t for t in tags if t != tag), None)
    log = git("log", "--format=%B%x1e", f"{previous}..{tag}" if previous else tag)
    keys = sorted(set(KEY.findall(log)))
    if not keys:
        print("no Jira keys in this release")
        return
    numbers = [int(n) for n in re.findall(r"\d+", tag)] + [0, 0, 0]
    send("deployments", {"deployments": [{"deploymentSequenceNumber": numbers[0] * 1_000_000 + numbers[1] * 1_000 + numbers[2], "updateSequenceNumber": SEQ,
                                          "associations": [{"associationType": "issueIdOrKeys", "values": keys[:500]}],
                                          "displayName": f"Loonar Compass {tag.lstrip('v')}", "url": f"{REPO_URL}/releases/tag/{tag}", "description": "Wydanie",
                                          "lastUpdated": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "state": "successful",
                                          "pipeline": {"id": "release", "displayName": "Wydanie Loonar Compass", "url": f"{REPO_URL}/releases"},
                                          "environment": {"id": "prod", "displayName": "Produkcja (loonar.eu, GitHub Releases)", "type": "production"}}]})


if __name__ == "__main__":
    what = sys.argv[1]
    if what == "commits":
        commits()
    elif what == "build":
        build(sys.argv[2], sys.argv[3])
    elif what == "release":
        release()
