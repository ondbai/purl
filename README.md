# purl

A curl-esque CLI for making HTTP requests that require payment. Supports both [x402](https://x402.org) and [MPP (Machine Payment Protocol)](https://paymentauth.org) payment-gated APIs. Designed for humans and agents alike.

## Installation

via Homebrew
```bash
brew install stripe/purl/purl
```

via source
```bash
git clone https://github.com/stripe/purl
cd purl
cargo install --path cli
```

Requires [Rust](https://rustup.rs/). Ensure `~/.cargo/bin` is in your PATH.

## Quickstart

It is recommended to use a wallet dedicated for usage with purl.

```bash
# Set up your wallet
purl wallet add

# Preview payment without executing
purl --dry-run https://api.example.com/data

# Make a request
purl https://api.example.com/data

# Require confirmation before paying
purl --confirm https://api.example.com/data

# Understand payment requirements for a resource
purl inspect http://api.example.com/data

# See your balance
purl balance

# See and manage wallets
purl wallet list
```

## Payment Protocols

purl automatically detects the payment protocol from the server's 402 response:

- **x402**: Payment requirements in the response body or `PAYMENT-REQUIRED` header. Supports EVM (Ethereum, Base, Polygon, etc.) and Solana.
- **MPP**: Payment challenge in the `WWW-Authenticate: Payment` header. Supports Tempo blockchain via the [mpp](https://github.com/tempoxyz/mpp-rs) crate.

### MPP / Tempo

For MPP-enabled endpoints on the Tempo Moderato testnet, you can auto-fund a wallet and pay in one command:

```bash
# With a fresh key (generate-and-go)
purl --private-key $(openssl rand -hex 32) --fund \
  https://api-test.ondb.ai/api/queries/app_e7ffa5e6ed124464/get_getCommunityIP/data?ip=8.8.8.8

# With a saved wallet
purl --fund https://api.example.com/paid-endpoint
```

The `--fund` flag calls the Tempo testnet faucet (`tempo_fundAddress`) to top up the wallet before paying.

To configure a custom Tempo RPC URL, add it to `~/.purl/config.toml`:

```toml
[rpc]
tempo = "https://rpc.moderato.tempo.xyz"
```

## Usage

```
purl [OPTIONS] <URL>
purl <COMMAND>
```

Run `purl help` for all commands or `purl topics` for detailed documentation.

## Development

```bash
make build    # Build
make test     # Run tests
make release  # Build release binary
```
