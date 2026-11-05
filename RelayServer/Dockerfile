FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto proto
COPY src src
RUN cargo build --release --locked

FROM ubuntu:24.04
RUN useradd --system --home /data --shell /usr/sbin/nologin relay \
    && mkdir /data && chown relay:relay /data
COPY --from=build /src/target/release/relay-server /usr/local/bin/relay-server
USER relay
WORKDIR /data
ENV DATA_DIR=/data
EXPOSE 21115 21116 21116/udp 21117 21118 21119
VOLUME /data
HEALTHCHECK --interval=15s --timeout=5s CMD bash -c '</dev/tcp/127.0.0.1/21116' || exit 1
ENTRYPOINT ["relay-server"]
