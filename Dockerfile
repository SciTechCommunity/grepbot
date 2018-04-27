# Compile in one docker container, but deploy in a smaller one
FROM rust as build

RUN mkdir -p /usr/src/app
WORKDIR /usr/src/app
COPY src/ src/
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo build --release

# Smaller container to deploy with
FROM debian
RUN apt-get update
RUN apt-get install -y openssl
RUN apt-get install -y ca-certificates

# Where to put the application
RUN mkdir /application
WORKDIR /application
COPY --from=build /usr/src/app/target/release/grepbot .

# Setup for logs and data storage
RUN mkdir /var/lib/grepbot

ENV STORAGE_FILE=/var/lib/grepbot/data

# ...aaaaand Go!
ENTRYPOINT /application/grepbot
