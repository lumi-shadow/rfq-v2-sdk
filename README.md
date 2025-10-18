# Market Maker Client SDK

Rust SDK for Jupiter RFQ market maker integration via gRPC streaming.

## Features

- **Bidirectional streaming** - Send quotes and receive swap requests simultaneously
- **Automatic sync** - Sequence number synchronization on connect
- **Transaction signing** - Sign and submit Solana transactions
- **Production ready** - HTTP/2 with TLS, connection health monitoring

## Quick Start

Set environment variables:
- `MM_MAKER_ID` - Your maker identifier
- `MM_AUTH_TOKEN` - JWT authentication token
- `SOLANA_PRIVATE_KEY` - Base58 encoded private key
- `BIRDEYE_API_KEY` - Birdeye API key (optional)

Run the example:
```bash
cargo run --example production_streaming
```

## Architecture

- **Client** - Manages gRPC connection with HTTP/2 and TLS
- **QuoteStreamHandle** - Bidirectional quote streaming with stats
- **SwapStreamHandle** - Bidirectional swap streaming with keepalive
- **Builders** - Type-safe quote construction with validation
- **Statistics** - Connection health and message tracking

## Known Issues

- Proto requires `auth_token` in every quote despite stream-level authentication
- Consider making `auth_token` field optional in future proto versions

## Requirements

- Rust 1.70+
- Tokio async runtime
- Solana keypair for swap signing

## License

MIT
