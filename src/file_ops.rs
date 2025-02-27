use std::fs::{self, OpenOptions};
use std::io::{Write, Read};
use std::path::Path;
use anyhow::{anyhow, Context, Result};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use chardet::detect;

use crate::cli::Cli;
use crate::output::OutputFormat;

/// Checks if a file or directory exists without modifying it.
///
/// # Arguments
///
/// * `path` - The path to check
/// * `verbose` - Whether to print verbose output
///
/// # Returns
///
/// A Result that contains nothing on success
pub fn check_existence(path: &Path, verbose: bool) -> Result<()> {
    if path.exists() {
        if verbose {
            println!("Exists: {}", path.display());
        }
    } else {
        println!("Does not exist: {}", path.display());
    }
    Ok(())
}

/// Creates a directory at the specified path.
///
/// # Arguments
///
/// * `path` - The path where the directory should be created
/// * `verbose` - Whether to print verbose output
///
/// # Returns
///
/// A Result that contains nothing on success
pub fn create_directory(path: &Path, verbose: bool) -> Result<()> {
    fs::create_dir_all(path).context("Failed to create directory")?;
    if verbose {
        println!("Directory created: {}", path.display());
    }
    Ok(())
}

/// Creates a new file or updates an existing one based on the CLI options.
///
/// # Arguments
///
/// * `path` - The path to the file
/// * `cli` - The command line arguments
///
/// # Returns
///
/// A Result that contains nothing on success
pub fn create_or_update_file(path: &Path, cli: &Cli) -> Result<()> {
    // Check if verbose output is enabled and if output format is text
    let verbose = cli.verbose && cli.output_format == OutputFormat::Text;
    
    // If file exists and we're going to trim or transform line endings
    if path.exists() && (cli.trim || cli.line_endings.is_some()) {
        // Read the entire file content
        let content = fs::read_to_string(path).context("Failed to read file content")?;
        let mut modified_content = content;
        
        // Apply trim if requested
        if cli.trim {
            modified_content = modified_content
                .lines()
                .map(|line| line.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            if verbose {
                println!("Trailing whitespace removed from: {}", path.display());
            }
        }
        
        // Apply line ending transformations if requested
        if let Some(line_endings) = &cli.line_endings {
            match line_endings.as_str() {
                "crlf2lf" => {
                    modified_content = modified_content.replace("\r\n", "\n");
                    if verbose {
                        println!("Converted CRLF to LF: {}", path.display());
                    }
                },
                "lf2crlf" => {
                    // First normalize all line endings to LF
                    let normalized = modified_content.replace("\r\n", "\n");
                    // Then convert LF to CRLF
                    modified_content = normalized.replace("\n", "\r\n");
                    if verbose {
                        println!("Converted LF to CRLF: {}", path.display());
                    }
                },
                _ => {
                    return Err(anyhow!("Unsupported line ending conversion: {}. Supported values: crlf2lf, lf2crlf", line_endings));
                }
            }
        }
        
        // Write the modified content back to the file
        fs::write(path, modified_content).context("Failed to write modified content to file")?;
        
        // If we're not writing or appending additional content, return early
        if cli.write.is_none() && cli.template.is_none() {
            return Ok(());
        }
    }

    // Handle file encoding transformation
    if path.exists() && cli.encoding.is_some() {
        if let Some(encoding) = &cli.encoding {
            match encoding.to_lowercase().as_str() {
                "utf8" | "utf-8" => {
                    // Read file as bytes
                    let bytes = fs::read(path).context("Failed to read file content")?;
                    
                    // First, detect the encoding
                    let detection_result = detect(&bytes);
                    let detected_encoding = detection_result.0;
                    let confidence = detection_result.1;
                    
                    if verbose {
                        println!("Detected encoding: {} with confidence {:.2}%", detected_encoding, confidence * 100.0);
                    }
                    
                    // Only convert if it's not already UTF-8
                    if detected_encoding != "UTF-8" && confidence > 0.5 {
                        // Convert to UTF-8 using the appropriate encoding
                        let (encoding_to_use, name) = match detected_encoding.as_str() {
                            "ISO-8859-1" | "ISO-8859-2" => {
                                if let Some(enc) = Encoding::for_label(b"iso-8859-1") {
                                    (enc, "ISO-8859-1")
                                } else {
                                    // Fallback
                                    (WINDOWS_1252, "Windows-1252 (fallback)")
                                }
                            },
                            "windows-1252" => (WINDOWS_1252, "windows-1252"),
                            // Add other encodings as needed
                            _ => {
                                // For other encodings, try to find by name
                                if let Some(enc) = Encoding::for_label(detected_encoding.as_bytes()) {
                                    (enc, detected_encoding.as_str())
                                } else {
                                    // If we can't find an encoding, fallback to Windows-1252
                                    let fallback = Encoding::for_label(b"windows-1252").unwrap_or(UTF_8);
                                    (fallback, "Windows-1252 (fallback)")
                                }
                            }
                        };
                        
                        let (cow, _encoding_used, had_errors) = encoding_to_use.decode(&bytes);
                        fs::write(path, cow.as_bytes()).context("Failed to write UTF-8 content")?;
                        
                        if verbose {
                            println!("Converted from {} to UTF-8: {}", name, path.display());
                            if had_errors {
                                println!("Warning: There were errors during conversion");
                            }
                        }
                    } else if verbose {
                        println!("File is already UTF-8 or confidence too low: {}", path.display());
                    }
                },
                "latin1" | "iso-8859-1" => {
                    // Read file as UTF-8
                    let mut content = String::new();
                    let mut file = fs::File::open(path).context("Failed to open file")?;
                    file.read_to_string(&mut content).context("Failed to read file as UTF-8")?;
                    
                    // Get the Latin1 encoding
                    let latin1 = Encoding::for_label(b"iso-8859-1")
                        .ok_or_else(|| anyhow!("ISO-8859-1 encoding not supported"))?;
                    
                    // Convert to Latin-1
                    let (cow, _, had_errors) = latin1.encode(&content);
                    fs::write(path, cow.to_vec()).context("Failed to write Latin-1 content")?;
                    
                    if verbose {
                        println!("Converted from UTF-8 to Latin-1: {}", path.display());
                        if had_errors {
                            println!("Warning: There were errors during conversion");
                        }
                    }
                },
                // Add support for more encodings
                "windows-1252" => {
                    // Read file as UTF-8
                    let mut content = String::new();
                    let mut file = fs::File::open(path).context("Failed to open file")?;
                    file.read_to_string(&mut content).context("Failed to read file as UTF-8")?;
                    
                    // Convert to Windows-1252
                    let (cow, _, had_errors) = WINDOWS_1252.encode(&content);
                    fs::write(path, cow.to_vec()).context("Failed to write Windows-1252 content")?;
                    
                    if verbose {
                        println!("Converted from UTF-8 to Windows-1252: {}", path.display());
                        if had_errors {
                            println!("Warning: There were errors during conversion");
                        }
                    }
                },
                _ => {
                    return Err(anyhow!("Unsupported encoding: {}. Supported values: utf8, latin1, windows-1252", encoding));
                }
            }
        }
    }

    // Create or update file with new content
    let mut options = OpenOptions::new();
    options.write(true).create(true);

    if cli.append {
        options.append(true);
    } else if cli.write.is_some() || cli.template.is_some() {
        options.truncate(true);
    }

    let mut file = options
        .open(path)
        .context("Failed to create or open file")?;

    if let Some(template) = &cli.template {
        let content = fs::read_to_string(template).context("Failed to read template file")?;
        file.write_all(content.as_bytes())
            .context("Failed to write template content to file")?;
        if verbose {
            println!(
                "File created/updated with template content: {}",
                path.display()
            );
        }
    } else if let Some(content) = &cli.write {
        file.write_all(content.as_bytes())
            .context("Failed to write content to file")?;
        if verbose {
            if cli.append {
                println!("Content appended to file: {}", path.display());
            } else {
                println!("File created/updated with content: {}", path.display());
            }
        }
    } else if verbose {
        let metadata = file.metadata().context("Failed to get file metadata")?;
        if metadata.len() == 0 {
            println!("File created: {}", path.display());
        } else {
            println!("File timestamp updated: {}", path.display());
        }
    }

    Ok(())
} 