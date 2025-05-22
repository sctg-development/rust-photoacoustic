// Example of loading and using configuration in a photoacoustic application

use anyhow::Result;
use rust_photoacoustic::config::Config;

fn main() -> Result<()> {
    // Load configuration from the default file
    let mut config = Config::from_file("config.yaml")?;
    println!("Loaded configuration: {:?}", config);

    // Override with command line arguments
    let web_port = 9000; // Example command line port
    let web_address = "192.168.1.100"; // Example command line address

    // Add a custom HMAC secret for JWT tokens
    let hmac_secret = Some("custom-jwt-secret-from-cmdline".to_string());

    config.apply_args(
        Some(web_port),
        Some(web_address.to_string()),
        hmac_secret,
        true,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    println!(
        "Configuration after applying command line arguments: {:?}",
        config
    );

    // Access configuration values
    println!(
        "Server will run on {}:{}",
        config.visualization.address, config.visualization.port
    );

    Ok(())
}
