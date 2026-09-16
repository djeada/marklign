use std::{error::Error, fs, io::Write, path::PathBuf, process::ExitCode};

use clap::Parser;

#[derive(Parser)]
#[command(version, about = "Format Markdown and simple display-math equations")]
struct Args {
    /// Markdown file to format.
    path: PathBuf,

    /// Write the formatted Markdown back to the input file.
    #[arg(long, conflicts_with = "check")]
    write: bool,

    /// Exit with status 1 when the file needs formatting; do not change it.
    #[arg(long)]
    check: bool,
}

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let args = Args::parse();
    let original = fs::read_to_string(&args.path)?;
    let formatted = marklign::format_document(&original)?;

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
