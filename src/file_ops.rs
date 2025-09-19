use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chardet::detect;
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

use crate::cli::{Cli, EncodingTarget, LineEndingConversion};
use crate::output::{OperationResult, OutputFormat};

pub fn check_existence(
    path: &Path,
    verbose: bool,
    interactive: bool,
    result: &mut OperationResult,
) -> Result<()> {
    let exists = path.exists();
    let status = if exists { "Exists" } else { "Does not exist" };

    if verbose || interactive {
        println!("{}: {}", status, path.display());
    }

    result.message = Some(status.to_string());
    Ok(())
}

pub fn create_directory(path: &Path, verbose: bool, result: &mut OperationResult) -> Result<()> {
    let already_exists = path.exists();
    fs::create_dir_all(path).context("Failed to create directory")?;

    if verbose {
        if already_exists {
            println!("Directory ensured: {}", path.display());
        } else {
            println!("Directory created: {}", path.display());
        }
    }

    result.message = Some(if already_exists {
        "Directory ensured".to_string()
    } else {
        "Directory created".to_string()
    });
    Ok(())
}

pub fn create_or_update_file(path: &Path, cli: &Cli, result: &mut OperationResult) -> Result<()> {
    let verbose = cli.verbose && cli.output_format == OutputFormat::Text;
    let interactive = cli.output_format == OutputFormat::Text && atty::is(atty::Stream::Stdout);
    let should_echo = verbose || interactive;
    let existed_before = path.exists();

    if existed_before && (cli.trim || cli.line_endings.is_some()) {
        let source = fs::read_to_string(path).map_err(|e| {
            anyhow!(
                "Failed to read {} as UTF-8 (required for --trim/--line-endings). {}",
                path.display(),
                e
            )
        })?;

        let mut modified = source.clone();
        let mut changed = false;

        if cli.trim {
            let (trimmed, trimmed_changed) = trim_trailing_whitespace(&modified);
            if trimmed_changed {
                modified = trimmed;
                changed = true;
                result
                    .details
                    .push("trailing whitespace removed".to_string());
                if should_echo {
                    println!("Trailing whitespace removed from: {}", path.display());
                }
            }
        }

        if let Some(conversion) = cli.line_endings {
            let (converted, converted_changed) = convert_line_endings(&modified, conversion);
            if converted_changed {
                modified = converted;
                changed = true;
                let detail = match conversion {
                    LineEndingConversion::CrlfToLf => "line endings normalized to LF",
                    LineEndingConversion::LfToCrlf => "line endings normalized to CRLF",
                };
                result.details.push(detail.to_string());
                if should_echo {
                    println!("{}: {}", detail.to_string(), path.display());
                }
            }
        }

        if changed {
            fs::write(path, modified).context("Failed to write modified content to file")?;
            if result.message.is_none() {
                result.message = Some("File updated".to_string());
            }
        }
    }

    if path.exists() {
        if let Some(target) = cli.encoding {
            match target {
                EncodingTarget::Utf8 => {
                    let bytes = fs::read(path)
                        .context("Failed to read file content for encoding conversion")?;
                    let detection = detect(&bytes);
                    let detected_encoding = detection.0;
                    let confidence = detection.1;

                    if detected_encoding.eq_ignore_ascii_case("UTF-8") || confidence <= 0.5 {
                        if verbose {
                            println!(
                                "Skipped UTF-8 conversion (detected {} @ {:.0}% confidence): {}",
                                detected_encoding,
                                confidence * 100.0,
                                path.display()
                            );
                        }
                    } else {
                        let encoding = Encoding::for_label(detected_encoding.as_bytes())
                            .or_else(|| Encoding::for_label(b"iso-8859-1"))
                            .unwrap_or(UTF_8);

                        let (decoded, _, had_errors) = encoding.decode(&bytes);
                        fs::write(path, decoded.as_bytes())
                            .context("Failed to write UTF-8 content")?;

                        result.details.push(format!(
                            "encoding converted from {} to utf8 (confidence {:.0}%)",
                            detected_encoding,
                            confidence * 100.0
                        ));

                        if should_echo {
                            println!(
                                "Converted from {} to UTF-8: {}",
                                detected_encoding,
                                path.display()
                            );
                        }

                        if had_errors {
                            result.warnings.push(
                                "Encoding conversion encountered invalid sequences".to_string(),
                            );
                        }
                    }
                }
                EncodingTarget::Latin1 | EncodingTarget::Windows1252 => {
                    let content = fs::read_to_string(path).map_err(|e| {
                        anyhow!(
                            "Failed to read {} as UTF-8 before encoding conversion. {}",
                            path.display(),
                            e
                        )
                    })?;
                    let encoder = match target {
                        EncodingTarget::Latin1 => Encoding::for_label(b"iso-8859-1")
                            .ok_or_else(|| anyhow!("ISO-8859-1 encoding not supported"))?,
                        EncodingTarget::Windows1252 => WINDOWS_1252,
                        EncodingTarget::Utf8 => unreachable!(),
                    };
                    let (encoded, _, had_errors) = encoder.encode(&content);
                    fs::write(path, encoded.into_owned())
                        .context("Failed to write converted content")?;

                    result.details.push(match target {
                        EncodingTarget::Latin1 => "encoding converted to latin1".to_string(),
                        EncodingTarget::Windows1252 => {
                            "encoding converted to windows-1252".to_string()
                        }
                        EncodingTarget::Utf8 => unreachable!(),
                    });

                    if should_echo {
                        println!("Converted file encoding to {}: {}", target, path.display());
                    }

                    if had_errors {
                        result.warnings.push(
                            "Encoding conversion encountered unrepresentable characters"
                                .to_string(),
                        );
                    }
                }
            }
        }
    }

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
        let content = fs::read_to_string(template)
            .with_context(|| format!("Failed to read template file {}", template))?;
        file.write_all(content.as_bytes())
            .context("Failed to write template content to file")?;
        if should_echo {
            println!(
                "Template content {} to: {}",
                if cli.append { "appended" } else { "written" },
                path.display()
            );
        }
        result.details.push("template content applied".to_string());
        result.message = Some(if cli.append {
            "Template content appended".to_string()
        } else {
            "File created from template".to_string()
        });
    } else if let Some(content) = &cli.write {
        file.write_all(content.as_bytes())
            .context("Failed to write content to file")?;
        if should_echo {
            if cli.append {
                println!("Content appended to file: {}", path.display());
            } else {
                println!("File created/updated with content: {}", path.display());
            }
        }
        result.details.push(if cli.append {
            "content appended".to_string()
        } else {
            "content written".to_string()
        });
        result.message = Some(if cli.append {
            "Content appended".to_string()
        } else {
            "File written with provided content".to_string()
        });
    } else if should_echo {
        let metadata = file.metadata().context("Failed to get file metadata")?;
        if metadata.len() == 0 && !existed_before {
            println!("File created: {}", path.display());
        } else {
            println!("File timestamp updated: {}", path.display());
        }
    }

    if result.message.is_none() {
        result.message = Some(if existed_before {
            "File touched".to_string()
        } else {
            "File created".to_string()
        });
    }

    Ok(())
}

fn trim_trailing_whitespace(text: &str) -> (String, bool) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut changed = false;
    let mut output = String::with_capacity(text.len());

    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }

        let mut end = i;
        while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
            end -= 1;
        }
        if end != i {
            changed = true;
        }
        output.push_str(&text[start..end]);

        if i < bytes.len() {
            if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                output.push_str("\r\n");
                i += 2;
            } else {
                output.push(bytes[i] as char);
                i += 1;
            }
        }
    }

    let changed_by_length = output.len() != text.len();
    (output, changed || changed_by_length)
}

fn convert_line_endings(text: &str, conversion: LineEndingConversion) -> (String, bool) {
    match conversion {
        LineEndingConversion::CrlfToLf => {
            let replaced = text.replace("\r\n", "\n").replace('\r', "\n");
            let changed = replaced != text;
            (replaced, changed)
        }
        LineEndingConversion::LfToCrlf => {
            let normalized = text.replace("\r\n", "\n");
            let converted = normalized.replace('\n', "\r\n");
            let changed = converted != text;
            (converted, changed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_preserves_newlines() {
        let (trimmed, changed) = trim_trailing_whitespace("line with spaces   \nnext\r\nlast");
        assert!(changed);
        assert_eq!(trimmed, "line with spaces\nnext\r\nlast");

        let (noop, no_change) = trim_trailing_whitespace("clean\n");
        assert_eq!(noop, "clean\n");
        assert!(!no_change);
    }

    #[test]
    fn line_endings_convert_round_trip() {
        let input = "a\r\nb\r\nc";
        let (lf, changed) = convert_line_endings(input, LineEndingConversion::CrlfToLf);
        assert!(changed);
        assert_eq!(lf, "a\nb\nc");

        let (crlf, roundtrip_changed) = convert_line_endings(&lf, LineEndingConversion::LfToCrlf);
        assert!(roundtrip_changed);
        assert_eq!(crlf, "a\r\nb\r\nc");
    }
}
