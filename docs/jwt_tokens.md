# JWT Token Implementation Documentation

## Overview

This document describes the JWT (JSON Web Token) and OpenID Connect implementation used in the rust-photoacoustic project for authentication and authorization. The system supports both OAuth 2.0 access tokens and OpenID Connect ID tokens.

## Token Types

### Access Tokens (JWT Format)

Access tokens are used for API authorization and follow the standard JWT format with three parts: header, payload, and signature. These tokens now include detailed user permission information for fine-grained access control.

#### Header

```json
{
  "alg": "HS256", // or "RS256" or "ES256" depending on configuration
  "typ": "JWT"
}
```

#### Payload (Claims)

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
  "permissions": ["read", "write", "admin"], // Array of user-specific permissions
  "metadata": { // Optional additional claims
    "user_id": "user123",
    "user_permissions": "read write admin", // Space-separated permissions string
    "preferred_username": "user123",
    "user_name": "user123",
    "redirect_uri": "http://localhost:8080/callback"
  }
}
```

### Access Tokens (JWT Format)

Access tokens are used for API authorization and follow the standard JWT format with three parts: header, payload, and signature. These tokens now include detailed user permission information for fine-grained access control.

#### Header

```json
{
  "alg": "HS256", // or "RS256" or "ES256" depending on configuration
  "typ": "JWT"
}
```

#### Payload (Claims)

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
  "permissions": ["read", "write", "admin"], // Array of user-specific permissions
  "metadata": { // Optional additional claims
    "user_id": "user123",
    "user_permissions": "read write admin", // Space-separated permissions string
    "preferred_username": "user123",
    "user_name": "user123",
    "redirect_uri": "http://localhost:8080/callback"
  }
}
```
### ID Tokens (OpenID Connect)

ID tokens are issued when the `openid` scope is requested and contain authentication information about the user.

#### ID Token Claims

```json
{
  "sub": "user123", // Subject (user ID)
  "iss": "rust-photoacoustic", // Issuer
  "aud": "client1", // Audience (client ID)
  "iat": 1626718856, // Issued at timestamp
  "exp": 1626722456, // Expiration timestamp
  "auth_time": 1626718856, // Authentication time
  "nonce": "random-nonce", // Nonce value (if provided in auth request)
  "sid": "session-12345", // Session ID
  "acr": "0", // Authentication Context Class Reference
  "amr": ["pwd"], // Authentication Methods References
  "azp": "client1", // Authorized party
  // Optional user profile information
  "name": "John Doe",
  "nickname": "john",
  "picture": "https://example.com/avatar.jpg",
  "email": "john@example.com",
  "email_verified": true,
  "updated_at": "2023-07-19T14:47:36Z"
}
```

## Token Response Format

When tokens are issued, the response includes both access and ID tokens (if requested):

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "def50200abcd...",
  "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...", // Only if 'openid' scope requested
  "scope": "read:api write:api openid profile email"
}
```

## Token Validation

The JWT validation process checks:

1. **Signature**: Verifies the token's signature using the configured key (symmetric or asymmetric)
2. **Expiration**: Checks if the token has expired by comparing the `exp` claim to the current time
3. **Not Before**: Ensures the token isn't used before its valid time by checking the `nbf` claim
4. **Issuer**: Validates the token was issued by the expected issuer by checking the `iss` claim
5. **Audience**: Verifies the token is intended for the current service by checking the `aud` claim

### OpenID Connect Specific Validation

For ID tokens, additional validation is performed:

1. **Nonce**: If a nonce was provided in the authorization request, it must match the ID token
2. **Authentication Time**: The `auth_time` claim indicates when the user was authenticated
3. **Session ID**: The `sid` claim provides session tracking capabilities

## Key Management

The system supports multiple key types:

1. **Symmetric Keys (HMAC)**: Uses a single secret key for both signing and verification
2. **Asymmetric Keys (RSA, ECDSA)**: Uses a private key for signing and a public key for verification

### Supported Algorithms

- `HS256`, `HS384`, `HS512`: HMAC with SHA-256, SHA-384, SHA-512
- `RS256`, `RS384`, `RS512`: RSA with SHA-256, SHA-384, SHA-512
- `ES256`, `ES384`: ECDSA with P-256, P-384 curve

## OpenID Connect Discovery

The system provides OpenID Connect discovery endpoints:

### Discovery Endpoint

```http
GET /.well-known/openid-configuration
```

Returns configuration information about the OpenID Provider:

```json
{
  "issuer": "https://your-server.com",
  "authorization_endpoint": "https://your-server.com/oauth/authorize",
  "token_endpoint": "https://your-server.com/oauth/token",
  "jwks_uri": "https://your-server.com/.well-known/jwks.json",
  "response_types_supported": ["code"],
  "grant_types_supported": ["authorization_code", "refresh_token"],
  "subject_types_supported": ["public"],
  "id_token_signing_alg_values_supported": ["HS256", "RS256"],
  "scopes_supported": ["openid", "profile", "email", "read:api", "write:api"],
  "claims_supported": ["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce", "name", "email"]
}
```

### JWKS Endpoint

```http
GET /.well-known/jwks.json
```

Exposes public keys for token verification (for asymmetric algorithms):

```json
{
  "keys": [
    {
      "kty": "RSA",
      "use": "sig",
      "kid": "key-id",
      "n": "...",
      "e": "AQAB"
    }
  ]
}
```

## Token Introspection

The token introspection endpoint follows RFC 7662 and allows resource servers to validate tokens and retrieve token metadata. Introspection is accessible at `/oauth/introspect` and accepts both JSON and form-encoded parameters.

### Request Format

```http
POST /oauth/introspect
Content-Type: application/x-www-form-urlencoded

token=<token_string>&token_type_hint=access_token
```

Or in JSON format:

```http
POST /oauth/introspect
Content-Type: application/json

{
  "token": "<token_string>",
  "token_type_hint": "access_token"
}
```

### Response Format

For active tokens:

```json
{
  "active": true,
  "scope": "read:api write:api openid",
  "client_id": "client1",
  "sub": "user123",
  "exp": 1626722456,
  "iat": 1626718856,
  "nbf": 1626718856,
  "aud": "client1",
  "iss": "rust-photoacoustic",
  "jti": "client1-12345",
  "token_type": "Bearer",
  // Additional metadata from JWT claims
  "user_id": "user123",
  "preferred_username": "user123"
}
```

When a token is invalid or expired, the response will be:

```json
{
  "active": false
}
```

## Token Storage and Management

### TokenEntry Structure

Each token set is stored as a `TokenEntry` containing:

- **Access Token**: The JWT access token string
- **ID Token**: Optional OpenID Connect ID token (only when `openid` scope is requested)
- **Refresh Token**: Optional refresh token for obtaining new access tokens
- **Grant Information**: Associated OAuth grant details
- **Expiry Time**: When the token set expires

### Token Maps

The system maintains two hash maps for efficient token lookup:

1. **Access Token Map**: Maps access token strings to `TokenEntry` instances
2. **Refresh Token Map**: Maps refresh token strings to the same `TokenEntry` instances

This dual-mapping allows fast token validation and refresh operations.

## OAuth 2.0 Flows

### Authorization Code Flow

1. **Authorization Request**: Client redirects user to `/oauth/authorize`
2. **User Authentication**: User provides credentials via login form
3. **Authorization Grant**: Server issues authorization code
4. **Token Request**: Client exchanges code for tokens at `/oauth/token`
5. **Token Response**: Server returns access token, optional refresh token, and optional ID token

### Token Refresh Flow

1. **Refresh Request**: Client sends refresh token to `/oauth/token`
2. **Token Validation**: Server validates the refresh token
3. **New Token Issuance**: Server issues new access token (ID tokens are not refreshed)
4. **Response**: Server returns new token set

## User Claims and Metadata

### Automatic User Claims

When issuing tokens, the system automatically includes user-specific claims:

- `user_id`: The authenticated user's identifier
- `user_permissions`: Space-separated list of user permissions
- `preferred_username`: The user's preferred username
- `user_name`: Alternative username field

### Custom Claims

Additional claims can be added through the JWT issuer's `add_user_claims` method:

```rust,ignore
issuer.add_user_claims("john_doe", &["read", "write", "admin"]);
```

### Claim Lifecycle

User claims are:

1. Added before token issuance
2. Included in both access and ID tokens
3. Automatically cleared after token generation to prevent cross-contamination

## How to Use JWT in API Authentication

### For API Clients

1. **Obtain Token**: Complete OAuth 2.0 authorization flow
2. **Include in Requests**: Add token to Authorization header

   ```http
   Authorization: Bearer <access_token>
   ```

3. **Handle Expiration**: Use refresh token to obtain new access tokens

### For Resource Servers

1. **Extract Token**: Parse Authorization header to get JWT
2. **Validate Token**: Verify signature, expiration, and claims
3. **Extract Claims**: Get user information and scopes from token
4. **Authorize Request**: Check if user has required scopes for the operation

### Example API Usage

```http
GET /api/data
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

The server validates the token and extracts user information:

```rust,ignore
// Token validation extracts these claims
let user_id = "user123";
let scopes = ["read:api", "write:api"];
let permissions = ["read", "write"];
```

## Security Considerations

### Token Security

- **Use HTTPS**: Always transmit tokens over encrypted connections
- **Short Expiration**: Set reasonable expiration times (typically 1 hour for access tokens)
- **Secure Storage**: Store refresh tokens securely on the client side
- **Key Rotation**: Implement key rotation for production environments

### Validation Requirements

- **Signature Verification**: Always verify token signatures
- **Claim Validation**: Validate all required claims (iss, aud, exp, etc.)
- **Scope Checking**: Verify the token has required scopes for requested operations
- **Replay Protection**: Use nonce values in ID tokens when appropriate

### Production Recommendations

- **Use RS256**: Prefer asymmetric keys (RS256) over symmetric keys (HS256) in production
- **Key Management**: Use proper key management solutions for private keys
- **Monitoring**: Log token issuance and validation for security monitoring
- **Rate Limiting**: Implement rate limiting on token endpoints

## Configuration Examples

### HMAC (Symmetric) Configuration

```rust,ignore
let issuer = JwtIssuer::new(b"your-256-bit-secret-key")
    .with_issuer("rust-photoacoustic")
    .valid_for(Duration::hours(1));
```

### RSA (Asymmetric) Configuration

```rust,ignore
let issuer = JwtTokenMap::with_rs256_pem(
    include_bytes!("private_key.pem"),
    include_bytes!("public_key.pem")
)?
.with_issuer("rust-photoacoustic")
.valid_for(Duration::hours(1));
```

## Troubleshooting

### Common Issues

1. **Token Validation Fails**: Check clock synchronization and key configuration
2. **Missing Claims**: Ensure user claims are added before token issuance
3. **ID Token Not Issued**: Verify that `openid` scope is included in the request
4. **Refresh Token Invalid**: Check token expiration and storage integrity

### Debug Information

Enable debug logging to see token validation details:

```rust,ignore
log::debug!("JWT validation failed: {:?}", err);
```

The system logs validation failures with detailed error information for troubleshooting.
