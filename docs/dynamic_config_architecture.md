# Dynamic Configuration Architecture

**Audience**: Junior developers and maintainers new to the project  
**Last updated**: April 2026

---

## Table of Contents

1. [Why Dynamic Configuration?](#why-dynamic-configuration)
2. [The Foundation: Shared State with `Arc<RwLock<Config>>`](#the-foundation)
3. [Processing Graph Hot-Reload](#processing-graph-hot-reload)
   - [The API Endpoint](#the-api-endpoint)
   - [The Monitoring Loop](#the-monitoring-loop)
   - [The `ProcessingNode` Trait](#the-processingnode-trait)
   - [Node Hot-Reload Inventory](#node-hot-reload-inventory)
4. [AccessConfig Hot-Reload (5 Phases)](#accessconfig-hot-reload)
   - [Why It Was Hard](#why-it-was-hard)
   - [Phase-by-Phase Summary](#phase-by-phase-summary)
   - [The Three Authentication Guards](#the-three-authentication-guards)
   - [OxideState and `update_access_config()`](#oxidestate-and-update_access_config)
   - [Daemon Wiring via `build_rocket_for_daemon`](#daemon-wiring)
5. [Complete Hot-Reload Reference](#complete-hot-reload-reference)
6. [How to Add Hot-Reload to a New Node](#how-to-add-hot-reload-to-a-new-node)
7. [Testing Dynamic Configuration](#testing-dynamic-configuration)

---

## Why Dynamic Configuration?

This system is deployed on industrial hardware (Raspberry Pi / Alpine Linux) measuring gas concentrations 24/7. Restarting the process takes several seconds and interrupts measurements. Dynamic configuration lets operators:

- Tune signal processing parameters (filter frequencies, gain) without stopping the acquisition
- Add or revoke user access without rebooting
- Update OAuth2 clients without a service window

**The challenge**: Rocket (our web framework) initializes most of its state at startup and passes it to request handlers as managed state. Making this state react to configuration changes required careful architecture.

---

## The Foundation

### `Arc<RwLock<Config>>`

All dynamic configuration is anchored to a single shared value:

```rust
Arc<tokio::sync::RwLock<Config>>
```

- `Arc` — multiple owners (Rocket state, daemon, processing consumer) share the same allocation
- `RwLock` — many concurrent readers, one exclusive writer at a time
- `tokio::sync::RwLock` — **async-aware**, accessed with `.await` — **not** `std::sync::RwLock`

**Key difference from `std::sync::RwLock`**: Tokio's `RwLock` does _not_ have lock poisoning. A panic in another async task will not poison the lock. You will never see `.unwrap_or_else(|e| e.into_inner())` — that is a `std` pattern that does not apply here.

```rust
// Writing to shared config (e.g., after POST /api/graph/config)
let mut config = config_arc.write().await;
config.processing.default_graph.nodes[i].parameters = new_params;
// Lock automatically released when `config` goes out of scope

// Reading from shared config (e.g., in a request guard)
let config = config_arc.read().await;
let access = config.access.clone(); // clone quickly, then release
drop(config); // explicit or implicit drop — keep the lock duration short
```

This `Arc<RwLock<Config>>` is registered as managed Rocket state and passed to `build_rocket()`. Every component that needs live configuration receives a clone of the `Arc` (which shares the underlying allocation).

---

## Processing Graph Hot-Reload

### The API Endpoint

**Route**: `POST /api/graph/config`  
**Scope required**: `admin:api`  
**Source**: `rust/src/visualization/api/graph/graph.rs`

The request body is a `NodeConfig` JSON object — **not** a full `Config`. It targets a single processing node by ID:

```json
{
  "id": "bandpass_filter_1",
  "node_type": "filter",
  "parameters": {
    "center_freq": 2050.0,
    "bandwidth": 80.0,
    "order": 4,
    "sample_rate": 48000
  }
}
```

The handler:
1. Checks that the node exists in the active graph and supports hot-reload
2. Validates the parameter types (type mismatch → HTTP 400)
3. **Merges** the new parameters into `config.processing.default_graph.nodes[i].parameters` (not a full replacement)
4. Writes atomically to `Arc<RwLock<Config>>`
5. Returns the merged parameters as JSON

No explicit notification is sent. The monitoring loop (below) picks up the change automatically within 1 second.

### The Monitoring Loop

`ProcessingConsumer::start_config_monitoring()` is called when the processing consumer starts. It spawns a `tokio` task that runs independently of the main processing loop:

```
Every 1 second:
  1. Read Arc<RwLock<Config>>
  2. Hash the processing.default_graph section
  3. If hash unchanged → skip
  4. If hash changed:
     a. Compare each node's parameters to the stored baseline
     b. For each node with changed parameters:
        - Call node.update_config(new_parameters)
        - Ok(true)  → hot-reload applied, no interruption
        - Ok(false) → node needs full rebuild
        - Err(e)    → log error, schedule rebuild
     c. Update stored baselines
```

**Source file**: `rust/src/processing/consumer.rs` — `start_config_monitoring()` and `check_and_apply_config_changes()`

The ≤1 second latency between writing the config and seeing the effect in signal processing is by design. It keeps the monitoring simple and the lock contention low.

### The `ProcessingNode` Trait

Every node in the processing graph implements this trait (from `rust/src/processing/nodes/traits.rs`):

```rust
pub trait ProcessingNode: Send + Sync {
    fn process(&mut self, data: ProcessingData) -> Result<ProcessingData>;
    fn node_id(&self) -> &str;
    fn node_type(&self) -> &str;
    fn clone_node(&self) -> Box<dyn ProcessingNode>;

    // Dynamic configuration methods
    fn supports_hot_reload(&self) -> bool { false }  // default: no hot-reload
    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        Ok(false)  // default: not supported
    }
}
```

**Return value of `update_config()`**:
- `Ok(true)` — parameters applied, processing continues uninterrupted
- `Ok(false)` — this node cannot apply these parameters without a rebuild
- `Err(e)` — parameter validation failed (e.g., frequency above Nyquist)

### Node Hot-Reload Inventory

#### Signal Processing Nodes (`processing/nodes/`)

| Node | `node_type` YAML | Hot-reload | Hot-reloadable parameters |
|---|---|---|---|
| `InputNode` | `input` | ❌ No | — (no configurable parameters) |
| `GainNode` | `gain` | ✅ Yes | `gain_db` |
| `FilterNode` | `filter` | ✅ Yes | `target_channel`, all filter params (see below) |
| `ChannelMixerNode` | `channel_mixer` | ✅ Yes | `mix_strategy` |
| `DifferentialNode` | `channel_mixer` (subtract) | ❌ No | Infrastructure ready, no configurable params yet |
| `StreamingNode` | `streaming` | ❌ No | State would be lost on rebuild |
| `RecordNode` | `record` | ❌ No | File management complexity |
| `OutputNode` | `output` | ❌ No | — |

**FilterNode** delegates parameter changes to the underlying filter implementation:

| Filter type | Hot-reloadable parameters |
|---|---|
| `LowpassFilter` | `cutoff_freq`, `sample_rate`, `order` |
| `HighpassFilter` | `cutoff_freq`, `sample_rate`, `order` |
| `BandpassFilter` | `center_freq`, `bandwidth`, `sample_rate`, `order` (must be even) |

The filter automatically **recalculates its coefficients** after an `update_config()` call.

#### Computing Nodes (`processing/computing_nodes/`)

| Node | `node_type` YAML | Hot-reload |
|---|---|---|
| `PeakFinderNode` | `computing_peak_finder` | ✅ Yes |
| `ConcentrationNode` | `computing_concentration` | ✅ Yes |
| `ActionUniversalNode` | `action_universal` | ✅ Yes |

---

## AccessConfig Hot-Reload

### Why It Was Hard

User accounts, OAuth2 clients, and JWT settings are stored in `AccessConfig`. Changing them without restart required touching four different systems that each held their own copy of this data:

1. **Rocket figment** — static configuration baked in at startup
2. **OxideState** (the OAuth2 engine) — holds the `ClientMap` and `JwtIssuer`
3. **`JwtValidator`** — validates incoming tokens using its own expected audience
4. **Guards** — `AuthenticatedUser`, `AccessConfig` (as a guard), `OAuthBearer` — each extracted config differently

The fix was implemented across 5 phases.

### Phase-by-Phase Summary

| Phase | What was fixed | Key files | Tests added |
|---|---|---|---|
| 1 | `AuthenticatedUser` + `AccessConfig` guards now read from `Arc<RwLock<Config>>` per-request instead of the static figment | `visualization/api_auth.rs`, `config/access.rs` | 13 |
| 2 | Login handler + OIDC discovery endpoint now read live config | `visualization/auth/oauth2/handlers.rs`, `visualization/oidc.rs` | +3 (16) |
| 3 | `OxideState.access_config` changed from `AccessConfig` (static copy) to `Arc<RwLock<AccessConfig>>` + new `update_access_config()` method | `visualization/auth/oauth2/state.rs` | +2 (18) |
| 4 | `JwtValidator.validate()` now uses `expected_audience` (set at startup) instead of iterating `access_config.clients` — prevents stale audience list after hot-reload | `visualization/auth/jwt/validator.rs` | +2 (20) |
| 5 | Daemon stores an `OxideState` handle and calls `update_access_config()` when the `"access"` YAML section changes | `visualization/server/builder.rs`, `daemon/launch_daemon.rs` | +2 (22) |

**All 22 tests**: `rust/tests/access_config_hot_reload_test.rs`

### The Three Authentication Guards

Understanding which guard runs when is essential for reasoning about hot-reload behavior.

#### `OAuthBearer` (in `bearer.rs`) — ✅ Always live

Used for routes that validate API tokens. Reads `Arc<RwLock<Config>>` on every request:

```rust
// Per-request: always reads the current config
let config = config_state.read().await.clone();
let access_config = config.access.clone();  // live copy

// Builds a JwtValidator with the live access config
let validator = JwtValidator::new(hmac, rs256_pub, access_config.clone());
// Joins the JWT claims against the LIVE user list
validator.get_user_info(token, access_config)
```

**Practical effect**: If you remove a user from `access.users` in the config and hot-reload, their token will be rejected on the very next request — even though the token itself has not expired.

#### `AuthenticatedUser` (in `api_auth.rs`) — ✅ Live since Phase 1

Before Phase 1, this guard read from the static Rocket figment. After Phase 1, it reads from `Arc<RwLock<Config>>`:

```rust
let config_state = request.guard::<&State<Arc<RwLock<Config>>>>().await?;
let access_config = config_state.read().await.access.clone();
```

`AuthenticatedUser.scopes` comes from the JWT claims (fixed at token-issue time). To get **live** permissions, call `JwtValidator::get_user_info(token, live_access_config)` which joins against the current user list.

#### `AccessConfig` (as a guard in `config/access.rs`) — ✅ Live since Phase 1

When a handler declares `access: AccessConfig` as a parameter, Rocket calls `FromRequest for AccessConfig`, which now reads from `Arc<RwLock<Config>>`.

### OxideState and `update_access_config()`

`OxideState` is the heart of the OAuth2 engine. It holds:
- `Arc<Mutex<ClientMap>>` — registered OAuth2 clients
- `Arc<Mutex<JwtIssuer>>` — token issuer (duration, issuer claim)
- `Arc<RwLock<AccessConfig>>` — live reference to access config (since Phase 3)

After Phase 3, `OxideState` exposes:

```rust
pub async fn update_access_config(&self, new_config: AccessConfig) {
    // 1. Rebuild the ClientMap from new_config.clients
    // 2. Update the JwtIssuer duration and issuer claim
    // 3. Update self.access_config (Arc<RwLock<AccessConfig>>)
}
```

This method is called by the daemon's `apply_configuration_changes()` whenever the `"access"` YAML section changes.

**Important**: `OxideState` implements `Clone` using `Arc::clone` for all inner fields. Cloning does **not** copy the data — it creates a new handle that shares the same underlying `Arc`s. This is what allows the daemon and Rocket to both see the same live state.

### Daemon Wiring

The final piece (Phase 5) connects `OxideState` to the daemon's configuration change handler.

**Before Phase 5**, `build_rocket()` consumed `OxideState` by moving it into Rocket via `.manage()`. The daemon had no handle to call `update_access_config()`.

**After Phase 5**, there is a new function:

```rust
// rust/src/visualization/server/builder.rs
pub async fn build_rocket_for_daemon(
    figment: Figment,
    config: Arc<RwLock<Config>>,
    // ... other params
) -> (Rocket<Build>, OxideState) {
    // Builds OxideState, then clones it BEFORE passing to .manage()
    // The clone shares all inner Arcs — both handles see the same live state
    let oxide_state = OxideState::from_config(&config).await;
    let oxide_clone = oxide_state.clone(); // Arc::clone under the hood
    let rocket = build_rocket_inner(figment, config, oxide_state, ...).await;
    (rocket, oxide_clone) // caller keeps a live handle
}
```

The `Daemon` struct stores the returned clone:

```rust
pub struct Daemon {
    config: Arc<RwLock<Config>>,
    oxide_state: Option<OxideState>, // live handle to the OAuth2 engine
    // ...
}
```

When the daemon detects a change in the `"access"` YAML section:

```rust
"access" => {
    let new_access_config = self.config.read().await.access.clone();
    if let Some(ref oxide) = self.oxide_state {
        oxide.update_access_config(new_access_config).await;
        // Rocket now uses the updated ClientMap and JwtIssuer
    }
}
```

---

## Complete Hot-Reload Reference

Summary of all YAML sections and their hot-reload behavior:

| YAML section | Hot-reload | Mechanism | Latency |
|---|---|---|---|
| `processing.default_graph.nodes` (params only) | ✅ Partial | `ProcessingConsumer` monitoring loop → `update_config()` | ≤ 1 second |
| `processing.default_graph` (structural: add/remove nodes) | ❌ Restart | Graph is rebuilt from scratch | Service restart |
| `access` (users, clients, JWT) | ✅ Full | `OxideState.update_access_config()` via daemon | Near-instant |
| `visualization` | ❌ Restart | Rocket binds port at startup | Service restart |
| `acquisition` | ❌ Restart | Audio device opened at startup | Service restart |
| `modbus` | ❌ Restart | TCP socket bound at startup | Service restart |
| `thermal_regulation` | ❌ Restart | PID controllers initialized at startup | Service restart |

---

## How to Add Hot-Reload to a New Node

Follow these four steps to make a new `ProcessingNode` support dynamic configuration.

### Step 1 — Override the trait methods

```rust
impl ProcessingNode for MyNewNode {
    // ... other required methods ...

    fn supports_hot_reload(&self) -> bool {
        true // tell the API and monitoring loop this node can be updated
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        // Extract the parameter you want to make configurable
        if let Some(my_param) = parameters.get("my_param") {
            let value: f64 = serde_json::from_value(my_param.clone())
                .map_err(|e| anyhow::anyhow!("Invalid my_param: {}", e))?;

            // Validate the value
            if value <= 0.0 {
                return Err(anyhow::anyhow!("my_param must be positive"));
            }

            self.my_param = value;
            return Ok(true); // hot-reload applied
        }

        // No recognized parameters changed → no rebuild needed either
        Ok(false)
    }
}
```

### Step 2 — Update the YAML schema

Add your new parameter to `rust/src/resources/config.schema.json` under the appropriate node type. New parameters **must have a default value** to preserve backward compatibility with existing config files.

### Step 3 — Add unit tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_my_node_supports_hot_reload() {
        let node = MyNewNode::new(/* ... */);
        assert!(node.supports_hot_reload());
    }

    #[test]
    fn test_my_node_update_config_valid() {
        let mut node = MyNewNode::new(/* ... */);
        let params = json!({ "my_param": 42.0 });
        let result = node.update_config(&params);
        assert!(matches!(result, Ok(true)));
        assert_eq!(node.my_param, 42.0);
    }

    #[test]
    fn test_my_node_update_config_invalid() {
        let mut node = MyNewNode::new(/* ... */);
        let params = json!({ "my_param": -1.0 });
        assert!(node.update_config(&params).is_err());
    }

    #[test]
    fn test_my_node_update_config_unknown_param() {
        let mut node = MyNewNode::new(/* ... */);
        let params = json!({ "unknown_key": 99.0 });
        // Unknown params should return Ok(false) — not an error
        assert!(matches!(node.update_config(&params), Ok(false)));
    }
}
```

### Step 4 — Verify end-to-end with the API

```bash
# Get a token with write:api scope
TOKEN=$(curl -s -X POST https://localhost:8080/token ... | jq -r .access_token)

# Send a hot-reload update
curl -k -X POST https://localhost:8080/api/graph/config \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"id": "my_node_id", "node_type": "my_node", "parameters": {"my_param": 42.0}}'
# Expected: HTTP 200 with the merged parameters
```

---

## Testing Dynamic Configuration

### Integration Tests for Rocket Handlers

Use `#[rocket::async_test]` (not `#[tokio::test]`) and build a test Rocket instance with `build_rocket()`:

```rust
use rust_photoacoustic::config::{AccessConfig, Config, VisualizationConfig};
use rust_photoacoustic::visualization::server::build_rocket;
use rocket::config::LogLevel;
use std::sync::Arc;
use tokio::sync::RwLock;

fn test_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 0))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Off))
        .merge(("hmac_secret", "test-hmac-secret-key-for-testing"))
        .merge(("secret_key", "/qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis="))
        .merge(("access_config", AccessConfig::default()))
        .merge(("visualization_config", VisualizationConfig::default()))
}

#[rocket::async_test]
async fn test_graph_config_endpoint() {
    let config = Arc::new(RwLock::new(Config::default()));
    let rocket = build_rocket(test_figment(), config, None, None, None, None, None).await;
    let client = rocket::local::asynchronous::Client::tracked(rocket).await.unwrap();
    // ... issue token and test POST /api/graph/config
}
```

### Integration Tests for Daemon Wiring

When testing code that needs both Rocket **and** an `OxideState` handle (e.g., hot-reload of `AccessConfig`), use `build_rocket_for_daemon()`:

```rust
use rust_photoacoustic::visualization::server::build_rocket_for_daemon;

#[rocket::async_test]
async fn test_access_config_hot_reload() {
    let config = Arc::new(RwLock::new(Config::default()));
    let (rocket, oxide_state) = build_rocket_for_daemon(
        test_figment(), Arc::clone(&config), None, None, None, None, None
    ).await;

    // oxide_state and the managed Rocket state share the same Arc internals
    let new_access = AccessConfig { /* updated config */ ..AccessConfig::default() };
    oxide_state.update_access_config(new_access.clone()).await;

    // Verify the change is visible through Rocket's managed state
    let client = rocket::local::asynchronous::Client::tracked(rocket).await.unwrap();
    // ... test that new clients/users are active
}
```

### Issuing Test JWT Tokens

```rust
use rust_photoacoustic::visualization::auth::jwt::JwtIssuer;
use oxide_auth::primitives::grant::{Extensions, Grant};
use oxide_auth::primitives::issuer::Issuer;

const TEST_HMAC_SECRET: &str = "test-hmac-secret-key-for-testing";

fn issue_test_token(username: &str, scopes: &[&str]) -> String {
    let mut issuer = JwtIssuer::new(TEST_HMAC_SECRET.as_bytes());
    // IMPORTANT: call add_user_claims BEFORE issue() — claims are cleared after issue()
    let scope_strings: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
    issuer.add_user_claims(username, &scope_strings);

    let grant = Grant {
        owner_id: username.to_string(),
        client_id: "LaserSmartClient".to_string(),
        scope: scopes.join(" ").parse().unwrap(),
        redirect_uri: "https://localhost/callback".parse().unwrap(),
        until: chrono::Utc::now() + chrono::Duration::hours(1),
        extensions: Extensions::new(),
    };
    issuer.issue(grant).unwrap().token
}
```

Expected issuer: `"LaserSmartServer"` / Expected audience: `"LaserSmartClient"`

### Reference Test File

`rust/tests/access_config_hot_reload_test.rs` — 22 tests covering all 5 phases.  
Run with: `cargo test --test access_config_hot_reload_test` (from `rust/`)

---

## Quick Reference

```
Q: I changed a user's password in config.yaml. When does it take effect?
A: After the daemon detects the file change and calls apply_configuration_changes("access"),
   which calls OxideState.update_access_config(). Existing tokens are NOT revoked,
   but the next login will use the new password.

Q: I added a new OAuth2 client. When is it registered?
A: Same — after apply_configuration_changes("access"). The ClientMap is rebuilt
   inside update_access_config().

Q: I changed filter parameters via POST /api/graph/config. When do I hear the change?
A: Within 1 second — that's the poll interval of the monitoring loop.

Q: Can I change which nodes exist in the graph without restarting?
A: No. Adding, removing, or rewiring nodes requires a daemon restart. Only
   node *parameters* can be hot-reloaded.

Q: My new node returns Ok(false) from update_config. What happens?
A: The monitoring loop marks it for rebuild. The processing graph is reconstructed
   from the updated config. There will be a brief interruption (~milliseconds).
```
