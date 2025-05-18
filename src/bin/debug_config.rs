// Debug test for certificate validation
use anyhow::Result;
use rust_photoacoustic::config::Config;
use std::path::Path;

fn main() -> Result<()> {
    let path = Path::new("tests/debug_cert.yml");
    
    println!("Testing file: {:?}", path);
    println!("File exists: {}", path.exists());
    
    let result = Config::from_file(path);
    
    match result {
        Ok(_) => println!("Validation succeeded (UNEXPECTED)"),
        Err(e) => println!("Validation failed as expected: {}", e),
    }
    
    Ok(())
}
