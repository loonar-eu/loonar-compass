# Compass Hub

The team service of Loonar Compass: the team's Enterprise Architect models (read only), a shared policy, a central audit
trail, change sets with review, a web view for people without EA, Jira and Confluence actions, and an MCP server for
other AI tools. It needs no Enterprise Architect and runs on Windows or Linux.

Full documentation: the "Compass Hub" chapter of the user guide at https://loonar.eu/docs/.

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

## Secrets

The hub reads secrets from environment variables `LOONAR_SECRET_<NAME>`, or from files named in
`LOONAR_SECRET_<NAME>_FILE` (Docker secrets). On Windows it also reads them from Credential Manager.

| Secret | Needed for |
|---|---|
| `LOONAR_SECRET_CLAUDE_API_KEY` or `LOONAR_SECRET_OPENAI_API_KEY` | questions in the web view (Ask) |
| `LOONAR_SECRET_JIRA_TOKEN` | Jira issues from the web view |
| `LOONAR_SECRET_CONFLUENCE_TOKEN` | Confluence pages from the web view |
| `LOONAR_SECRET_DB_PASSWORD` | models in PostgreSQL |

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
  above, the scopes `openid profile email` (AD FS: also `allatclaims`) and a `groups` claim in the ID token with group
  names. The client secret is the hub secret `OIDC_CLIENT_SECRET` (`LOONAR_SECRET_OIDC_CLIENT_SECRET`).
- The hub checks the ID token itself: RS256 signature with the provider's keys, issuer, audience, expiry, nonce; the
  sign-in uses PKCE (S256) and a one-time state.
- The role comes from the groups at every sign-in (the highest one wins). A person in none of the groups gets no access.
  `name_claim` chooses the user name (default: `upn`, then `preferred_username`, `email`).
- `"password_login": false` turns off sign-in with a name and a password; API tokens for Compass and MCP clients still work.
- Every company sign-in is in the audit trail.

## Security

- **HTTPS:** the hub speaks plain HTTP. For access from other computers, put it behind an HTTPS reverse proxy (IIS, nginx).
- **hub.json:** protect it with file permissions. Passwords and tokens are kept in it only as hashes.
- **Default account:** it can do nothing until its password is changed.
- **Lockout:** after 20 wrong passwords or tokens, an address is refused for 10 minutes.
