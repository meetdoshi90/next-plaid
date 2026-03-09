# NextPlaid

High-performance multi-vector search built in Rust.

## Credits

This project is a fork of [NextPlaid](https://github.com/lightonai/next-plaid), originally developed by [LightOn](https://www.lighton.ai/). Original work licensed under Apache-2.0.

## Packages

| Package | Description |
| --- | --- |
| [`next-plaid`](./next-plaid) | Core PLAID index |
| [`next-plaid-api`](./next-plaid-api) | REST API server |

## Quick Start
```bash
# Build and run the API server
cargo build --release -p next-plaid-api
cargo run --release -p next-plaid-api -- --index-dir /tmp/indices --port 8080
```

See [next-plaid-api/README.md](./next-plaid-api/README.md) for full documentation.

## License

Apache-2.0