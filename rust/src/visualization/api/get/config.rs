// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::serde::json::Json;
use rust_photoacoustic::config::Config;

use auth_macros::openapi_protect_get;
