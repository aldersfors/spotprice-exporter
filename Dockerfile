# Builder stages run on the target platform. Do not pin them to BUILDPLATFORM:
# that produces a binary for the build host, which breaks multi-arch images.

# Stage 1: chef base (install cargo-chef once)
FROM rust:1.100 AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

# Stage 2: planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: dependency cache
FROM chef AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 4: builder
FROM chef AS builder
COPY . .
COPY --from=cacher /app/target target
RUN cargo build --release --bin spotprice-exporter

# Stage 5: runtime
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=builder /app/target/release/spotprice-exporter /
ENTRYPOINT ["/spotprice-exporter"]
