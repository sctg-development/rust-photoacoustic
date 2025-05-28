//! Request guards for authentication and authorization
//!
//! This module provides Rocket request guards for validating authentication
//! and checking permissions in API endpoints.

pub mod bearer;

// Re-export main guards
pub use bearer::OAuthBearer;
