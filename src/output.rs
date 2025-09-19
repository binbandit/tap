use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}

#[derive(Default, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

pub fn format_results(
    results: &[OperationResult],
    format: OutputFormat,
) -> Result<String, anyhow::Error> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&results)?),
        OutputFormat::Yaml => Ok(serde_yaml::to_string(&results)?),
        OutputFormat::Text => Ok(String::new()),
    }
}
