FROM rust:1.85-bullseye AS builder

LABEL org.opencontainers.image.authors="vokup <vokup@makeany.app>"

WORKDIR /src
COPY Cargo.lock .
COPY Cargo.toml .
COPY src/       src/
RUN cargo build --bin grpc-server --release

FROM debian:bullseye-slim AS runner
WORKDIR /app
COPY --from=builder /src/target/release/grpc-server .
ENTRYPOINT ["/app/grpc-server"]