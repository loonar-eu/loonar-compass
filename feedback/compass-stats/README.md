# Loonar Compass – usage statistics

Anonymous daily usage summaries sent by [Loonar Compass](https://loonar.eu), published openly so that everybody can
see exactly what is collected. **[SUMMARY.md](SUMMARY.md)** shows the last 30 days and is rebuilt every day.

## What one summary contains

One file per installation and day: `data/YYYY/MM/<day>/<installation id>.json`.

```json
{
  "v": 1,
  "install": "7f3c2b1a-1234-4abc-8def-0123456789ab",
  "day": "2026-10-24",
  "app": "0.9.0",
  "os": "Windows 11 (26200)",
  "lang": "pl",
  "setup": { "mode": "ea_tab", "model_source": "file", "ai_provider": "anthropic", "ai_model": "claude-haiku-5-5",
             "approval": "ask", "jira": true, "confluence": false, "hub": false, "mcp_servers": 2, "event_agents": 1 },
  "turns": 14,
  "ai_errors": 1,
  "tools": { "ea_write": 5, "db_query": 22, "mcp": 3 },
  "tool_errors": { "ea_write": 1 },
  "approvals": { "asked": 6, "accepted": 5, "rejected": 1 },
  "views": { "chat": 3, "statistics": 1 },
  "events": { "session_ea_tab": 2 },
  "tokens": { "in": 120000, "out": 9000 }
}
```

- `install` is a random id created by the application. It joins the days of one installation and nothing else; it can
  be renewed in the application at any time.
- Counts only: which functions of the assistant were used and how often they failed, answers, errors of the AI
  service, approvals, views opened, tokens, and the kind of setup.
- **Never** sent: the content of conversations, names or ids from the model, file paths, server addresses, error
  messages, user names. Tools of the user's own MCP servers are counted together as `mcp`; a custom AI model name is
  `other`. The receiving service refuses any field not listed above.

## Switching it off

In the application: **Usage statistics** → untick "Send anonymous usage statistics". For a whole organisation: the
environment variable `LOONAR_TELEMETRY=0`, the registry value `Telemetry` = 0 (DWORD) under
`HKLM\SOFTWARE\Policies\Loonar\Compass`, or `"telemetry": false` in the policy of the team hub.

Privacy policy: https://loonar.eu/privacy/
