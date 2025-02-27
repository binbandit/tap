use serde::{Deserialize, Serialize};

/// Represents the output format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

/// Data structure for JSON/YAML output
#[derive(Serialize, Deserialize)]
pub struct OperationResult {
    pub path: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    pub operation: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Format a vector of operation results in the specified format
pub fn format_results(
    results: &[OperationResult], 
    format: OutputFormat
) -> Result<String, anyhow::Error> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&results)?),
        OutputFormat::Yaml => Ok(serde_yaml::to_string(&results)?),
        OutputFormat::Text => {
            // In text mode, we don't do anything here as output is handled directly
            Ok(String::new())
        }
    }
} 