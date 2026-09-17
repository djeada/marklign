use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, ValueEnum};
use marklign::{DEFAULT_MATH_WIDTH, FormatOptions, MIN_MATH_WIDTH, MathStyle};

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

/// Directory names a formatter has no business walking into.
const SKIPPED: [&str; 2] = ["target", "node_modules"];

/// Extensions recognized when a directory is walked. A file named on the
/// command line is formatted whatever it is called.
const EXTENSIONS: [&str; 2] = ["md", "markdown"];

#[derive(Parser)]
#[command(
    version,
    about = "Tasteful Markdown and mathematical equation formatting",
    after_help = "A directory is searched for .md and .markdown files, skipping hidden \
                  entries, target, and node_modules. `-` reads standard input."
)]
struct Args {
    /// Markdown files or directories to format; `-` reads standard input.
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Write the formatted Markdown back to each file that needs it.
    #[arg(long, conflicts_with = "check")]
    write: bool,

    /// Report files that need formatting and exit 1; change nothing.
    #[arg(long)]
    check: bool,

    /// Display math layout: readable separates contextual clauses, compact prefers one line.
    #[arg(long, value_enum, default_value = "readable")]
    math_style: CliMathStyle,

    /// Preferred display-math source line width (long atomic expressions may exceed it).
    #[arg(long, default_value_t = DEFAULT_MATH_WIDTH)]
    math_width: usize,
}

/// One thing to format: a file, or standard input.
enum Input {
    File(PathBuf),
    Standard,
}

impl Input {
    fn name(&self) -> String {
        match self {
            Input::File(path) => path.display().to_string(),
            Input::Standard => "<stdin>".to_owned(),
        }
    }

    fn read(&self) -> std::io::Result<String> {
        match self {
            Input::File(path) => fs::read_to_string(path),
            Input::Standard => {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text)?;
                Ok(text)
            }
        }
    }
}

/// Collect the files named, walking directories in a stable order.
fn collect(path: &Path, inputs: &mut Vec<Input>) -> std::io::Result<()> {
    if path == Path::new("-") {
        inputs.push(Input::Standard);
        return Ok(());
    }
    if !path.metadata()?.is_dir() {
        inputs.push(Input::File(path.to_owned()));
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(path)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIPPED.contains(&name.as_ref()) {
            continue;
        }
        // Checked rather than followed: a symbolic link can point at an
        // ancestor, and walking it would never end.
        let child = entry.path();
        if entry.file_type()?.is_dir() {
            collect(&child, inputs)?;
        } else if child
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .is_some_and(|extension| EXTENSIONS.contains(&extension.as_str()))
        {
            inputs.push(Input::File(child));
        }
    }
    Ok(())
}

/// Format one input, returning whether it was already formatted.
fn format(input: &Input, args: &Args, preferences: FormatOptions) -> Result<bool, String> {
    let original = input.read().map_err(|error| error.to_string())?;
    let formatted = marklign::format_document_with_options(&original, preferences)
        .map_err(|error| error.to_string())?;
    let unchanged = formatted == original;

    match input {
        // Rewrite a file only when it needs it, so that unrelated files keep
        // their modification time.
        Input::File(path) if args.write && !unchanged => {
            fs::write(path, &formatted).map_err(|error| error.to_string())?;
            eprintln!("formatted {}", path.display());
        }
        _ if args.write || args.check => {}
        _ => {
            std::io::stdout()
                .write_all(formatted.as_bytes())
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(unchanged)
}

fn run() -> Result<ExitCode, String> {
    let args = Args::parse();
    if args.math_width < MIN_MATH_WIDTH {
        return Err(format!("--math-width must be at least {MIN_MATH_WIDTH}"));
    }
    let preferences = FormatOptions {
        math_style: args.math_style.into(),
        math_width: args.math_width,
    };

    let mut inputs = Vec::new();
    for path in &args.paths {
        collect(path, &mut inputs).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    if inputs.is_empty() {
        return Err("no Markdown files found".to_owned());
    }
    if !args.write && !args.check && inputs.len() > 1 {
        return Err(
            "printing to standard output needs a single input; use --write or --check".to_owned(),
        );
    }
    if args.write && inputs.iter().any(|input| matches!(input, Input::Standard)) {
        return Err("--write needs a file; standard input has nowhere to go".to_owned());
    }

    let mut unformatted = 0;
    let mut failed = 0;
    for input in &inputs {
        match format(input, &args, preferences) {
            Ok(true) => {}
            Ok(false) => {
                unformatted += 1;
                if args.check {
                    eprintln!("needs formatting: {}", input.name());
                }
            }
            Err(error) => {
                failed += 1;
                eprintln!("marklign: {}: {error}", input.name());
            }
        }
    }

    if failed > 0 || (args.check && unformatted > 0) {
        if args.check && unformatted > 0 {
            eprintln!("{unformatted} of {} files need formatting", inputs.len());
        }
        return Ok(ExitCode::FAILURE);
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
