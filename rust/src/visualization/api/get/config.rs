// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use crate::config::Config;
use rocket::get;
use rocket::serde::json::Json;

use auth_macros::openapi_protect_get;

#[openapi_protect_get("/api/config", "admin:api", tag = "Configuration")]
pub async fn get_config(config: &Config) -> Json<Config> {
    // Return the current configuration as JSON
    Json(config.clone())
}
