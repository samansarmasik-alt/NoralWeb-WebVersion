# ---- derleme ----
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
COPY ui ./ui
# No GUI deps needed (ureq+axum are pure Rust) — fast build, no webkit.
RUN cargo build --release

# ---- koşma ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists
COPY --from=build /app/target/release/noral-web-server /app/server
WORKDIR /app
# Render injects $PORT; default 10000 for local runs.
ENV PORT=10000
EXPOSE 10000
CMD ["/app/server"]
