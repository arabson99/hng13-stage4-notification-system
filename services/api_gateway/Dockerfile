# -------- Builder --------
    FROM rustlang/rust:nightly as builder
    WORKDIR /app
    
    # cache deps
    COPY Cargo.toml Cargo.lock ./
    RUN mkdir -p src && echo 'fn main(){}' > src/main.rs
    RUN cargo build --release || true
    
    # build
    COPY src ./src
    RUN cargo build --release
    
    # -------- Runtime --------
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
    ENV RUST_LOG=info
    WORKDIR /app
    COPY --from=builder /app/target/release/api_gateway /usr/local/bin/api_gateway
    EXPOSE 8080
    CMD ["api_gateway"]
    