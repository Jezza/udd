# udd

`udd` is a small UDP client (single-shot CLI and TUI) written in Rust, designed for manual testing.

This project also has basic [uqtt](https://github.com/Jezza/uqtt) support.

## Prerequisites

- Rust toolchain (stable)

## Build

```bash
cargo build
```

## Run

CLI mode (single command, non-interactive):

```bash
cargo run -- <target_host:port> --mode auto "hello\\nworld"
cargo run -- <target_host:port> --mode hex deadbeef
cargo run -- <target_host:port> --mode mqtt "connect id1 keepalive=30"
```

TUI mode:

```bash
cargo run -- <target_host:port> --tui
```

Optional bind address:

```bash
cargo run -- <target_host:port> --bind 0.0.0.0:0
```

`--mode auto` matches TUI auto mode behavior: try MQTT command, then hex, then text-with-escapes.

## License

MIT-style license text is in `LICENSE`.
