# Compass Hub on Linux. Build from the root of the repository:
#   docker build -f hub/Dockerfile -t compass-hub .
# Run (hub.json with "listen": "0.0.0.0:8730" and the models in ./hub-data):
#   docker run -d --name compass-hub -p 8730:8730 -v "$PWD/hub-data:/data" \
#     -e LOONAR_SECRET_CLAUDE_API_KEY=... compass-hub
# Put an HTTPS reverse proxy in front of it for access from other computers.
# A SQL Server, MySQL or Oracle model needs the vendor's ODBC driver for Linux in an image built on this one
# (e.g. Microsoft's msodbcsql18 package); PostgreSQL and EA files need nothing more.

FROM rust:1-bookworm AS build
# unixODBC: SQL Server, MySQL and Oracle repositories are read through ODBC.
RUN apt-get update && apt-get install -y --no-install-recommends unixodbc-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY src ./src
COPY i18n ./i18n
COPY hub ./hub
RUN cargo build --release --locked --manifest-path hub/Cargo.toml

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="Compass Hub" org.opencontainers.image.source="https://github.com/loonar-eu/loonar-compass" org.opencontainers.image.description="Team service of Loonar Compass: EA models, policy, audit trail, change sets, web view, MCP server"
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 unixodbc \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin compasshub
COPY --from=build /build/hub/target/release/compass-hub /usr/local/bin/compass-hub
COPY hub/THIRD-PARTY-NOTICES-linux.txt /usr/share/doc/compass-hub/THIRD-PARTY-NOTICES.txt
COPY LICENSE.en.txt /usr/share/doc/compass-hub/LICENSE.txt
USER 10001
WORKDIR /data
VOLUME ["/data"]
EXPOSE 8730
ENTRYPOINT ["compass-hub"]
CMD ["/data/hub.json"]
