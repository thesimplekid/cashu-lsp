# Lightning Service Provider with LDK and NUT-18 Payment Requests

A modular Lightning Service Provider (LSP) implementation that uses CDK (Conductor Development Kit) to create NUT-18 payment requests for accepting Lightning Network channel payments, integrated with Lightning Development Kit (LDK) for node functionality.

## Overview

This LSP implementation allows Lightning Network node operators to offer liquidity-as-a-service to users by:

1. Accepting payment for inbound liquidity via NUT-18 payment requests
2. Automatically opening channels to users upon payment confirmation
3. Providing a simple API for channel management and LSP operations

The solution leverages the robust LDK (Lightning Development Kit) for core Lightning Network functionality with the CDK (Conductor Development Kit) for creating payment requests used in the channel funding process.

## Features

- **NUT-18 Payment Request Generation**: Create standardized payment requests through CDK
- **LDK Node Integration**: Full Lightning node functionality via LDK
- **REST API**: Simple HTTP API for interacting with the LSP
- **Customizable Fee Structure**: Configure your channel opening fees and policies
- **Simple Deployment**: Easy setup and configuration

## Configuration

The LSP is configured through a `config.toml` file with the following sections:

### Bitcoin Configuration
```toml
[bitcoin]
network = "regtest"  # Options: "bitcoin", "testnet", "signet", "regtest"
rpc_host = "127.0.0.1"
rpc_port = 18443
rpc_user = "testuser"
rpc_password = "testpass"
```

### LDK Node Configuration
```toml
[ldk]
listen_host = "127.0.0.1"
listen_port = 8090
```

### gRPC Server Configuration
```toml
[grpc]
host = "127.0.0.1"
port = 50051
```

### LSP Server Configuration
```toml
[lsp]
listen_host = "127.0.0.1"
listen_port = 3000
min_channel_size_sat = 500000
max_channel_size_sat = 2000000
min_fee = 1000
fee_ppk = 1000
payment_url = "https://your-lsp-payment-url.com"
accepted_mints = [
  "https://mint1.example.com",
  "https://mint2.example.com"
]
```

## Getting Started

1. Copy `example.config.toml` to `config.toml` and adjust settings as needed
2. Ensure you have a Bitcoin node running with the RPC credentials specified in your config
3. Start the LSP node:
   ```
   cargo run --bin cdk-ldk-node
   ```
4. Interact with the LSP using the CLI or API:
   ```
   cargo run --bin cdk-ldk-cli
   ```

## Data Storage

The LSP stores all persistent data in the directory specified by `data_dir` in the config file (default: `~/.cashu_lsp`).

## Channel Policies

- Minimum channel size: 500,000 sats (configurable)
- Maximum channel size: 2,000,000 sats (configurable)
- Base fee: 1,000 sats (configurable)
- Fee rate: 1,000 parts per thousand (configurable)

