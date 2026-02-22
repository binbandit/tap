use clap::{Parser, ValueEnum};

use crate::output::OutputFormat;

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum LineEndingConversion {
    #[value(alias = "crlf2lf")]
    CrlfToLf,
    #[value(alias = "lf2crlf")]
    LfToCrlf,
}

impl std::fmt::Display for LineEndingConversion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LineEndingConversion::CrlfToLf => "crlf2lf",
            LineEndingConversion::LfToCrlf => "lf2crlf",
        })
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum EncodingTarget {
    #[value(alias = "utf8")]
    Utf8,
    #[value(alias = "iso-8859-1", alias = "latin1")]
    Latin1,
    #[value(alias = "windows-1252", alias = "win1252")]
    Windows1252,
}

impl std::fmt::Display for EncodingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EncodingTarget::Utf8 => "utf8",
            EncodingTarget::Latin1 => "latin1",
            EncodingTarget::Windows1252 => "windows-1252",
        })
    }
}

#[derive(Parser)]
#[command(name = "tap")]
#[command(about = "A next-gen version of touch with extended capabilities", long_about = None)]
pub struct Cli {
    #[arg(required = true)]
    pub paths: Vec<String>,

    #[arg(short, long, alias = "mkdir")]
    pub dir: bool,

    #[arg(short, long)]
    pub chmod: Option<String>,

    #[arg(short, long)]
    pub write: Option<String>,

    #[arg(short, long)]
    pub timestamp: Option<String>,

    #[arg(short, long)]
    pub append: bool,

    #[arg(short, long)]
    pub verbose: bool,

    #[arg(short = 'R', long, requires = "chmod")]
    pub recursive: bool,

    #[arg(long)]
    pub template: Option<String>,

    #[arg(long)]
    pub trim: bool,

    #[arg(long, alias = "exists")]
    pub check: bool,

    #[arg(long = "no-parent")]
    pub no_parent: bool,

    #[arg(long)]
    pub line_endings: Option<LineEndingConversion>,

    #[arg(long)]
    pub encoding: Option<EncodingTarget>,

    #[arg(long = "timestamp-format")]
    pub timestamp_format: Option<String>,

    #[arg(long = "output-format", default_value = "text")]
    pub output_format: OutputFormat,
}
