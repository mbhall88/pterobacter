# syntax=docker/dockerfile:1
# Multi-platform base digests are pinned; the release gate currently tests linux/amd64.
FROM rust:slim-bookworm@sha256:a24e292b4af7bdee351a31d3a6379972a5652f0ea22a1b6d6883324ac4f5097c AS build
RUN apt-get update && apt-get install -y --no-install-recommends g++ make pkg-config liblzma-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /source
COPY . .
ENV CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
RUN cargo build --release --locked --bin ptero --bin pterobacter

# CI exports only synthetic inputs from this stage, never into the runtime image.
FROM build AS fixtures
RUN cargo run --release --locked --example local_database -- /fixture

FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587 AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libstdc++6 libgcc-s1 liblzma5 tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ptero && useradd --uid 10001 --gid 10001 --create-home ptero \
    && mkdir /database /work && chown 10001:10001 /database /work
COPY --from=build /source/target/release/ptero /source/target/release/pterobacter /usr/local/bin/
COPY LICENSE LICENSE-GPL-3.0.txt THIRD-PARTY-LICENSES.md THIRD-PARTY-NOTICES.txt THIRDPARTY.json /usr/share/doc/pterobacter/
ARG VERSION=development
ARG REVISION=unknown
LABEL org.opencontainers.image.title="Pterobacter" \
      org.opencontainers.image.description="Classify Enterobacter isolate assemblies" \
      org.opencontainers.image.source="https://github.com/mbhall88/pterobacter" \
      org.opencontainers.image.documentation="https://mbhall88.github.io/pterobacter/containers.html" \
      org.opencontainers.image.licenses="GPL-3.0-or-later" \
      org.opencontainers.image.version=$VERSION \
      org.opencontainers.image.revision=$REVISION
ENV HOME=/home/ptero XDG_DATA_HOME=/database XDG_STATE_HOME=/work/.ptero-state
USER 10001:10001
WORKDIR /work
ENTRYPOINT ["/usr/bin/tini", "--", "ptero"]
CMD ["--help"]
