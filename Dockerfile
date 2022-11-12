FROM rust:latest as deps

RUN USER=root cargo new --bin strumbot
WORKDIR /strumbot

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs
RUN rm ./target/release/deps/strumbot*


FROM debian:buster as libs


FROM rust:latest as build

WORKDIR /strumbot

RUN mkdir -p ./target/release/deps
COPY --from=deps /strumbot/target/ ./target/
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo build --release


FROM gcr.io/distroless/cc

WORKDIR /app

COPY --from=libs /lib/x86_64-linux-gnu/libz.so.1 /lib/x86_64-linux-gnu/libz.so.1
COPY --from=build /strumbot/target/release/strumbot /usr/bin/strumbot

ENV RUST_LOG=info,twilight_gateway=error

CMD ["/usr/bin/strumbot"]
