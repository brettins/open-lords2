# l2-relay, the join-code relay (docs/netcode.md §7, C254). Two stages: the
# workspace builds in rust:1-bookworm, the binary alone ships on bookworm-slim.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p l2-relay

FROM debian:bookworm-slim
COPY --from=build /src/target/release/l2-relay /usr/local/bin/l2-relay
USER nobody
EXPOSE 4714
ENTRYPOINT ["/usr/local/bin/l2-relay", "--listen", "0.0.0.0:4714"]
