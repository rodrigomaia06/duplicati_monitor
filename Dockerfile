# Define arguments for app metadata
ARG APP_VERSION=0.1.0
ARG MAINTAINER="Rodrigo Maia <rodrigo.m.t.maia@gmail.com>"

# Stage 1: Build the Rust application
FROM rust:1.83.0-bullseye AS builder

# Set metadata labels
LABEL maintainer=${MAINTAINER}
LABEL version=${APP_VERSION}
LABEL description="A Dockerized Duplicati Monitor written in Rust."

# Set the working directory
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

# Build the application in release mode
RUN cargo build --release

# Stage 2: Create a minimal runtime image
FROM debian:12-slim

# Set metadata labels
LABEL maintainer=${MAINTAINER}
LABEL version=${APP_VERSION}
LABEL description="A minimal runtime environment for duplicati monitor."

# Set environment variables
ENV APP_VERSION=${APP_VERSION}

# Install required libraries
RUN apt-get update && apt-get install -y \
    libssl3 \
    libgcc-s1 \
    libc6 \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Copy the built application from the builder stage
COPY --from=builder /app/target/release/duplicati_monitor /app/

# Expose the port (choose a port that’s not commonly used)
EXPOSE 5050

# Run the application
CMD ["/app/duplicati_monitor"]
