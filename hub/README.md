# Compass Hub

The team service of Loonar Compass: the team's Enterprise Architect models (read only), a shared policy, a central audit
trail, change sets with review, a web view for people without EA, Jira and Confluence actions, Word and Excel reports,
company sign-in (OpenID Connect), a read-only GraphQL API and tables for Power BI, and an MCP server for other AI tools.
It needs no Enterprise Architect and runs on Windows or Linux.

Full documentation: the "Compass Hub" chapter of the user guide at https://loonar.eu/compass/docs/.
Step-by-step installation on Windows Server with IIS, AD FS and Group Policy: the "Compass Hub – Installation guide
for IT" (`docs/Hub_Installation_Guide.en.pdf`, Polish: `docs/Instalacja_huba.pdf`).

## Start

1. Put `hub.json` next to the program (start from `hub.example.json`). A hub without users gets the default account
   **admin / passw0rd**. Sign in at `http://127.0.0.1:8730/` and choose your own password at once.
2. Windows: `compass-hub hub.json`, or as a service (as administrator): `compass-hub --service-install C:\Hub\hub.json`.
   Remove the service with `compass-hub --service-uninstall`.
3. Linux / Docker: from the root of the repository, build the image and run it:

   ```
   docker build -f hub/Dockerfile -t compass-hub .
   docker run -d --name compass-hub -p 8730:8730 -v /srv/compass-hub:/data \
     -e LOONAR_SECRET_CLAUDE_API_KEY=... compass-hub
   ```

   - `/srv/compass-hub` holds `hub.json` (with `"listen": "0.0.0.0:8730"`), the models (.qea files) and the hub's data.
   - It must be writable by user 10001.

## Models

A source in `hub.json` points to an `app.properties` with the model: an EA file (`model.source=file`, `model.file=…`)
or a database: `model.source=postgres`, `sqlserver`, `mysql` or `oracle` with `db.host`, `db.port`, `db.name`,
`db.user` and `db.sslmode`; the password is the secret `DB_PASSWORD` (SQL Server without it: Windows authentication of the account the hub runs under). SQL Server, MySQL/MariaDB and Oracle are read
through the vendor's 64-bit ODBC driver, found automatically; `db.odbc` takes a full ODBC connection string without
the password instead. Reading is read-only (every read is rolled back). The Docker image has unixODBC: add the vendor's
Linux ODBC driver (e.g. Microsoft's `msodbcsql18`) in an image built on it.

## Secrets

The hub reads secrets from environment variables `LOONAR_SECRET_<NAME>`, or from files named in
`LOONAR_SECRET_<NAME>_FILE` (Docker secrets). On Windows it also reads them from Credential Manager.

| Secret | Needed for |
|---|---|
| `LOONAR_SECRET_CLAUDE_API_KEY` or `LOONAR_SECRET_OPENAI_API_KEY` | questions in the web view (Ask) |
| `LOONAR_SECRET_JIRA_TOKEN` | Jira issues from the web view and Jira through the hub |
| `LOONAR_SECRET_CONFLUENCE_TOKEN` | Confluence pages from the web view and Confluence through the hub |
| `LOONAR_SECRET_<NAME>` named in `"ai"` → `"keys"` | the AI gateway (one Claude key per person, role or everybody) |
| `LOONAR_SECRET_DB_PASSWORD` | models in PostgreSQL, SQL Server, MySQL or Oracle |
| `LOONAR_SECRET_OIDC_CLIENT_SECRET` | company sign-in (`"oidc"`) |

## Company sign-in (OpenID Connect)

People can sign in to the web view with their company account (AD FS, Microsoft Entra ID, Keycloak, Okta and other
OpenID Connect providers). Add to `hub.json`:

```json
"oidc": {
  "issuer": "https://adfs.example.com/adfs",
  "client_id": "<the client id from the provider>",
  "redirect_uri": "https://hub.example.com/auth/callback",
  "groups_claim": "groups",
  "roles": { "admin": ["Compass-Admins"], "architect": ["Compass-Architects"], "viewer": ["Compass-Viewers"] },
  "password_login": true
}
```

- Register the hub at the provider as a confidential web application, flow *authorization code*, with the redirect URI
  above, the scopes `openid profile email` (AD FS: the hub adds `allatclaims` itself; `"scopes"` changes them) and a `groups` claim in
  the ID token with group names. The client secret is the hub secret `OIDC_CLIENT_SECRET` (`LOONAR_SECRET_OIDC_CLIENT_SECRET`).
- The hub checks the ID token itself: RS256 signature with the provider's keys, issuer, audience, expiry, nonce; the
  sign-in uses PKCE (S256) and a one-time state.
- The role comes from the groups at every sign-in (the highest one wins). A person in none of the groups gets no access.
  Without `roles` the groups are Compass-Admins, Compass-Architects and Compass-Viewers. `groups_claim` names the claim
  (default `groups`).
- `name_claim` chooses the user name (default: `upn`, then `preferred_username`, `email`). A local user of the hub with
  the same name is refused until the administrator renames or removes it.
- `"password_login": false` turns off sign-in with a name and a password; API tokens for Compass and MCP clients still work.
- The web view shows the button "Sign in with your company account" (`"button"` changes its text).
- Every company sign-in is in the audit trail.

## Team policy

`"policy"` in `hub.json`: `min_approval`, `governance` (the model check rules), `event_agents` (the team's event agents
in the format of the `"agents"` list of `event_agents.json`, guards too; marked "(team)" in everybody's Compass in EA),
`ask_min_role` and `ask_per_hour`. Admins edit it in the **Team policy** tab of the web view.

More keys of the policy for the Compass apps connected to the hub:

| Key | Effect in Compass |
|---|---|
| `web_access` | `false`: no web search and no reading of pages |
| `web_allowed_domains` | the only domains Compass may read pages from |
| `update_check` | `false`: no daily check for a new version |
| `ai_via_hub` | `true`: questions go through the AI gateway of the hub (no key on the computers) |
| `jira_via_hub`, `confluence_via_hub` | `true`: Jira and Confluence go through the hub (its connection and token) |

## AI gateway

With `"ai"` in `hub.json` the hub keeps the Claude API keys and Compass asks through it (`POST /ai/v1/messages`, the
Anthropic Messages API; the hub token of the signed-in person is the key). Nobody needs a key on their computer.

```json
"ai": {
  "keys": [
    { "for": "anna@example.com", "secret": "claude_key_anna" },
    { "for": "role:architect", "secret": "claude_key_team" },
    { "for": "*", "secret": "claude_key_team" }
  ],
  "limits": [
    { "for": "anna@example.com", "tokens_per_day": 1000000 },
    { "for": "*", "tokens_per_day": 2000000, "tokens_per_month": 30000000 }
  ],
  "models": ["claude-haiku-5-5", "claude-sonnet-5-5"]
}
```

- `keys`: which key a person uses. The first match wins in this order: the user name, `role:<role>`, `*`. A person
  without a matching entry gets "the hub has no AI key for you". `secret` names a hub secret
  (`LOONAR_SECRET_CLAUDE_KEY_ANNA`, a file in `LOONAR_SECRET_CLAUDE_KEY_ANNA_FILE` or Credential Manager).
- `limits`: tokens per day and per month (input + output; tokens read from the prompt cache count a tenth). Over the
  limit the hub answers 429 with a message that Compass shows.
- `models`: the models allowed (no list = any).
- Every request is in `ai_usage.jsonl` (person, key, model, tokens, status). Admins see the totals per person in
  **Users → AI usage**.
- With the policy key `ai_via_hub` Compass uses the gateway after the person signs in with the company account
  (Settings → Team).

## Jira and Confluence through the hub

With `"integrations"` (`jira`: `url`, `user`, `project`, `issue_type`; `confluence`: `url`, `user`, `space`, `parent`)
and the secrets `JIRA_TOKEN` and `CONFLUENCE_TOKEN`, the hub forwards Compass requests to the Atlassian REST API:
`/proxy/jira/rest/…` and `/proxy/confluence/rest/…`. Compass uses them with the policy keys `jira_via_hub` and
`confluence_via_hub`, so no Atlassian token is needed on the computers; the project and the space come from the hub.

- Only REST paths are forwarded.
- A viewer may only read (GET); other methods get 403 "a viewer only reads".
- Every change (POST, PUT, DELETE) is in the audit trail with the person's name.

## Reports, GraphQL and Power BI

- **Reports:** in the web view, pick a package and open **Model check**: "Word report of the package" and "Excel report
  of the package" (description, elements, relationships; no diagram pictures). Every download is audited.
- **GraphQL (read-only):** `POST /api/sources/{id}/graphql` with `{"query": "…"}`: `packages`, `package`,
  `elements(name, type, stereotype, package, limit)`, `element`, `diagrams`, `diagram`; elements have `attributes`,
  `operations`, `taggedValues`, `connectors` (with `source` and `target`) and `package`. Schema (SDL):
  `GET /api/graphql/schema`.
- **Tables:** `GET /api/sources/{id}/table/{elements|connectors|packages|diagrams|attributes|tagged_values}` as JSON.
  Power BI: Get Data → Web → Advanced, header `Authorization: Bearer <API token>`.
- Any signed-in role may use them, with an API token from the **Users** tab.

## Security

- **HTTPS:** the hub speaks plain HTTP. For access from other computers, put it behind an HTTPS reverse proxy (IIS, nginx).
- **IIS with ARR:** listen on `127.0.0.1:8730`, rewrite the IIS site to `http://127.0.0.1:8730/{R:1}`, and turn off
  `reverseRewriteHostInResponseHeaders` in the ARR proxy settings (otherwise ARR rewrites the redirect to the identity
  provider to the hub's own address and the company sign-in fails). Keep the `Authorization` header (the default).
- **Client address behind a proxy:** `X-Forwarded-For` is trusted only from a proxy on the same computer (its last address
  is the client); the lockout and the audit trail use that address.
- **Sign-ins in the audit trail:** failed and refused sign-ins (password and company account) are recorded with result ✗
  and the client address. The **Users** tab also lists company accounts with their role and last sign-in (read only).
- **hub.json:** protect it with file permissions. Passwords and tokens are kept in it only as hashes.
- **Default account:** it can do nothing until its password is changed.
- **Lockout:** after 20 wrong passwords or 20 different wrong tokens within 10 minutes, an address is refused for
  10 minutes (a client repeating one old token counts once; valid tokens still work).
