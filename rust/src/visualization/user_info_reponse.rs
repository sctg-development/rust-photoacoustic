use serde::{Deserialize, Serialize};

/// OpenID Connect UserInfo Response
///
/// This struct represents the standard response format for the OpenID Connect
/// UserInfo endpoint as defined in the OpenID Connect Core 1.0 specification.
///
/// The `sub` claim is the only required field, and identifies the subject of the token.
/// All other fields are optional according to the specification.
///
/// References:
/// - https://openid.net/specs/openid-connect-core-1_0.html#UserInfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoResponse {
    // Required claim - Subject identifier
    pub sub: String,

    // Profile claims
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoneinfo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>, // UNIX timestamp

    // Email claims
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,

    // Phone claims
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number_verified: Option<bool>,

    // Address claim
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,

    // Application-specific extensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

/// Address claim for OpenID Connect UserInfo
///
/// Represents a physical mailing address as defined in the OpenID Connect
/// Core 1.0 specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub formatted: Option<String>,
    pub street_address: Option<String>,
    pub locality: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

impl UserInfoResponse {
    /// Creates a new UserInfoResponse with minimal required fields
    pub fn new(subject: String) -> Self {
        UserInfoResponse {
            sub: subject,
            name: None,
            given_name: None,
            family_name: None,
            middle_name: None,
            nickname: None,
            preferred_username: None,
            profile: None,
            picture: None,
            website: None,
            gender: None,
            birthdate: None,
            zoneinfo: None,
            locale: None,
            updated_at: None,
            email: None,
            email_verified: None,
            phone_number: None,
            phone_number_verified: None,
            address: None,
            permissions: None,
        }
    }

    /// Build a UserInfoResponse from a User object
    pub fn from_user(user: &crate::config::User) -> Self {
        let mut response = Self::new(user.user.clone());

        // Map user fields to OIDC claims
        response.name = user.name.clone();
        response.preferred_username = Some(user.user.clone());
        response.email = user.email.clone();
        response.permissions = Some(user.permissions.clone());

        response
    }
}
