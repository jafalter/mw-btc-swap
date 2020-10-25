FROM rust:1-buster
RUN mkdir /app
RUN mkdir /build
RUN mkdir /slates
WORKDIR /build
COPY Cargo.toml /build
COPY src /build/src
RUN cargo build --release --target-dir /app
COPY config /app/release/config
WORKDIR /app
CMD tail -f /dev/null