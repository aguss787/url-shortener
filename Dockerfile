FROM rust:1.80.0 AS api_builder

RUN ["mkdir", "-p", "/opt/app"]
WORKDIR /opt/app

COPY . .
RUN ["cargo", "build", "--release"]

WORKDIR /opt/app/migration
RUN ["cargo", "build", "--release"]

FROM ubuntu:latest
LABEL authors="agus"

RUN ["apt-get", "update"]
RUN ["apt-get", "install", "-y", "libpq5"]

RUN ["mkdir", "-p", "/opt/app"]
WORKDIR /opt/app

COPY --from=api_builder /opt/app/target/release/url-shortener /opt/app/
COPY --from=api_builder /opt/app/migration/target/release/migration /opt/app/

CMD ["/opt/app/url-shortener"]
