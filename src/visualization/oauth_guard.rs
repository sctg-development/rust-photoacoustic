// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Rocket request guard for validating Bearer tokens using JwtValidator (HS256/RS256)

use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;
use crate::visualization::jwt::jwt_validator::{JwtValidator, UserInfo};
use crate::visualization::oidc_auth::OxideState;
use base64::Engine;

/// Request guard for extracting and validating a Bearer JWT from the Authorization header
pub struct OAuthBearer(pub UserInfo);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OAuthBearer {
    type Error = (Status, &'static str);

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get the Authorization header
        let auth_header = request.headers().get_one("Authorization");

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                // Get the OxideState from Rocket state
                let state = match request.guard::<&State<OxideState>>().await {
                    Outcome::Success(state) => state,
                    _ => return Outcome::Error((Status::InternalServerError,(Status::InternalServerError, "Missing state"))),
                };
                // Build JwtValidator from state (supporting both HS256 and RS256)
                let hmac_secret = state.hmac_secret.as_bytes();
                let rs256_public_key = if !state.rs256_public_key.is_empty() {
                    base64::engine::general_purpose::STANDARD.decode(&state.rs256_public_key).ok()
                } else {
                    None
                };

                let mut validator = match rs256_public_key {
                    Some(ref pem) => JwtValidator::new(Some(hmac_secret), Some(pem)),
                    None => JwtValidator::new(Some(hmac_secret), None),
                };
                match validator {
                    Ok(validator) => {
                        match validator.get_user_info(token) {
                            Ok(user_info) => Outcome::Success(OAuthBearer(user_info)),
                            Err(_) => Outcome::Error((Status::Unauthorized,(Status::Unauthorized, "Invalid token"))),
                        }
                    }
                    Err(_) => Outcome::Error((Status::InternalServerError,(Status::InternalServerError, "Validator error"))),
                }
            } else {
                Outcome::Error((Status::Unauthorized,(Status::Unauthorized, "Missing Bearer token")))
            }
        } else {
            Outcome::Error((Status::Unauthorized,(Status::Unauthorized, "Missing Authorization header")))
        }
    }
}
