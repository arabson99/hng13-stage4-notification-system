# build
FROM rust:1.90 as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# run
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/api_gateway /app/api_gateway
ENV RUST_LOG=info
EXPOSE 8080
ENTRYPOINT ["/app/api_gateway"]
