use std::{
    fmt, fs,
    io::{Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Parser, ValueEnum};
use marklign::{DEFAULT_MATH_WIDTH, FormatOptions, MIN_MATH_WIDTH, MathStyle};
use similar::TextDiff;

mod paths;

use paths::{DEFAULT_EXCLUDE, DEFAULT_INCLUDE, Filters};

/// Reserved, as in Black, for a run the formatter could not complete. A file
/// that merely needs formatting is not an internal error.
const INTERNAL_ERROR: u8 = 123;

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
#[command(
    version,
    about = "Tasteful Markdown and mathematical equation formatting",
    after_help = "Files are reformatted in place. A directory is searched for Markdown, \
                  honoring .gitignore and skipping hidden entries; `-` formats standard \
                  input to standard output. Exit code 1 means a file would be reformatted \
                  under --check or --diff, and 123 that a file could not be formatted."
)]
struct Args {
    /// Markdown files or directories to format; `-` reads standard input.
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Do not write anything; exit 1 if a file would be reformatted.
    #[arg(long)]
    check: bool,

    /// Do not write anything; print a diff of what would change.
    #[arg(long)]
    diff: bool,

    /// Report each file that is already formatted as well.
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    /// Report nothing but errors.
    #[arg(short, long)]
    quiet: bool,

    /// Display math layout: readable separates contextual clauses, compact prefers one line.
    #[arg(long, value_enum, default_value = "readable")]
    math_style: CliMathStyle,

    /// Preferred display-math source line width (long atomic expressions may exceed it).
    #[arg(long, default_value_t = DEFAULT_MATH_WIDTH)]
    math_width: usize,

    /// Regular expression for the files a directory walk formats.
    #[arg(long, default_value = DEFAULT_INCLUDE, value_name = "REGEX")]
    include: String,

    /// Regular expression for the paths a directory walk skips; replaces the default.
    #[arg(long, default_value = DEFAULT_EXCLUDE, value_name = "REGEX")]
    exclude: String,

    /// Further paths a directory walk skips, in addition to --exclude.
    #[arg(long, value_name = "REGEX")]
    extend_exclude: Option<String>,

    /// Paths to skip even when named on the command line.
    #[arg(long, value_name = "REGEX")]
    force_exclude: Option<String>,
}

impl Args {
    /// Whether this run may rewrite files. `--check` and `--diff` only look.
    fn writing(&self) -> bool {
        !self.check && !self.diff
    }
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
            Input::Standard => "-".to_owned(),
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

/// What a run did, and what it says about it. Counted and worded after Black,
/// so that the last line of a build log reads the same way.
struct Report {
    /// True when nothing is written, under `--check` or `--diff`.
    looking_only: bool,
    quiet: bool,
    verbose: bool,
    reformatted: usize,
    unchanged: usize,
    failed: usize,
}

impl Report {
    fn new(args: &Args) -> Self {
        Self {
            looking_only: !args.writing(),
            quiet: args.quiet,
            verbose: args.verbose,
            reformatted: 0,
            unchanged: 0,
            failed: 0,
        }
    }

    fn done(&mut self, name: &str, changed: bool) {
        if changed {
            self.reformatted += 1;
            if !self.quiet {
                let verb = if self.looking_only {
                    "would reformat"
                } else {
                    "reformatted"
                };
                eprintln!("{verb} {name}");
            }
        } else {
            self.unchanged += 1;
            if self.verbose {
                eprintln!("{name} is already formatted");
            }
        }
    }

    fn failed(&mut self, name: &str, error: &str) {
        self.failed += 1;
        eprintln!("error: cannot format {name}: {error}");
    }

    /// A path that could not even be looked at. Counted as a failure: the run
    /// did not see everything it was asked to see.
    fn unreadable(&mut self, problem: &str) {
        self.failed += 1;
        eprintln!("error: {problem}");
    }

    fn exit(&self) -> ExitCode {
        if self.failed > 0 {
            ExitCode::from(INTERNAL_ERROR)
        } else if self.looking_only && self.reformatted > 0 {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn files(count: usize) -> &'static str {
            if count == 1 { "file" } else { "files" }
        }

        let mut parts = Vec::new();
        if self.reformatted > 0 {
            let verb = if self.looking_only {
                "would be reformatted"
            } else {
                "reformatted"
            };
            parts.push(format!(
                "{} {} {verb}",
                self.reformatted,
                files(self.reformatted)
            ));
        }
        if self.unchanged > 0 {
            parts.push(format!(
                "{} {} left unchanged",
                self.unchanged,
                files(self.unchanged)
            ));
        }
        if self.failed > 0 {
            let verb = if self.looking_only {
                "would fail to reformat"
            } else {
                "failed to reformat"
            };
            parts.push(format!("{} {} {verb}", self.failed, files(self.failed)));
        }
        write!(formatter, "{}.", parts.join(", "))
    }
}

/// What the paths on the command line name, and what could not be read while
/// looking for it.
#[derive(Default)]
struct Work {
    inputs: Vec<Input>,
    problems: Vec<String>,
}

/// Collect what the paths on the command line name. A path that does not
/// exist at all is the user's mistake and stops the run before it starts.
fn collect(args: &Args, filters: &Filters) -> Result<Work, String> {
    let mut work = Work::default();
    for path in &args.paths {
        if path.as_os_str() == "-" {
            work.inputs.push(Input::Standard);
            continue;
        }
        let kind = path
            .metadata()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir() {
            if filters.allows_named(path, true) {
                let mut found = Vec::new();
                paths::walk(path, filters, &mut found, &mut work.problems);
                work.inputs.extend(found.into_iter().map(Input::File));
            }
        } else if filters.allows_named(path, false) {
            work.inputs.push(Input::File(path.clone()));
        }
    }
    Ok(work)
}

/// Format one input, returning whether it needed formatting.
fn format(input: &Input, args: &Args, preferences: FormatOptions) -> Result<bool, String> {
    let original = input.read().map_err(|error| error.to_string())?;
    let formatted = marklign::format_document_with_options(&original, preferences)
        .map_err(|error| error.to_string())?;
    let changed = formatted != original;

    if args.diff && changed {
        let name = input.name();
        let difference = TextDiff::from_lines(original.as_str(), formatted.as_str());
        print!(
            "{}",
            difference.unified_diff().header(
                &format!("{name}\t(original)"),
                &format!("{name}\t(marklign)")
            )
        );
    }

    match input {
        // Rewrite a file only when it needs it, so that files already
        // formatted keep their modification time.
        Input::File(path) if args.writing() && changed => {
            fs::write(path, &formatted).map_err(|error| error.to_string())?;
        }
        // Standard input has no file to rewrite; the formatted document is the
        // output of the run, whether or not anything changed.
        Input::Standard if args.writing() => {
            std::io::stdout()
                .write_all(formatted.as_bytes())
                .map_err(|error| error.to_string())?;
        }
        _ => {}
    }
    Ok(changed)
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
    let filters = Filters::new(
        &args.include,
        &args.exclude,
        args.extend_exclude.as_deref(),
        args.force_exclude.as_deref(),
    )?;

    let work = collect(&args, &filters)?;
    let mut report = Report::new(&args);
    for problem in &work.problems {
        report.unreadable(problem);
    }
    if work.inputs.is_empty() && work.problems.is_empty() {
        // Nothing to format is not a failure: a project may hold no Markdown
        // yet, or all of it may be excluded, and a `--check` job should not
        // fail for that. A path that does not exist is an error, and was
        // already reported as one.
        if !args.quiet {
            eprintln!("no Markdown files to format, nothing to do");
        }
        return Ok(ExitCode::SUCCESS);
    }

    for input in &work.inputs {
        let name = input.name();
        match format(input, &args, preferences) {
            Ok(changed) => report.done(&name, changed),
            Err(error) => report.failed(&name, &error),
        }
    }
    if !args.quiet {
        eprintln!("\n{report}");
    }
    Ok(report.exit())
}

fn main() -> ExitCode {
    match run() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("marklign: {error}");
            ExitCode::from(INTERNAL_ERROR)
        }
    }
}
