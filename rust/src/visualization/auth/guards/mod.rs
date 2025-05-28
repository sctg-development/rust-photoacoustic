// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Request guards for authentication and authorization
//!
//! This module provides Rocket request guards for validating authentication
//! and checking permissions in API endpoints.

pub mod bearer;

#[cfg(test)]
mod test_macro;

#[cfg(test)]
//mod macro_integration_test;
mod macro_test_example;

// Re-export main guards
pub use bearer::OAuthBearer;
//pub use macros::{protect_get, protected_route_mounts, protected_routes};
