# mqtt-actor [![CI](https://github.com/DanNixon/mqtt-actor/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/DanNixon/mqtt-actor/actions/workflows/ci.yml)

Simple tool to schedule MQTT messages.

Rather handy for driving event schedule announcements and other similar tasks.

## Usage

TL;DR: see `mqtt-actor --help` and the [examples](./examples).

A "script" is generated from several script fragments, see [examples](./examples) for some examples.
The general format is as such: `[timestamp] [delimiter] [topic] [delimiter] [message]`.

`[delimiter]` defaults to the pipe (`|`), but can be configured via the command line options.

`[timestamp]` can be either an absolute timestamp, in either RFC2822 or RFC3339 format or a relative timestamp.
Relative timestamps are relative to the last message in the current file, unless it is the first message in the file, in which case it is relative to the time the script was loaded.
The actor watches the script source directory and will reload the script when it detects that a script source file *might* have changed.

## Deployment

For testing and small/temporary deployments, Podman (or Docker if you really must) can be used:
```sh
podman run \
  --rm -it \
  -e RUST_LOG=debug \
  -e MQTT_BROKER=broker.hivemq.com \
  -v ./examples/:/config \
  ghcr.io/dannixon/mqtt-actor:main
```

For more serious deployements, Kubernetes is a better option.
A barebones deployment is <TODO>.
