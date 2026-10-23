# yappa-control as a container (spike S6). Built from the repo root:
#   docker build -f infra/control/Dockerfile -t yappa-control .
# home.yml / vps.yml build it this way; the SQLite database lives in the /data volume.

FROM rust:1.97-slim-bookworm AS build
WORKDIR /src
COPY . .
# Only the control plane and its dependencies — no Tauri, no audio, no LiveKit client.
RUN cargo build --release -p control --bin yappa-control

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --home /data yappa \
 && mkdir -p /data && chown yappa /data
COPY --from=build /src/target/release/yappa-control /usr/local/bin/yappa-control
USER yappa
ENV YAPPA_DATABASE_URL=sqlite:///data/yappa.db \
    YAPPA_LISTEN=127.0.0.1:8090 \
    YAPPA_LOG_JSON=1
VOLUME /data
ENTRYPOINT ["yappa-control"]
CMD ["serve"]
