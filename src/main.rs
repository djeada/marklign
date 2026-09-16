use std::{error::Error, fs, io::Write, path::PathBuf, process::ExitCode};

use clap::{Parser, ValueEnum};
use marklign::{FormatOptions, MathStyle};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliMathStyle {
    Readable,
    Compact,
}

impl From<CliMathStyle> for MathStyle {
    fn from(style: CliMathStyle) -> Self {
        match style {
            CliMathStyle::Readable => MathStyle::Readable,
            CliMathStyle::Compact => MathStyle::Compact,
        }
    }
}

#[derive(Parser)]
#[command(version, about = "Tasteful Markdown and mathematical equation formatting")]
struct Args {
    /// Markdown file to format.
    path: PathBuf,

    /// Write the formatted Markdown back to the input file.
    #[arg(long, conflicts_with = "check")]
    write: bool,

    /// Exit with status 1 when the file needs formatting; do not change it.
    #[arg(long)]
    check: bool,

    /// Display math layout: readable separates contextual clauses, compact prefers one line.
    #[arg(long, value_enum, default_value = "readable")]
    math_style: CliMathStyle,

    /// Preferred display-math source line width (long atomic expressions may exceed it).
    #[arg(long, default_value_t = 88)]
    math_width: usize,
}

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let args = Args::parse();
    if args.math_width < 20 {
        return Err("--math-width must be at least 20".into());
    }

    let original = fs::read_to_string(&args.path)?;
    let formatted = marklign::format_document_with_options(
        &original,
        FormatOptions {
            math_style: args.math_style.into(),
            math_width: args.math_width,
        },
    )?;

    if args.check {
        if original != formatted {
            eprintln!("Needs formatting: {}", args.path.display());
            return Ok(ExitCode::FAILURE);
        }
        return Ok(ExitCode::SUCCESS);
    }

    if args.write {
        if original != formatted {
            fs::write(&args.path, formatted)?;
        }
    } else {
        std::io::stdout().write_all(formatted.as_bytes())?;
    }

    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("marklign: {error}");
            ExitCode::FAILURE
        }
    }
}
