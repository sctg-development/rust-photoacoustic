//! OAuth 2.0 implementation using Oxide Auth
//!
//! This submodule contains all OAuth2-related functionality including
//! state management, endpoint handlers, and token processing.

pub mod auth;
pub mod consent;
pub mod forms;
pub mod handlers;
pub mod state;

// Re-export main items
pub use auth::validate_user;
pub use consent::{consent_decision, consent_form, consent_page_html};
pub use forms::{
    decode_user_session, encode_user_session, AuthForm, AuthenticatedUser, UserSession,
};
pub use handlers::{authorize, authorize_consent, login, refresh, token, userinfo};
pub use state::OxideState;
