FROM rust:1.75-slim

WORKDIR /app

COPY . .
RUN cargo build --release

EXPOSE 8000

CMD ["./target/release/lfv"]
