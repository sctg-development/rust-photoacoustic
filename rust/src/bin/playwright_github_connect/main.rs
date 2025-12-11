use playwright::Playwright;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Playwright and install browsers if needed
    print!("Starting Playwright... ");
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?;

    println!("Playwright started.");
    // Launch a headless Chromium browser
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?; // Use .headless(false) to see the browser

    print!("Launching browser... ");
    // Create a new browser context and page
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    println!("Browser launched.");
    // Navigate to the GitHub Pages documentation
    page.goto_builder("https://sctg-development.github.io/rust-photoacoustic/")
        .goto()
        .await?;

    // Wait a moment for the page to fully render
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get all links as JSON values
    let links_json: serde_json::Value = page
        .eval(r#"() => {
            const links = [];
            document.querySelectorAll('a').forEach(a => {
                links.push({href: a.href, text: a.innerText});
            });
            return links;
        }"#)
        .await?;
    println!("Found links: {}", links_json);

    // Try to find the API documentation link
    let api_link: serde_json::Value = page
        .eval(r#"() => {
            const links = document.querySelectorAll('a');
            for (let link of links) {
                if (link.innerText.includes('rust_photoacoustic') || link.href.includes('rust_photoacoustic/')) {
                    return link.href;
                }
            }
            return null;
        }"#)
        .await?;

    if let Some(url_str) = api_link.as_str() {
        println!("Found API documentation at: {}", url_str);
        page.goto_builder(url_str).goto().await?;
        
        // Wait for the documentation page to load
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Extract the version number from the documentation page
        let version: serde_json::Value = page
            .eval(r#"() => {
                const versionSpan = document.querySelector("span.version");
                if (versionSpan) {
                    return versionSpan.innerText.trim();
                }
                return "Version not found";
            }"#)
            .await?;
        println!("Package version: {}", version);
    } else {
        println!("Could not find API documentation link (got: {:?})", api_link);
    }

    // Verify we're on the correct page
    assert_eq!(
        page.url().unwrap(),
        "https://sctg-development.github.io/rust-photoacoustic/rust_photoacoustic/"
    );

    // Clean up - browser context and page are automatically closed when dropped
    browser.close().await?;
    Ok(())
}