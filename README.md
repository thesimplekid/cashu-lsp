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

