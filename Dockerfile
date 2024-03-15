FROM docker.io/library/rust:1.76.0-bullseye AS build

ARG DATABASE_URL

WORKDIR /app

COPY Cargo.toml Cargo.lock .
COPY models/Cargo.toml models/Cargo.toml
COPY sessions/Cargo.toml sessions/Cargo.toml

RUN mkdir models/src \
    && touch models/src/lib.rs \
    && mkdir sessions/src \
    && echo "fn main() {}" > sessions/src/main.rs \
    && cargo build --release

COPY . /app

RUN touch models/src/lib.rs \
    && touch sessions/src/main.rs \
    && cargo build --release

FROM gcr.io/distroless/cc AS deploy

COPY --from=build /app/target/release/sessions /sessions

ENTRYPOINT ["/sessions"]
