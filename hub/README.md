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

## Security

- **HTTPS:** the hub speaks plain HTTP. For access from other computers, put it behind an HTTPS reverse proxy (IIS, nginx).
- **hub.json:** protect it with file permissions. Passwords and tokens are kept in it only as hashes.
- **Default account:** it can do nothing until its password is changed.
- **Lockout:** after 20 wrong passwords or tokens, an address is refused for 10 minutes.
