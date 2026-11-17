# spotprice-exporter

Exports day-ahead electricity spot prices per bidding zone as OpenTelemetry metrics, pushed over OTLP.

Status: early development. See [BACKLOG.md](BACKLOG.md) for the plan and [docs/adr](docs/adr) for decisions.

## Development

```sh
make all          # check, test, clippy, fmt-check
make docker-build # local container image
```

Toolchain: Rust 1.100, pinned in `rust-toolchain.toml`.

## Licence

To be decided (see BACKLOG.md, open questions).
