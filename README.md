# udd

`udd` is a small UDP client (cli and tui) written in Rust, designed for manual testing.

This project also has basic [uqtt](https://github.com/Jezza/uqtt) support.

## Prerequisites

- Rust toolchain (stable)

## Build

```bash
cargo build
```

## Run

CLI mode:

```bash
cargo run -- <target_host:port>
```

TUI mode:

```bash
cargo run -- <target_host:port> --tui
```

Optional bind address:

```bash
cargo run -- <target_host:port> --bind 0.0.0.0:0
```

## License

MIT-style license text is in `LICENSE`.
