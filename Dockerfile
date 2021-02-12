FROM rust:1-buster
RUN apt-get update -y
RUN apt-get upgrade -y
RUN mkdir /app
RUN mkdir /slates
COPY target/ /app
COPY config /app/release/config
WORKDIR /app
CMD tail -f /dev/null