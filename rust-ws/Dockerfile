# Build stage
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# Runtime stage
FROM alpine:3.21

RUN adduser -D -u 1000 app
USER app

COPY --from=builder /app/target/release/rust-ws /usr/local/bin/rust-ws

ENV WS_PORT=3001
EXPOSE 3001

CMD ["rust-ws"]
