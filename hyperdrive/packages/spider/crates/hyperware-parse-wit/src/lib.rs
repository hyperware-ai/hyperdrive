use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use wit_parser::Resolve;
use zip::ZipArchive;

// Include the default hyperware.wit file
const DEFAULT_HYPERWARE_WIT: &str = include_str!("hyperware.wit");

/// Parse WIT files from a zip archive and return JSON representation
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip(zip_bytes: &[u8], fallback_wit: Option<Vec<u8>>) -> Result<String> {
    let resolve = parse_wit_from_zip_to_resolve(zip_bytes, fallback_wit)?;
    let json = serde_json::to_string_pretty(&resolve).context("failed to serialize to JSON")?;
    Ok(json)
}

/// Parse WIT files from a zip archive and return parsed Resolve
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip_to_resolve(
    zip_bytes: &[u8],
    fallback_wit: Option<Vec<u8>>,
) -> Result<Resolve> {
    // Open the zip archive from bytes
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).context("failed to open zip archive")?;

    let mut resolve = Resolve::default();
    let mut has_package_header = false;
    let mut wit_files = Vec::new();

    // First pass: collect all WIT files and check for package headers
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("failed to access zip entry")?;
        let name = file.name().to_string();

        // Skip directories
        if name.ends_with('/') {
            continue;
        }

        // Only process .wit files
        if !name.ends_with(".wit") {
            continue;
        }

        // Read the file contents
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("failed to read file: {}", name))?;

        // Check if this file contains a package header
        if contents.contains("package ") && contents.contains("@") {
            has_package_header = true;
        }

        wit_files.push((name, contents));
    }

    // If no package header found, combine all WIT files with the fallback package header
    if !has_package_header && !wit_files.is_empty() {
        let fallback_content = match fallback_wit {
            Some(bytes) => {
                String::from_utf8(bytes).context("fallback WIT content is not valid UTF-8")?
            }
            None => DEFAULT_HYPERWARE_WIT.to_string(),
        };

        // Combine all WIT files into a single document with the package header
        let mut combined_wit = fallback_content;
        combined_wit.push_str("\n\n");

        for (_, contents) in &wit_files {
            combined_wit.push_str(&contents);
            combined_wit.push_str("\n\n");
        }

        // Parse the combined WIT document
        resolve
            .push_str(Path::new("combined.wit"), &combined_wit)
            .context("failed to parse combined WIT package")?;
    } else {
        // Parse each file individually if package headers are present
        for (name, contents) in wit_files {
            let path = Path::new(&name);
            resolve
                .push_str(path, &contents)
                .with_context(|| format!("failed to parse WIT file: {}", name))?;
        }
    }

    Ok(resolve)
}

/// Parse WIT files from a zip archive and return serde_json::Value
///
/// # Arguments
/// * `zip_bytes` - The bytes of the zip file containing WIT files
/// * `fallback_wit` - Optional WIT content to use when package header is missing.
///                    If None, uses the built-in hyperware.wit
pub fn parse_wit_from_zip_to_value(
    zip_bytes: &[u8],
    fallback_wit: Option<Vec<u8>>,
) -> Result<serde_json::Value> {
    let resolve = parse_wit_from_zip_to_resolve(zip_bytes, fallback_wit)?;
    let value = serde_json::to_value(&resolve).context("failed to convert to JSON value")?;
    Ok(value)
}
