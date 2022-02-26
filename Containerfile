FROM docker.io/library/rust:alpine3.15 as builder

RUN apk add \
  cmake \
  libc-dev \
  make \
  openssl-dev

COPY . .
RUN RUSTFLAGS=-Ctarget-feature=-crt-static cargo install \
  --path . \
  --root /usr/local

FROM docker.io/library/alpine:3.15

RUN apk add \
  libgcc

COPY --from=builder \
  /usr/local/bin/mqtt-actor \
  /usr/local/bin/mqtt-actor

RUN mkdir /config

ENTRYPOINT ["/usr/local/bin/mqtt-actor"]
CMD ["/config"]
