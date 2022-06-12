FROM rust:1.61 as deps

RUN USER=root cargo new --bin strumbot
WORKDIR /strumbot

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs
RUN rm ./target/release/deps/strumbot*



FROM rust:1.61 as build

WORKDIR /strumbot

RUN mkdir -p ./target/release/deps
COPY --from=deps /strumbot/target/ ./target/
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo build --release



FROM gcr.io/distroless/cc

WORKDIR /app

COPY --from=build /strumbot/target/release/strumbot /usr/bin/strumbot

CMD ["/usr/bin/strumbot"]