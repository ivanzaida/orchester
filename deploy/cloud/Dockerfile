FROM rust:1.95-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --locked --release -p orchester-cloud

FROM debian:bookworm-slim
# The trust store for TLS to the SMTP relay and to a PostgreSQL across a network.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/orchester-cloud /usr/local/bin/orchester-cloud
USER 10001:10001
ENV ORCHESTER_CLOUD_LISTEN=0.0.0.0:8090
EXPOSE 8090
ENTRYPOINT ["orchester-cloud"]
CMD ["serve"]
