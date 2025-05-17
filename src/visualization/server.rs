// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::fairing::{Fairing, Info, Kind};
use rocket::figment::Figment;
use rocket::http::{ContentType, Header};
use rocket::response::{Redirect, Responder};
use rocket::{async_trait, delete, get, options, post, put, routes, uri, Build, Rocket};
use rocket::{Request, Response};
use include_dir::{include_dir, Dir};
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use rocket_okapi::{openapi, openapi_get_routes, rapidoc::*, settings::UrlObject};
use crate::visualization::oxide_auth::{authorize, authorize_consent, token, refresh};

use super::oxide_auth::OxideState;

const STATIC_DIR: Dir = include_dir!("web/dist");
#[derive(Debug)]
struct StaticFileResponse(Vec<u8>, ContentType);

#[async_trait]
impl<'r> Responder<'r, 'r> for StaticFileResponse {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .header(self.1)
            .header(Header {
                name: "Cache-Control".into(),
                value: "max-age=604800".into(), // 1 week
            })
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}

pub struct CORS;

#[rocket::async_trait]
impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, GET, PUT, DELETE, OPTIONS",
        ));
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}

/// # Answers to OPTIONS requests
#[openapi(tag = "Cors")]
#[options("/<_path..>")]
async fn options(_path: PathBuf) -> Result<(), std::io::Error> {
    Ok(())
}

pub async fn build_rocket(figment: Figment) -> Rocket<Build> {
    // Create OAuth2 state
    let oxide_state = OxideState::preconfigured();
    
    // Initialize JWT validator for API authentication
    let jwt_validator = match super::api_auth::init_jwt_validator() {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };
    
    let rocket = rocket::custom(figment)
        .attach(CORS)
        .mount(
            "/",
            openapi_get_routes![
                webclient_index,
                webclient_index_html,
            ],
        )
        .mount("/",routes![
            favicon,
            webclient,
            authorize,
            authorize_consent,
            token,
            refresh,
            super::introspection::introspect,
        ])
        .mount("/api", routes![
            super::api_auth::get_profile,
            super::api_auth::get_data,
        ])
        .manage(oxide_state)
        .manage(jwt_validator);
        rocket
}

#[cfg(test)]
pub fn build_rocket_test_instance() -> Rocket<Build> {
    use rocket::Config;
    
    // Create a test configuration
    let config = Config::figment()
        .merge(("address", "localhost"))
        .merge(("port", 0))  // Random port for tests
        .merge(("log_level", rocket::config::LogLevel::Off));
    
    // Create OAuth2 state
    let oxide_state = super::oxide_auth::OxideState::preconfigured();
    
    // Initialize JWT validator for API authentication
    let jwt_validator = match super::api_auth::init_jwt_validator() {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };
    
    // Build Rocket instance for tests
    rocket::custom(config)
        .attach(CORS)
        .mount(
            "/",
            routes![
                // Routes for OAuth tests
                authorize,
                authorize_consent,
                token,
                refresh,
                // TODO: Add introspection endpoint once fixed
                // super::introspection::introspect,
            ],
        )
        .mount("/api", routes![
            super::api_auth::get_profile,
            super::api_auth::get_data,
        ])
        .manage(oxide_state)
        .manage(jwt_validator)
}

/// Retrieves a static file from the web/dist directory
///
/// # Arguments
///
/// * `path` - the path to the file relative to the web/dist directory
///
/// # Returns
///
/// * `Some(StaticFileResponse)` if the file exists, containing the file data and content type
/// * `None` if the file does not exist
#[get("/client/<path..>")]
async fn webclient(path: PathBuf) -> Option<StaticFileResponse> {
    if env::var("VITE_DEVELOPMENT").is_ok() {
        let vite_base = env::var("VITE_DEVELOPMENT").unwrap_or("http://localhost:5173".to_string());
        let url = format!("{}/{}", vite_base, path.to_str().unwrap_or(""));
        let response = reqwest::get(&url).await.unwrap();
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<ContentType>()
            .unwrap();
        let bytes = response.bytes().await.unwrap();
        let response_content: Vec<u8> = bytes.iter().map(|byte| *byte).collect();
        let content = StaticFileResponse(response_content, content_type);
        return Some(content);
    }

    let path = path.to_str().unwrap_or("");
    let file = STATIC_DIR.get_file(path).map(|file| {
        let content_type = ContentType::from_extension(
            file.path()
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap(),
        )
        .unwrap_or(ContentType::Binary);
        StaticFileResponse(file.contents().to_vec(), content_type)
    });
    if file.is_some() {
        return file;
    } else {
        let file = STATIC_DIR.get_file("index.html").map(|file| {
            let content_type = ContentType::from_extension(
                file.path()
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap(),
            )
            .unwrap_or(ContentType::Binary);
            StaticFileResponse(file.contents().to_vec(), content_type)
        });
        return file;
    }
}

#[openapi(tag = "webclient")]
#[get("/index.html")]
async fn webclient_index_html() -> Redirect {
    webclient_index_multi().await
}
async fn webclient_index_multi() -> Redirect {
    Redirect::to(uri!("/client/index.html"))
}

#[openapi(tag = "webclient")]
#[get("/")]
async fn webclient_index() -> Redirect {
    webclient_index_multi().await
}

#[get("/favicon.ico")]
async fn favicon() -> Option<StaticFileResponse>  {
    let file = STATIC_DIR.get_file("favicon.ico").map(|file| {
        let content_type = ContentType::from_extension(
            file.path()
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap(),
        )
        .unwrap_or(ContentType::Binary);
        StaticFileResponse(file.contents().to_vec(), content_type)
    });
    return file;
}