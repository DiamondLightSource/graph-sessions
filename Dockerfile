FROM docker.io/library/rust:1.76.0-bullseye AS build

WORKDIR /app

COPY Cargo.toml Cargo.lock .

RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release

COPY . /app

RUN touch src/main.rs \
    && cargo build --release

FROM gcr.io/distroless/cc AS deploy

COPY --from=build /app/target/release/sessions /sessions

ENTRYPOINT ["/sessions"]
