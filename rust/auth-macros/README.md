# Authentication Macros for Rust Photoacoustic

This document describes the procedural macro system for creating protected Rocket routes with automatic JWT token validation and permission checking.

## Overview

The `protect_get` macro automatically adds Bearer token validation and permission checking to Rocket route handlers. If the user doesn't have the required permission, it returns HTTP 403 Forbidden.

## Project Structure

```
rust/
├── auth-macros/                    # Separate proc macro crate
│   ├── Cargo.toml                 # Proc macro dependencies
│   └── src/
│       └── lib.rs                 # The protect_get macro implementation
├── src/
│   └── visualization/
│       └── auth/
│           └── guards/
│               ├── mod.rs         # Re-exports the macro
│               ├── bearer.rs      # OAuthBearer guard implementation
│               ├── test_macro.rs  # Original test file
│               ├── macro_test_example.rs       # Working examples
│               └── macro_integration_test.rs   # Integration tests
└── Cargo.toml                     # Main project with auth-macros dependency
```

## How It Works

### 1. Macro Processing

The `protect_get` macro:
1. **Parses arguments**: Extracts the route path and required permission
2. **Analyzes function signature**: Checks if `OAuthBearer` is already a parameter
3. **Injects authentication**: Adds `OAuthBearer` parameter if not present
4. **Adds permission checking**: Validates user permissions before executing the function
5. **Handles return types**: Uses `rocket::Either` to return either 403 Forbidden or the original response

### 2. Type System Integration

The macro transforms functions to return:
```rust
rocket::Either<rocket::response::status::Forbidden<&'static str>, OriginalReturnType>
```

- **Left**: 403 Forbidden with "Permission denied" message if permission check fails
- **Right**: Original function's return value if permission check passes

### 3. Authentication Flow

1. **Token Extraction**: The `OAuthBearer` guard extracts Bearer tokens from Authorization headers
2. **Token Validation**: JWT signature and claims are validated
3. **Permission Checking**: The macro calls `bearer.has_permission(permission)` 
4. **Response Generation**: Either executes the original function or returns 403

## Usage Examples

### Basic Protected Route (Automatic Bearer Injection)

```rust
use auth_macros::protect_get;
use rocket::serde::json::Json;

#[protect_get("/api/users", "read:users")]
fn list_users() -> Json<Vec<User>> {
    // The macro automatically injects: bearer: OAuthBearer
    // The 'bearer' variable is available in scope
    Json(vec![
        User {
            id: bearer.user_info.user_id.clone(),
            name: "Current User".to_string(),
        }
    ])
}
```

### Explicit Bearer Parameter

```rust
#[protect_get("/api/data", "read:data")]
fn get_data(bearer: crate::visualization::auth::guards::OAuthBearer) -> Json<DataResponse> {
    // When Bearer is explicitly in signature, macro just adds permission checking
    Json(DataResponse {
        user_id: bearer.user_info.user_id,
        data: fetch_user_data(&bearer.user_info.user_id),
    })
}
```

### With Route Parameters

```rust
#[protect_get("/api/user/<user_id>/profile", "read:profiles")]
fn get_user_profile(user_id: String) -> Json<Profile> {
    // Route parameters work normally, bearer is auto-injected
    Json(Profile {
        user_id: user_id,
        viewer: bearer.user_info.user_id.clone(),
    })
}
```

## HTTP Response Behavior

| Condition | Response | Description |
|-----------|----------|-------------|
| Missing Authorization header | 401 Unauthorized | Handled by `OAuthBearer` guard |
| Invalid/expired JWT token | 401 Unauthorized | Handled by `OAuthBearer` guard |
| Valid token, insufficient permissions | 403 Forbidden | Returned by macro |
| Valid token, sufficient permissions | Original response | Function executes normally |

## Implementation Details

### Separate Proc Macro Crate

Procedural macros must be in a separate crate with `proc-macro = true` in `Cargo.toml`. This is why we have:

- `auth-macros/` - The proc macro crate
- Main crate depends on `auth-macros` and re-exports the macro

### Syn 2.0 Compatibility

The macro uses modern syn 2.0 features:
- `Punctuated<Expr, Token![,]>` for argument parsing
- Updated `TokenStream` handling
- Proper error handling with `syn::Error`

### Bearer Token Access

The macro ensures the `bearer` variable is available in the function scope:

```rust
// For functions without explicit OAuthBearer parameter:
fn original_function() -> Response {
    // bearer is available here automatically
    bearer.user_info.user_id
}

// Gets transformed to:
fn protected_function(
    bearer: crate::visualization::auth::guards::OAuthBearer,
) -> rocket::Either<Forbidden, Response> {
    if !bearer.has_permission("permission") {
        return rocket::Either::Left(Forbidden("Permission denied"));
    }
    rocket::Either::Right({
        // Original function body with bearer in scope
        bearer.user_info.user_id
    })
}
```

## Testing

The macro includes comprehensive tests:

1. **Compilation Tests**: Verify the macro compiles correctly
2. **Integration Tests**: Test with actual Rocket applications  
3. **Error Handling Tests**: Verify proper 401/403 responses

Run tests with:
```bash
cargo test --lib macro_integration_test
```

## Dependencies

### auth-macros/Cargo.toml
```toml
[dependencies]
proc-macro2 = "1.0"
quote = "1.0"
syn = { version = "2.0", features = ["full"] }
```

### Main Cargo.toml
```toml
[dependencies]
auth-macros = { path = "auth-macros" }
rocket = { version = "0.5", features = ["json"] }
# ... other dependencies
```

## Troubleshooting

### Common Issues

1. **"cannot return different types"**: The macro uses `rocket::Either` to solve this
2. **"bearer not found"**: Make sure the function signature is parsed correctly
3. **Module path errors**: Use `crate::` instead of the full crate name in generated code

### Debug Tips

1. Use `cargo expand` to see the generated code
2. Check that `OAuthBearer` is properly imported
3. Verify the permission string format matches your auth system

## Future Enhancements

Potential improvements:
1. Support for other HTTP methods (POST, PUT, DELETE)
2. Multiple permission requirements (AND/OR logic)
3. Dynamic permission calculation
4. Custom error responses
5. Integration with OpenAPI documentation generation
