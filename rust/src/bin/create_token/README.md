# Create Token - Refactored Structure

## 📋 Overview

This module provides a command-line utility to manually create JWT tokens for the photoacoustic application. The structure has been refactored to improve maintainability and testability.

## 🏗️ Architecture

### Modular Structure

```
create_token/
├── mod.rs              # Main module and exports
├── cli.rs              # CLI argument handling
├── config_loader.rs    # Config loading and validation
├── token_creator.rs    # Token creation logic
├── error.rs            # Typed error management
└── tests.rs            # Unit tests
```

### Responsibilities

- **`cli.rs`**: Parses and validates command-line arguments
- **`config_loader.rs`**: Loads configuration and validates users/clients
- **`token_creator.rs`**: Handles JWT token creation with different algorithms
- **`error.rs`**: Defines specific errors with exit codes
- **`tests.rs`**: Unit tests to validate logic

## 🚀 Usage

### Command Line

```bash
# Create a token with RS256 (default)
cargo run --bin create_token_refactored -- -u username -i client_id

# Create a token with HS256
cargo run --bin create_token_refactored -- -u username -i client_id -a HS256

# Specify a custom duration
cargo run --bin create_token_refactored -- -u username -i client_id -d 7200

# Use a specific configuration file
cargo run --bin create_token_refactored -- -c custom_config.yaml -u username -i client_id
```

### Available Arguments

- `-q, --quiet`: Suppress output (only token is printed)
- `-c, --config <FILE>`: Path to the configuration file (default: config.yaml)
- `-a, --algorithm <ALGORITHM>`: JWT algorithm (HS256 or RS256, default: RS256)
- `-d, --duration <SECONDS>`: Token duration in seconds (overrides config)
- `-u, --user <USERNAME>`: Username (required, must exist in config)
- `-i, --client <CLIENT>`: Client ID (required, must exist in config)

## 🧪 Tests

Run the tests:

```bash
cargo test --bin create_token_refactored
```

The tests cover:

- JWT algorithm parsing
- User and client validation
- Duration override handling
- Error exit codes

## 🔧 Required Configuration

The YAML configuration file must contain:

```yaml
visualization:
  hmac_secret: "your-hmac-secret"
  rs256_private_key: "base64-encoded-private-key"
  rs256_public_key: "base64-encoded-public-key"

access:
  duration: 3600 # default duration in seconds
  iss: "IssuerName"
  users:
    - user: "username"
      permissions: ["read", "write"]
  clients:
    - client_id: "client_id"
      default_scope: "basic"
      allowed_callbacks: ["http://localhost:3000/callback"]
```

## 🚦 Exit Codes

- `0`: Success
- `1`: Configuration error
- `2`: User not found
- `3`: Client not found
- `4`: Key decoding error
- `5`: JwtIssuer creation error
- `6`: Invalid scope
- `7`: Invalid redirect URI
- `8`: No redirect URI configured
- `9`: Token issuing error
