// Tests of the feedback Worker against a fake GitHub API: node feedback/test.mjs
import assert from "node:assert/strict";
import worker, { validateStats, validateReport, noMentions, weekSummary, lastWeek } from "./worker.js";

const calls = [];
let issueNumber = 41;
globalThis.fetch = async (url, init) => {
  const body = init.body ? JSON.parse(init.body) : undefined;
  calls.push({ url, method: init.method, body, auth: init.headers.Authorization });
  if (init.method === "POST" && url.endsWith("/issues")) {
    issueNumber += 1;
    return new Response(JSON.stringify({ number: issueNumber, html_url: `https://github.com/loonar-eu/loonar-compass/issues/${issueNumber}` }), { status: 201 });
  }
  if (init.method === "PUT" && url.includes("/contents/") && url.includes("exists") && !body.sha) return new Response("{}", { status: 422 });
  if (init.method === "GET" && url.includes("/contents/")) return new Response(JSON.stringify({ sha: "abc" }), { status: 200 });
  if (init.method === "PUT") return new Response("{}", { status: 201 });
  return new Response("{}", { status: 404 });
};

const env = {
  GITHUB_TOKEN: "test-token",
  GITHUB_API: "https://api.test",
  ISSUES_REPO: "loonar-eu/loonar-compass",
  STATS_REPO: "loonar-eu/compass-stats",
  PRIVATE_REPO: "loonar-eu/compass-private",
};
const post = (path, body, headers = {}) =>
  worker.fetch(new Request("https://feedback.test" + path, { method: "POST", body: typeof body === "string" ? body : JSON.stringify(body), headers }), env);

const today = new Date().toISOString().slice(0, 10);
const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
const install = "7f3c2b1a-1234-4abc-8def-0123456789ab";
const summary = {
  v: 1, install, day: yesterday, app: "0.9.0", os: "Windows 11 (26200)", lang: "pl",
  setup: { mode: "ea_tab", model_source: "file", ai_provider: "anthropic", ai_model: "claude-haiku-5-5", approval: "ask", jira: true, confluence: false, hub: false, mcp_servers: 2, event_agents: 1 },
  turns: 14, ai_errors: 1,
  tools: { ea_write: 5, db_query: 22, mcp: 3 }, tool_errors: { ea_write: 1 },
  approvals: { asked: 6, accepted: 5, rejected: 1 }, views: { chat: 3, statistics: 1 }, events: { session_ea_tab: 2 }, tokens: { in: 120000, out: 9000 },
};

// 1. A valid summary is stored in the public repository under data/YYYY/MM/day/install.json.
let r = await post("/v1/stats", summary);
assert.equal(r.status, 201);
const put = calls.find((c) => c.method === "PUT");
assert.ok(put.url.startsWith(`https://api.test/repos/loonar-eu/compass-stats/contents/data/${yesterday.slice(0, 4)}/${yesterday.slice(5, 7)}/${yesterday}/${install}.json`));
assert.equal(put.auth, "Bearer test-token");
assert.deepEqual(JSON.parse(Buffer.from(put.body.content, "base64").toString()), summary);

// 2. Anything unknown or out of range is refused (400), nothing reaches GitHub.
calls.length = 0;
for (const bad of [
  { ...summary, prompt: "secret question" },
  { ...summary, install: "not-a-uuid" },
  { ...summary, day: "2020-01-01" },
  { ...summary, tools: { "C:\\models\\acme.qea": 1 } },
  { ...summary, setup: { ...summary.setup, server: "db.acme.local" } },
  { ...summary, setup: { ...summary.setup, ai_model: "Acme Internal GPT" } },
  { ...summary, turns: -1 },
]) {
  r = await post("/v1/stats", bad);
  assert.equal(r.status, 400, JSON.stringify(bad).slice(0, 80));
}
assert.equal((await post("/v1/stats", "{not json")).status, 400);
assert.equal((await post("/v1/stats", "x".repeat(40_000))).status, 400);
assert.equal(calls.length, 0, "refused summaries never reach GitHub");

// 3. A report: public issue without the private parts; the private parts go to the private repository.
const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3]).toString("base64");
const report = {
  v: 1, install, app: "0.9.0", os: "Windows 11 (26200)", lang: "pl",
  title: "Wrong answer\nof the assistant", description: "It added the class twice. cc @someone",
  technical: "Loonar Compass 0.9.0\nSystem: Windows 11", private: "Question:\nAdd class Invoice to package ACME-Billing\n\nAnswer:\nDone.",
  images: [{ name: "shot.png", data: png }],
};
calls.length = 0;
r = await post("/v1/report", report);
assert.equal(r.status, 201);
const answer = await r.json();
assert.equal(answer.url, "https://github.com/loonar-eu/loonar-compass/issues/42");
const issue = calls.find((c) => c.url.endsWith("/repos/loonar-eu/loonar-compass/issues"));
assert.equal(issue.body.title, "[0.9.0] Wrong answer of the assistant");
assert.deepEqual(issue.body.labels, ["bug", "from-app"]);
assert.ok(!issue.body.body.includes("ACME-Billing"), "the reported answer is not public");
assert.ok(!issue.body.body.includes("@someone"), "mentions are broken");
assert.ok(issue.body.body.includes("1 screenshot(s) and the reported answer"));
const privateFiles = calls.filter((c) => c.method === "PUT").map((c) => c.url);
assert.equal(privateFiles.length, 2);
assert.ok(privateFiles.every((u) => u.includes("/repos/loonar-eu/compass-private/contents/reports/") && u.includes("/issue-42-")));
const privateText = Buffer.from(calls.find((c) => c.url.endsWith("report.md")).body.content, "base64").toString();
assert.ok(privateText.includes("ACME-Billing") && privateText.includes("issues/42"));

// 4. Reports that are not allowed.
calls.length = 0;
for (const bad of [
  { ...report, description: "  " },
  { ...report, images: [{ data: Buffer.from("GIF89a").toString("base64") }] },
  { ...report, images: Array(6).fill({ data: png }) },
  { ...report, extra: 1 },
]) {
  assert.equal((await post("/v1/report", bad)).status, 400);
}
assert.equal(calls.length, 0);

// 5. Helpers.
assert.equal(validateStats(summary).turns, 14);
assert.equal(noMentions("hi @bob and a@b"), "hi @\u200bbob and a@\u200bb");
assert.throws(() => validateReport({ ...report, v: 2 }));

// 6. Rate limit.
const blocked = { limit: async () => ({ success: false }) };
r = await worker.fetch(new Request("https://feedback.test/v1/report", { method: "POST", body: JSON.stringify(report) }), { ...env, REPORT_LIMITER: blocked });
assert.equal(r.status, 429);

// 7. Health and unknown paths.
assert.equal((await worker.fetch(new Request("https://feedback.test/health"), env)).status, 200);
assert.equal((await worker.fetch(new Request("https://feedback.test/v1/other", { method: "POST" }), env)).status, 404);
// 8. The weekly summary: totals over all installations, rare values folded into "other".
assert.equal(lastWeek(new Date("2026-10-27T03:17:00Z")), "2026-10-19");
assert.equal(lastWeek(new Date("2026-11-01T10:00:00Z")), "2026-10-19");
{
  const ids = ["a", "b", "c", "d"].map((x) => `7f3c2b1a-1234-4abc-8def-0123456789a${x}`);
  const starts = [
    ...ids.slice(0, 3).map((install) => ({ install, day: "2026-10-20", app: "0.9.1", os: "Windows 11 (26200)", lang: "pl" })),
    { install: ids[0], day: "2026-10-22", app: "0.9.1", os: "Windows 11 (26200)", lang: "pl" },
    { install: ids[3], day: "2026-10-21", app: "0.9.0", os: "Windows 10 (19045)", lang: "de" },
  ];
  const days = ids.map((install) => ({ install, day: "2026-10-20", data: JSON.stringify({ ...summary, install, day: "2026-10-20" }) }));
  const first = { [ids[0]]: "2026-10-01", [ids[1]]: "2026-10-20", [ids[2]]: "2026-10-21", [ids[3]]: "2026-09-01" };
  const w = weekSummary("2026-10-19", starts, days, first);
  assert.equal(w.installations, 4);
  assert.equal(w.new_installations, 2);
  assert.equal(w.returning, 2); // a on two days; d started on another day than its summary
  assert.deepEqual(w.versions, { "0.9.1": 3, other: 1 });
  assert.deepEqual(w.languages, { pl: 3, other: 1 });
  assert.deepEqual(w.systems, { "Windows 11": 3, other: 1 });
  assert.equal(w.counters.turns, 56);
  assert.equal(w.counters.tools.db_query, 88);
  assert.deepEqual(w.setup.jira, { true: 4 });
  assert.ok(!JSON.stringify(w).includes(ids[0]) && !("ai_model" in w.setup));
}
console.log("feedback worker: all tests passed");
