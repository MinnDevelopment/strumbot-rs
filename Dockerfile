FROM rust:bullseye as deps

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
RUN find . -wholename "*/src/*.rs" | xargs rm -f
RUN rm -f ./target/release/deps/{libcommons*,libdatabase_api*,libdiscord_api*,strumbot*,libtwitch_api*}


FROM debian:bullseye as libs


FROM rust:bullseye as build

WORKDIR /strumbot

COPY . .

COPY --from=deps /strumbot/target/ ./target/

RUN cargo build --release


FROM gcr.io/distroless/cc-debian11

WORKDIR /app

COPY --from=libs /lib/x86_64-linux-gnu/libz.so.1 /lib/x86_64-linux-gnu/libz.so.1
COPY --from=build /strumbot/target/release/strumbot /usr/bin/strumbot

ENV RUST_LOG=info,twilight_gateway=error

CMD ["/usr/bin/strumbot"]
