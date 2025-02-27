use clap::Parser;
use crate::output::OutputFormat;

/// The command-line interface for the tap utility
#[derive(Parser)]
#[command(name = "tap")]
#[command(about = "A next-gen version of touch with extended capabilities", long_about = None)]
pub struct Cli {
    /// File(s) or directory to create or update (supports glob patterns)
    #[arg(required = true)]
    pub paths: Vec<String>,

    /// Create a directory instead of a file
    #[arg(short, long)]
    pub dir: bool,

    /// Set specific permissions (octal format, e.g., 644)
    #[arg(short, long)]
    pub chmod: Option<String>,

    /// Add content to the file
    #[arg(short, long)]
    pub write: Option<String>,

    /// Set access and modification times (format: YYYY-MM-DD HH:MM:SS by default)
    #[arg(short, long)]
    pub timestamp: Option<String>,

    /// Append content instead of overwriting
    #[arg(short, long)]
    pub append: bool,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Apply chmod recursively (only works with directories)
    #[arg(short = 'R', long)]
    pub recursive: bool,

    /// Use a template file for content
    #[arg(long)]
    pub template: Option<String>,

    /// Remove trailing whitespace from each line
    #[arg(long)]
    pub trim: bool,

    /// Check if the file or directory exists (dry run)
    #[arg(long)]
    pub check: bool,
    
    /// Force creation of parent directories without confirmation
    #[arg(short, long)]
    pub force: bool,
    
    /// Convert line endings (values: crlf2lf, lf2crlf)
    #[arg(long)]
    pub line_endings: Option<String>,
    
    /// Convert file encoding (values: utf8, latin1, etc.)
    #[arg(long)]
    pub encoding: Option<String>,
    
    /// Custom timestamp format (e.g., "%Y/%m/%d %H:%M", default: "%Y-%m-%d %H:%M:%S")
    #[arg(long)]
    pub timestamp_format: Option<String>,
    
    /// Output format (values: text, json, yaml)
    #[arg(long, default_value = "text")]
    pub output_format: OutputFormat,
} 