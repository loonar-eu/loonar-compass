// The Worker on this computer with a fake GitHub, for testing the application end to end:
//   node feedback/dev-server.mjs 8787
//   set LOONAR_FEEDBACK_URL=http://127.0.0.1:8787 and start Compass (or run the ignored Rust test feedback_end_to_end)
// Every call that would go to GitHub is printed as one JSON line ("github ...").
import http from "node:http";
import worker from "./worker.js";

const port = Number(process.argv[2] || 8787);
let issue = 0;
globalThis.fetch = async (url, init) => {
  const body = init.body ? JSON.parse(init.body) : {};
  const shown = { method: init.method, url, ...(body.title ? { title: body.title, issue_body: body.body } : {}) };
  if (body.content) shown.content = Buffer.from(body.content, "base64").toString("utf8").slice(0, 2000);
  console.log("github " + JSON.stringify(shown));
  if (init.method === "POST") {
    issue += 1;
    return new Response(JSON.stringify({ number: issue, html_url: `https://github.com/loonar-eu/loonar-compass/issues/${issue}` }), { status: 201 });
  }
  return new Response("{}", { status: 201 });
};
const env = { GITHUB_TOKEN: "dev", GITHUB_API: "https://api.github.invalid", ISSUES_REPO: "loonar-eu/loonar-compass", STATS_REPO: "loonar-eu/compass-stats", PRIVATE_REPO: "loonar-eu/compass-private" };

http
  .createServer(async (req, res) => {
    const chunks = [];
    for await (const c of req) chunks.push(c);
    const request = new Request(`http://127.0.0.1:${port}${req.url}`, { method: req.method, headers: req.headers, body: ["GET", "HEAD"].includes(req.method) ? undefined : Buffer.concat(chunks) });
    const response = await worker.fetch(request, env);
    res.writeHead(response.status, Object.fromEntries(response.headers));
    res.end(Buffer.from(await response.arrayBuffer()));
  })
  .listen(port, "127.0.0.1", () => console.log(`feedback dev server on http://127.0.0.1:${port}`));
