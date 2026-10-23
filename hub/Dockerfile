# Compass Hub on Linux. Build from the root of the repository:
#   docker build -f hub/Dockerfile -t compass-hub .
# Run (hub.json with "listen": "0.0.0.0:8730" and the models in ./hub-data):
#   docker run -d --name compass-hub -p 8730:8730 -v "$PWD/hub-data:/data" \
#     -e LOONAR_SECRET_CLAUDE_API_KEY=... compass-hub
# Put an HTTPS reverse proxy in front of it for access from other computers.

FROM rust:1-bookworm AS build
WORKDIR /build
COPY src ./src
COPY i18n ./i18n
COPY hub ./hub
RUN cargo build --release --locked --manifest-path hub/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin compasshub
COPY --from=build /build/hub/target/release/compass-hub /usr/local/bin/compass-hub
USER 10001
WORKDIR /data
VOLUME ["/data"]
EXPOSE 8730
ENTRYPOINT ["compass-hub"]
CMD ["/data/hub.json"]
