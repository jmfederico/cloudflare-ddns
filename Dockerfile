# Use the official Rust image as the base image for building
FROM rust:1.85 AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files first to leverage Docker layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this will be cached if Cargo.toml doesn't change)
RUN cargo build --release && rm -rf src/

# Remove the dummy source and target binary to force rebuild with actual source
RUN rm -rf src target/release/deps/cloudflare_ddns* target/release/cloudflare-ddns*

# Copy the actual source code
COPY src ./src

# Build the application
RUN cargo build --release

# Use a minimal runtime image
FROM debian:bookworm-slim

# Install CA certificates for HTTPS requests
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user for security
RUN useradd -r -s /bin/false cloudflare-ddns

# Set the working directory
WORKDIR /app

# Copy the binary and repeat script from the builder stage
COPY --from=builder /usr/src/app/target/release/cloudflare-ddns /app/cloudflare-ddns
COPY repeat-cloudflare-ddns /app/repeat-cloudflare-ddns

# Make the repeat script executable and change ownership
RUN chmod +x /app/repeat-cloudflare-ddns && \
    chown cloudflare-ddns:cloudflare-ddns /app/cloudflare-ddns /app/repeat-cloudflare-ddns

# Switch to the non-root user
USER cloudflare-ddns

# Set the entrypoint
CMD ["/app/repeat-cloudflare-ddns"]
