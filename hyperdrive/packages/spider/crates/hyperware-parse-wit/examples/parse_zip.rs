use anyhow::Result;
use hyperware_parse_wit::parse_wit_from_zip;
use std::env;
use std::fs;

fn main() -> Result<()> {
    // Get the zip file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!(
            "Usage: {} <path-to-wit-zip-file> [path-to-fallback-wit]",
            args[0]
        );
        eprintln!("  If no fallback WIT is provided, uses built-in hyperware.wit");
        std::process::exit(1);
    }

    let zip_path = &args[1];

    // Read the zip file into memory
    let zip_bytes = fs::read(zip_path).expect(&format!("Failed to read zip file: {}", zip_path));

    // Optionally read custom fallback WIT
    let fallback_wit = if args.len() == 3 {
        Some(fs::read(&args[2]).expect(&format!("Failed to read fallback WIT file: {}", args[2])))
    } else {
        None
    };

    // Parse WIT from the zip and get JSON
    // If no package header is found in the zip, it will use the fallback WIT
    let json = parse_wit_from_zip(&zip_bytes, fallback_wit)?;

    // Print the JSON output
    println!("{}", json);

    Ok(())
}
