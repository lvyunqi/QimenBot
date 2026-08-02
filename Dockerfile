# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS admin-builder
WORKDIR /src/web/admin
COPY web/admin/package.json web/admin/package-lock.json ./
RUN npm ci
COPY web/admin/ ./
RUN npm run build

FROM rust:1.89-bookworm AS rust-builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps/ apps/
COPY crates/ crates/
COPY plugins/qimen-plugin-example/ plugins/qimen-plugin-example/
COPY web/admin/ web/admin/
COPY --from=admin-builder /src/web/admin/dist web/admin/dist
RUN cargo build --release --locked --package qimenbotd

FROM debian:bookworm-slim AS runtime
ARG QIMEN_VERSION=dev
ENV QIMEN_VERSION=${QIMEN_VERSION} \
    QIMEN_CONFIG_PATH=/data/config/base.toml \
    QIMEN_DEPLOYMENT=docker \
    RUST_LOG=info

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl gosu tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /home/qimenbot qimenbot \
    && mkdir -p /opt/qimenbot/defaults /data/config /data/plugins/bin /data/logs \
    && chown -R qimenbot:qimenbot /opt/qimenbot /data

COPY --from=rust-builder /src/target/release/qimenbotd /usr/local/bin/qimenbotd
COPY deploy/docker/base.toml.example /opt/qimenbot/defaults/base.toml.example
COPY config/plugin-state.toml /opt/qimenbot/defaults/plugin-state.toml
COPY deploy/docker/entrypoint.sh /usr/local/bin/qimen-entrypoint
RUN chmod 0755 /usr/local/bin/qimen-entrypoint /usr/local/bin/qimenbotd

WORKDIR /data
EXPOSE 3210 6701 8088
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3210/healthz || exit 1
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/qimen-entrypoint"]
CMD ["qimenbotd"]
