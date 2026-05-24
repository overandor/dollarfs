# Deployment Guide

## Prerequisites

- Rust 1.75+
- macOS

## Local Development

```bash
cargo build --release
./target/release/lfv init
```

## Docker Deployment

```bash
docker build -t dollarfs .
docker run -p 8000:8000 dollarfs
```

## Binary Distribution

```bash
cargo build --release
cp target/release/lfv /usr/local/bin/
```
