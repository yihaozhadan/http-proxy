FROM rust:1.75-slim AS builder

WORKDIR /usr/src/app
COPY . .

# Install OpenSSL development packages
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

RUN cargo build --release

FROM debian:bookworm-slim

# Install OpenSSL runtime library
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin
COPY --from=builder /usr/src/app/target/release/http-proxy .

# These environment variables can be overridden at runtime
ENV TARGET_URL=https://httpbin.org/post
ENV SUCCESS_PROBABILITY=0.8

EXPOSE 3000

CMD ["./http-proxy"]
