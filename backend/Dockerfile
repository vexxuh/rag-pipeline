FROM rust:1.93 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY src ./src
COPY config ./config

RUN touch src/main.rs src/lib.rs
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/rag-backend /app/rag-backend
COPY --from=builder /app/config /app/config

RUN mkdir -p /app/data

ENV RUN_ENV=production
EXPOSE 3000

CMD ["/app/rag-backend"]
