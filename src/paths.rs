//! Which files Marklign formats.
//!
//! A directory is walked the way Black walks a project: `.gitignore` files are
//! honored, hidden entries are skipped, and every remaining path is matched
//! against an include and an exclude regular expression. Patterns are matched
//! against the path relative to the directory being walked, written with `/`
//! separators, beginning with `/`, and ending with `/` for a directory — so
//! `vendor/` excludes a directory anywhere below, and an unanchored pattern
//! means what it looks like.

use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};

/// Files a directory walk offers to the formatter. Case-insensitive, because
/// `README.MD` is the same document as `README.md` and half the world writes
/// it that way. A file named on the command line is formatted whatever it is
/// called.
pub const DEFAULT_INCLUDE: &str = r"(?i)\.(md|markdown)$";

/// Directories a formatter has no business walking into.
pub const DEFAULT_EXCLUDE: &str =
    r"/(\.git|\.hg|\.svn|\.venv|venv|node_modules|target|build|dist|_build)/";

/// The include and exclude patterns of one run.
#[derive(Clone)]
pub struct Filters {
    include: Regex,
    exclude: Regex,
    extend_exclude: Option<Regex>,
    force_exclude: Option<Regex>,
}

impl Filters {
    pub fn new(
        include: &str,
        exclude: &str,
        extend_exclude: Option<&str>,
        force_exclude: Option<&str>,
    ) -> Result<Self, String> {
        Ok(Self {
            include: compile("--include", include)?,
            exclude: compile("--exclude", exclude)?,
            extend_exclude: extend_exclude
                .map(|pattern| compile("--extend-exclude", pattern))
                .transpose()?,
            force_exclude: force_exclude
                .map(|pattern| compile("--force-exclude", pattern))
                .transpose()?,
        })
    }

    /// Whether a path named on the command line survives `--force-exclude`.
    /// Nothing else applies to it: a named file is one the user chose.
    pub fn allows_named(&self, path: &Path, is_dir: bool) -> bool {
        let name = key(path, Path::new(""), is_dir);
        !self
            .force_exclude
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(&name))
    }

    /// Whether a path found by walking is excluded, directory or file.
    fn excluded(&self, name: &str) -> bool {
        self.exclude.is_match(name)
            || [&self.extend_exclude, &self.force_exclude]
                .into_iter()
                .flatten()
                .any(|pattern| pattern.is_match(name))
    }

    fn wanted(&self, name: &str) -> bool {
        self.include.is_match(name) && !self.excluded(name)
    }
}

/// Compile one pattern. Whitespace is insignificant, as in Black, so a long
/// exclude pattern can be written legibly.
fn compile(flag: &str, pattern: &str) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .ignore_whitespace(true)
        .build()
        .map_err(|error| format!("{flag} is not a valid regular expression: {error}"))
}

/// The text an include or exclude pattern is matched against.
fn key(path: &Path, root: &Path, is_dir: bool) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut name = String::new();
    for component in relative.components() {
        // A prefix, a root, `.`, and `..` say where the walk started, not what
        // the file is called, and a pattern should not have to know them.
        if let Component::Normal(part) = component {
            name.push('/');
            name.push_str(&part.to_string_lossy());
        }
    }
    if name.is_empty() {
        name.push('/');
    }
    if is_dir && !name.ends_with('/') {
        name.push('/');
    }
    name
}

/// Collect the formattable files under a directory, in a stable order.
///
/// A path that cannot be read is reported and the walk continues: one
/// unreadable directory is no reason to leave the rest of a project
/// unformatted.
pub fn walk(root: &Path, filters: &Filters, found: &mut Vec<PathBuf>, problems: &mut Vec<String>) {
    let mut builder = WalkBuilder::new(root);
    builder
        // Hidden entries and anything a .gitignore covers are not the
        // formatter's business; `.ignore` files and the user's global excludes
        // are a search tool's convention, not a formatter's.
        .hidden(true)
        .ignore(false)
        .git_global(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        // Followed, a symbolic link can point at an ancestor and the walk
        // would never end.
        .follow_links(false)
        .sort_by_file_path(Path::cmp);

    let pruned = filters.clone();
    let start = root.to_owned();
    builder.filter_entry(move |entry| {
        !entry.file_type().is_some_and(|kind| kind.is_dir())
            || !pruned.excluded(&key(entry.path(), &start, true))
    });

    for entry in builder.build() {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_some_and(|kind| kind.is_file())
                    && filters.wanted(&key(entry.path(), root, false))
                {
                    found.push(entry.into_path());
                }
            }
            // The message names the path it is about.
            Err(error) => problems.push(error.to_string()),
        }
    }
}
