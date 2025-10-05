# Use the official Rust image as the builder stage
FROM rust:1-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/src/lynx

# Copy the entire project
COPY . .

# Build the application
RUN cargo build --release

# Use a minimal image for the final stage
FROM debian:stable-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd --gid 1000 lynx && \
    useradd --uid 1000 --gid 1000 --shell /bin/bash --create-home lynx

# Environment variables (see .env.example for details)
ENV DATABASE_BACKEND=sqlite \
    DATABASE_URL=sqlite:///home/lynx/lynx.db \
    API_HOST=0.0.0.0 \
    API_PORT=8080 \
    REDIRECT_HOST=0.0.0.0 \
    REDIRECT_PORT=3000 \
    AUTH_MODE=none

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/lynx/target/release/lynx /opt/lynx

# Set the user
USER lynx

# Create working directory for database
WORKDIR /home/lynx

# Expose the API and redirect server ports
EXPOSE 8080 3000

# Set the entrypoint
ENTRYPOINT ["/opt/lynx"]
