FROM rust:1.95-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --locked --release -p orchester-server

FROM debian:bookworm-slim
# The system's certificate authorities: with proxy_certificate = "auto" the server checks its proxy's certificate
# against them, as an app does.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /data && chown 10001:10001 /data
COPY --from=build /src/target/release/orchester-server /usr/local/bin/orchester-server
USER 10001:10001
ENV ORCHESTER_SERVER_LISTEN=0.0.0.0:8080 ORCHESTER_SERVER_DATA_DIR=/data
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["orchester-server"]
CMD ["serve"]
