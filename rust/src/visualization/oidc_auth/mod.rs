// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # OAuth 2.0 / OpenID Connect Authentication Module
//!
//! This module provides a complete OAuth 2.0 and OpenID Connect implementation
//! for the visualization server, including authentication flows, token management,
//! and session handling.
//!
//! ## Module Structure
//!
//! - [`state`] - OAuth state management and configuration
//! - [`forms`] - Authentication forms and session handling
//! - [`auth`] - User credential validation
//! - [`consent`] - OAuth consent form processing
//! - [`handlers`] - OAuth endpoint handlers
//!
//! ## OAuth Flow
//!
//! The module implements the OAuth 2.0 authorization code flow with PKCE:
//!
//! 1. Client initiates authorization request
//! 2. User is redirected to login form
//! 3. After authentication, user sees consent form
//! 4. Authorization code is issued
//! 5. Client exchanges code for access token
//! 6. Access token is used for API access

pub mod auth;
pub mod consent;
pub mod forms;
pub mod handlers;
pub mod state;

// Re-export commonly used items
pub use auth::validate_user;
pub use consent::{consent_decision, consent_form, consent_page_html};
pub use forms::{
    decode_user_session, encode_user_session, format_scopes, login_page_html, AuthForm,
    AuthenticatedUser, UserSession,
};
pub use handlers::{authorize, authorize_consent, login, refresh, token, userinfo};
pub use state::OxideState;
