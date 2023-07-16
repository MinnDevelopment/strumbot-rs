FROM rust:latest as deps

WORKDIR /strumbot

RUN USER=root cargo new --lib commons
RUN USER=root cargo new --lib database-api
RUN USER=root cargo new --lib discord-api
RUN USER=root cargo new --bin strumbot
RUN USER=root cargo new --lib twitch-api

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./commons/Cargo.toml ./commons/Cargo.toml
COPY ./database-api/Cargo.toml ./database-api/Cargo.toml
COPY ./discord-api/Cargo.toml ./discord-api/Cargo.toml
COPY ./strumbot/Cargo.toml ./strumbot/Cargo.toml
COPY ./twitch-api/Cargo.toml ./twitch-api/Cargo.toml

RUN cargo build --release
RUN rm **/*.rs
RUN rm ./target/release/deps/strumbot*


FROM debian:buster as libs


FROM rust:latest as build

WORKDIR /strumbot

RUN mkdir -p ./target/release/deps
COPY --from=deps /strumbot/target/ ./target/
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./commons ./commons
COPY ./database-api ./database-api
COPY ./discord-api ./discord-api
COPY ./strumbot ./strumbot
COPY ./twitch-api ./twitch-api

RUN cargo build --release


FROM gcr.io/distroless/cc

WORKDIR /app

COPY --from=libs /lib/x86_64-linux-gnu/libz.so.1 /lib/x86_64-linux-gnu/libz.so.1
COPY --from=build /strumbot/target/release/strumbot /usr/bin/strumbot

ENV RUST_LOG=info,twilight_gateway=error

CMD ["/usr/bin/strumbot"]
