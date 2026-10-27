# Feedback service

A small Cloudflare Worker between Loonar Compass and GitHub. The application never holds a GitHub token.

| Request | What happens |
|---|---|
| `POST /v1/start` | The start of the application (random installation id, version, Windows, language, day) is stored in the D1 database `compass-stats` (table `starts`). |
| `POST /v1/stats` | The usage summary of a day of one installation (sent every hour, `partial` until the day ends) is checked against a strict schema and stored in D1 (table `days`; the newest replaces the earlier one, a final one is never replaced by a partial one). Without the database binding it goes to the public repository `loonar-eu/compass-stats` as before. |
| `GET /v1/summary` | The summary of a week over all installations together (`?week=<Monday>`, default the newest); values shared by fewer than 3 installations are counted as `other`. |
| cron, daily 03:17 UTC | Deletes data older than 13 months; computes the summary of the week that ended (table `weekly`). |
| `POST /v1/report` | A bug report becomes a **public** issue in `loonar-eu/loonar-compass` (labels `bug`, `from-app`) with the title, the description and the technical information. The screenshots and the context of the reported answer go to the **private** repository `loonar-eu/compass-private` (`reports/YYYY-MM/issue-<n>-<ref>/`). |
| `GET /health` | `{"ok": true}` |

Limits: 30 summaries and 3 reports a minute per address; a summary up to 32 KB; a report up to 5 PNG/JPEG images of
3 MB each. `@mentions` in the description are broken so that a report cannot notify GitHub users.

## Setting it up (once)

1. **Repositories** (GitHub, organisation `loonar-eu`):
   - `compass-stats`, **public**: copy the contents of `feedback/compass-stats/` (README, `summarize.py`, the
     workflow and the empty `data/`). In Settings → Actions → General allow "Read and write permissions" for the
     daily summary.
   - `compass-private`, **private**, empty.
   - In `loonar-compass` create the labels `bug`, `from-app`, `idea`, `question`, and a public Project
     (Projects → New project → Board, visibility Public) with the columns *New*, *Accepted*, *In progress*, *Done*;
     add the workflow "Item added to project → auto-add issues from loonar-compass".
2. **Token**: a fine-grained personal access token (Settings → Developer settings → Fine-grained tokens), resource
   owner `loonar-eu`, repositories `loonar-compass`, `compass-stats`, `compass-private`, permissions
   *Contents: Read and write* and *Issues: Read and write*, expiry 1 year (put a reminder in the calendar).
3. **Cloudflare** (free plan): `npx wrangler login`, then in this folder `npx wrangler secret put GITHUB_TOKEN`
   (paste the token there yourself) and `npx wrangler deploy`.
   - The address `feedback.loonar.eu` needs the `loonar.eu` zone in Cloudflare. Without it, remove `routes` from
     `wrangler.toml`; the Worker gets `https://compass-feedback.<account>.workers.dev` and the constant
     `SERVICE_URL` in `src/telemetry.rs` must be changed to it.
4. Check: `curl https://feedback.loonar.eu/health`.

No IP addresses are stored anywhere. The schema: `npx wrangler d1 execute compass-stats --remote --file schema.sql`.

The author's private dashboard: `python tools/stats/dashboard.py` (reads D1 with the Cloudflare API token, writes
`compass_stats.html` and opens it).

## Testing

- `node feedback/test.mjs` – the Worker against a fake GitHub (schema, privacy of the report, limits).
- `node feedback/dev-server.mjs 8787`, then
  `LOONAR_FEEDBACK_URL=http://127.0.0.1:8787 cargo test feedback_end_to_end -- --ignored --nocapture` – the
  application end to end; every call that would reach GitHub is printed.
- A real Compass against the local service: start it with `LOONAR_FEEDBACK_URL=http://127.0.0.1:8787`.
