# JWT Token Implementation Documentation

## Overview

This document describes the JWT (JSON Web Token) implementation used in the rust-photoacoustic project for authentication and authorization.

## JWT Token Format

Our JWT tokens follow the standard JWT format with three parts: header, payload, and signature.

### Header

```json
{
  "alg": "HS256", // or "RS256" or "ES256" depending on configuration
  "typ": "JWT"
}
```

### Payload (Claims)

```json
{
  "sub": "user123", // Subject (user ID)
  "iat": 1626718856, // Issued at timestamp
  "exp": 1626722456, // Expiration timestamp
  "nbf": 1626718856, // Not before timestamp
  "jti": "client1-12345", // JWT ID (unique identifier)
  "aud": "client1", // Audience (client ID)
  "iss": "rust-photoacoustic", // Issuer
  "scope": "read:api write:api", // Space-delimited OAuth scopes
  "metadata": { // Optional additional claims
    "email": "user@example.com",
    "name": "John Doe"
  }
}
```

## Token Validation

The JWT validation process checks:

1. **Signature**: Verifies the token's signature using the configured key (symmetric or asymmetric)
2. **Expiration**: Checks if the token has expired by comparing the `exp` claim to the current time
3. **Not Before**: Ensures the token isn't used before its valid time by checking the `nbf` claim
4. **Issuer**: Validates the token was issued by the expected issuer by checking the `iss` claim
5. **Audience**: Verifies the token is intended for the current service by checking the `aud` claim

## Key Management

The system supports multiple key types:

1. **Symmetric Keys (HMAC)**: Uses a single secret key for both signing and verification
2. **Asymmetric Keys (RSA, ECDSA)**: Uses a private key for signing and a public key for verification

### Supported Algorithms

- `HS256`, `HS384`, `HS512`: HMAC with SHA-256, SHA-384, SHA-512
- `RS256`, `RS384`, `RS512`: RSA with SHA-256, SHA-384, SHA-512
- `ES256`, `ES384`: ECDSA with P-256, P-384 curve

## Token Introspection

The token introspection endpoint follows RFC 7662 and allows resource servers to validate tokens and retrieve token metadata. Introspection is accessible at `/introspect` and accepts both JSON and form-encoded parameters.

### Request Format

```
POST /introspect
Content-Type: application/x-www-form-urlencoded

token=<token_string>&token_type_hint=access_token
```

### Response Format

```json
{
  "active": true,
  "scope": "read:api write:api",
  "client_id": "client1",
  "sub": "user123",
  "exp": 1626722456,
  "iat": 1626718856,
  "nbf": 1626718856,
  "aud": "client1",
  "iss": "rust-photoacoustic",
  "jti": "client1-12345",
  "token_type": "Bearer"
}
```

When a token is invalid or expired, the response will be:

```json
{
  "active": false
}
```

## How to Use JWT in API Authentication

1. Extract the JWT token from the Authorization header (`Authorization: Bearer <token>`)
2. Validate the token using the `JwtValidator`
3. Extract user information and granted scopes from the token
4. Check if the user has the required scopes for the requested operation

## Security Considerations

- Use appropriate key lengths (at least 256 bits for symmetric keys)
- Set reasonable expiration times for tokens (typically 1 hour for access tokens)
- Store keys securely and implement key rotation where appropriate
- Validate all token fields, not just the signature
- Always use TLS to prevent token theft during transmission
