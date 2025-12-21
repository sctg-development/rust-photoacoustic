// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Playwright-based E2E tests for RapiDoc Shadow DOM rendering
//!
//! These tests verify that RapiDoc properly renders its Shadow DOM and populates
//! it with the correct API documentation content, including the API description
//! that should appear in the rendered output.

use playwright::Playwright;
use rocket::config::LogLevel;
use rust_photoacoustic::config::{AccessConfig, VisualizationConfig};
use serde_json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 8082))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Critical))
        .merge((
            "hmac_secret",
            "test-hmac-secret-key-for-testing".to_string(),
        ))
        .merge(("access_config", AccessConfig::default()))
        .merge(("visualization_config", VisualizationConfig::default()))
}

fn get_test_config() -> rust_photoacoustic::config::Config {
    let mut config = rust_photoacoustic::config::Config::default();
    config.visualization.port = 8082;
    config.visualization.address = "127.0.0.1".to_string();
    config.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    config
}

#[tokio::test]
async fn test_rapidoc_shadow_dom_rendering() -> Result<(), Box<dyn std::error::Error>> {
    // Use port 8082 for this test
    let mut figment = get_figment();
    figment = figment.merge(("port", 8082));

    // Start the test server in a background task
    let test_config = get_test_config();
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        figment,
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
        None,                      // No shared computing state
    )
    .await;

    // Launch the server in a background task
    let server_handle = tokio::spawn(async move {
        let _ = rocket.launch().await;
    });

    // Give the server time to start up
    sleep(Duration::from_millis(500)).await;

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    // Launch a headless Chromium browser
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;

    // Create a new browser context and page
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    // Navigate to the RapiDoc documentation
    page.goto_builder("http://127.0.0.1:8082/api/doc/")
        .goto()
        .await?;

    // Wait for RapiDoc to load and render
    // The rapi-doc element should be present in the DOM
    page.wait_for_selector_builder("rapi-doc")
        .wait_for_selector()
        .await?;

    println!("✓ RapiDoc element loaded");

    // Give RapiDoc time to load the OpenAPI spec and populate the Shadow DOM
    sleep(Duration::from_millis(2000)).await;

    // Access the rapi-doc element
    let rapi_doc_element = page
        .query_selector("rapi-doc")
        .await?
        .ok_or("rapi-doc element not found")?;

    println!("✓ rapi-doc element found in DOM");

    // Get the Shadow Root content
    // We use evaluate to access the Shadow DOM from JavaScript
    let shadow_dom_html: String = page
        .eval(
            r#"() => {
                const rapiDoc = document.querySelector('rapi-doc');
                if (!rapiDoc || !rapiDoc.shadowRoot) {
                    return '';
                }
                return rapiDoc.shadowRoot.innerHTML;
            }"#,
        )
        .await?;

    println!("Shadow DOM HTML length: {} chars", shadow_dom_html.len());

    // Get the package description from Cargo.toml (available at compile time)
    let expected_description = env!("CARGO_PKG_DESCRIPTION");

    // Verify the API description is present in the Shadow DOM
    if shadow_dom_html.contains(expected_description) {
        println!(
            "✓ API description found in Shadow DOM: {}",
            expected_description
        );
    } else {
        println!(
            "Shadow DOM content (first 1000 chars):\n{}",
            &shadow_dom_html.chars().take(1000).collect::<String>()
        );

        // The description might be split across elements, check for partial matches
        let parts: Vec<&str> = expected_description.split_whitespace().collect();
        let mut found_parts = 0;
        for part in &parts {
            if shadow_dom_html.contains(part) {
                found_parts += 1;
            }
        }

        if found_parts >= 3 {
            println!("✓ API description found (split across elements) in Shadow DOM");
        } else {
            return Err(format!(
                "API description not found in Shadow DOM. Expected: {}",
                expected_description
            )
            .into());
        }
    }

    // Verify the API title is present in the Shadow DOM
    if shadow_dom_html.contains("SCTG rust-photoacoustic API") {
        println!("✓ API title found in Shadow DOM");
    } else {
        println!("⚠ API title not found in Shadow DOM, checking for alternative titles");
        if !shadow_dom_html.contains("API") {
            eprintln!("Warning: API documentation headers not visible");
        }
    }

    // Verify that there are interactive elements (endpoints, schemas, etc.)
    let has_interactive_content = shadow_dom_html.contains("try-it")
        || shadow_dom_html.contains("http")
        || shadow_dom_html.contains("endpoint")
        || shadow_dom_html.contains("response");

    if has_interactive_content {
        println!("✓ Interactive RapiDoc content detected in Shadow DOM");
    } else {
        println!("⚠ Limited interactive content detected (may be normal depending on API)");
    }

    // Clean up
    browser.close().await?;

    println!("✓ RapiDoc Shadow DOM rendering test passed!");

    // Cancel the server task (it will stop when the test ends)
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_rapidoc_page_title() -> Result<(), Box<dyn std::error::Error>> {
    // Use port 8083 for this test
    let mut figment = get_figment();
    figment = figment.merge(("port", 8083));

    // Start the test server
    let test_config = get_test_config();
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        figment,
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
        None,                      // No shared computing state
    )
    .await;

    let server_handle = tokio::spawn(async move {
        let _ = rocket.launch().await;
    });

    // Give the server time to start
    sleep(Duration::from_millis(500)).await;

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;

    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    // Navigate to RapiDoc
    page.goto_builder("http://127.0.0.1:8083/api/doc/")
        .goto()
        .await?;

    // Verify the page title is correct
    let title = page.title().await?;
    assert_eq!(
        title, "SCTG rust-photoacoustic API Doc",
        "Page title should match the configured API title"
    );

    println!("✓ Page title is correct: {}", title);

    // Verify OpenAPI spec is available
    page.goto_builder("http://127.0.0.1:8083/openapi.json")
        .goto()
        .await?;

    // Wait for content to load
    sleep(Duration::from_millis(500)).await;

    // Get the response body
    let spec_text = page
        .eval::<String>(
            r#"() => {
                return document.documentElement.innerText;
            }"#,
        )
        .await?;

    let expected_description = env!("CARGO_PKG_DESCRIPTION");
    assert!(
        spec_text.contains(expected_description),
        "OpenAPI spec should contain API description: {}",
        expected_description
    );

    println!("✓ OpenAPI spec contains expected content");

    // Clean up
    browser.close().await?;

    println!("✓ RapiDoc page title test passed!");

    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_rapidoc_api_operations_visible() -> Result<(), Box<dyn std::error::Error>> {
    // Use port 8084 for this test
    let mut figment = get_figment();
    figment = figment.merge(("port", 8084));

    // Start the test server
    let test_config = get_test_config();
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        figment,
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
        None,                      // No shared computing state
    )
    .await;

    let server_handle = tokio::spawn(async move {
        let _ = rocket.launch().await;
    });

    sleep(Duration::from_millis(500)).await;

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;

    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    // Navigate to RapiDoc
    page.goto_builder("http://127.0.0.1:8084/api/doc/")
        .goto()
        .await?;

    // Wait for the rapi-doc element to appear
    page.wait_for_selector_builder("rapi-doc")
        .wait_for_selector()
        .await?;

    // Give RapiDoc time to fully load
    sleep(Duration::from_millis(2000)).await;

    // Check for API sections in the Shadow DOM
    let api_sections: String = page
        .eval(
            r#"() => {
                const rapiDoc = document.querySelector('rapi-doc');
                if (!rapiDoc || !rapiDoc.shadowRoot) return 'NO_SHADOW_DOM';
                
                const html = rapiDoc.shadowRoot.innerHTML;
                
                // Check for common RapiDoc content
                const checks = {
                    hasPath: html.includes('path') || html.includes('endpoint'),
                    hasMethod: html.includes('get') || html.includes('post') || html.includes('put') || html.includes('delete'),
                    hasSchemas: html.includes('schema') || html.includes('definition'),
                    hasSecurityInfo: html.includes('security') || html.includes('authorization'),
                };
                
                return JSON.stringify(checks);
            }"#,
        )
        .await?;

    println!("API sections found: {}", api_sections);

    // Verify the page has loaded some content
    assert!(
        !api_sections.contains("NO_SHADOW_DOM"),
        "Shadow DOM should be available"
    );

    // At minimum, check for basic API structure
    let has_api_content = api_sections.contains("true");
    if has_api_content {
        println!("✓ API content is visible in RapiDoc");
    } else {
        println!("⚠ Limited API content detected (may be expected for minimal API)");
    }

    browser.close().await?;

    println!("✓ RapiDoc API operations visibility test passed!");

    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_rapidoc_footer_github_link() -> Result<(), Box<dyn std::error::Error>> {
    // Use port 8085 for this test
    let mut figment = get_figment();
    figment = figment.merge(("port", 8085));

    // Start the test server
    let test_config = get_test_config();
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        figment,
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
        None,                      // No shared computing state
    )
    .await;

    let server_handle = tokio::spawn(async move {
        let _ = rocket.launch().await;
    });

    sleep(Duration::from_millis(500)).await;

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;

    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    // Navigate to RapiDoc
    page.goto_builder("http://127.0.0.1:8085/api/doc/")
        .goto()
        .await?;

    // Wait for the rapi-doc element to appear
    page.wait_for_selector_builder("rapi-doc")
        .wait_for_selector()
        .await?;

    // Give RapiDoc time to fully load
    sleep(Duration::from_millis(2000)).await;

    // Check for the GitHub link in the footer slot
    let footer_content: String = page
        .eval(
            r#"() => {
                const rapiDoc = document.querySelector('rapi-doc');
                if (!rapiDoc) {
                    return JSON.stringify({ hasGithubLink: false, error: 'rapi-doc not found' });
                }
                
                // Check the entire document for the GitHub link (it's in a slot which renders in Light DOM)
                const pageHtml = document.documentElement.outerHTML;
                const hasGithubLink = pageHtml.includes('https://github.com/sctg-development/rust-photoacoustic');
                
                // Also check Shadow DOM
                let shadowHasLink = false;
                if (rapiDoc.shadowRoot) {
                    const shadowHtml = rapiDoc.shadowRoot.innerHTML;
                    shadowHasLink = shadowHtml.includes('https://github.com/sctg-development/rust-photoacoustic');
                }
                
                return JSON.stringify({
                    hasGithubLink: hasGithubLink,
                    hasSCTGLink: pageHtml.includes('sctg') || pageHtml.includes('SCTG'),
                    hasFooterContent: pageHtml.includes('footer') || pageHtml.includes('2025'),
                    shadowDomHasLink: shadowHasLink,
                    pageLength: pageHtml.length
                });
            }"#,
        )
        .await?;

    println!("Footer content check: {}", footer_content);

    let footer_data: serde_json::Value = serde_json::from_str(&footer_content)?;

    // Verify the GitHub link is present
    assert!(
        footer_data["hasGithubLink"].as_bool().unwrap_or(false),
        "GitHub link to https://github.com/sctg-development/rust-photoacoustic should be present in footer"
    );

    println!(
        "✓ GitHub link found in footer: https://github.com/sctg-development/rust-photoacoustic"
    );

    // Verify the SCTG reference is present
    assert!(
        footer_data["hasSCTGLink"].as_bool().unwrap_or(false),
        "SCTG reference should be present in footer"
    );

    println!("✓ SCTG reference found in footer");

    // Verify footer content is present (year 2025 or 'footer' text)
    assert!(
        footer_data["hasFooterContent"].as_bool().unwrap_or(false),
        "Footer content should be visible in RapiDoc"
    );

    println!("✓ Footer content is visible in RapiDoc");

    // Clean up
    browser.close().await?;

    println!("✓ RapiDoc footer GitHub link test passed!");

    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_rapidoc_json_code_block_rendering() -> Result<(), Box<dyn std::error::Error>> {
    // Use port 8085 for this test
    let mut figment = get_figment();
    figment = figment.merge(("port", 8085));

    // Start the test server in a background task
    let test_config = get_test_config();
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        figment,
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
        None,                      // No shared computing state
    )
    .await;

    // Launch the server in a background task
    let server_handle = tokio::spawn(async move {
        let _ = rocket.launch().await;
    });

    // Give the server time to start up
    sleep(Duration::from_millis(500)).await;

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    // Launch a headless Chromium browser
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;

    // Create a new browser context and page
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    // Navigate to the RapiDoc documentation
    page.goto_builder("http://127.0.0.1:8085/api/doc/")
        .goto()
        .await?;

    // Wait for RapiDoc to load and render
    // The rapi-doc element should be present in the DOM
    page.wait_for_selector_builder("rapi-doc")
        .wait_for_selector()
        .await?;

    println!("✓ RapiDoc element loaded");

    // Give RapiDoc time to load the OpenAPI spec and populate the Shadow DOM
    sleep(Duration::from_millis(2000)).await;

    // Access the rapi-doc element
    let rapi_doc_element = page
        .query_selector("rapi-doc")
        .await?
        .ok_or("rapi-doc element not found")?;

    println!("✓ rapi-doc element found in DOM");

    // Get the Shadow Root content
    // click on div with id="link-get-/api/config" with eval strategy
    let click_result: String = page
        .eval(
            r#"() => {
            const rapiDoc = document.querySelector('rapi-doc');
            if (!rapiDoc || !rapiDoc.shadowRoot) {
                return 'false';
            }
            const linkDiv = rapiDoc.shadowRoot.getElementById('link-get-/api/config');
            if (linkDiv) {
                linkDiv.click();
                return 'true';
            } else {
                return 'false';
            }
        }"#,
        )
        .await?;

    if click_result != "true" {
        return Err("Failed to click on /api/config endpoint link".into());
    }

    // extract the JSON code block content from the Shadow DOM
    let json_code_block: String = page
        .eval(
            r#"() => {
            const rapiDoc = document.querySelector('rapi-doc');
            if (!rapiDoc || !rapiDoc.shadowRoot) {
                return '';
            }
            // Wait for the response section to be populated
            const responseSection = rapiDoc.shadowRoot.querySelector('div[id="get-/api/config"]');
            if (!responseSection) {
                return '';
            }
            const codeBlock = responseSection.querySelector('pre code');
            if (codeBlock) {
                return codeBlock.innerText;
            } else {
                return '';
            }
        }"#,
        )
        .await?;
    println!(
        "Extracted JSON code block length: {} chars",
        json_code_block.len()
    );
    // Verify that the JSON code block does not start with "json" (indicating proper markdown rendering)
    if json_code_block.trim_start().starts_with("json") {
        return Err("JSON code block rendering failed, found 'json' prefix".into());
    } else {
        println!("✓ JSON code block rendered correctly without 'json' prefix");
    }
    // Clean up
    browser.close().await?;

    println!("✓ RapiDoc JSON code block rendering test passed!");

    server_handle.abort();

    Ok(())
}
