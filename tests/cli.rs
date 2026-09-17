//! End-to-end behavior of the command-line interface.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const BROKEN: &str = "x\n\n$$\na\n=\nb\n$$\n";
const FORMATTED: &str = "x\n\n$$\na = b\n$$\n";

/// The exit code reserved for a run the formatter could not complete.
const INTERNAL_ERROR: i32 = 123;

struct Sandbox(PathBuf);

impl Sandbox {
    /// A directory of its own per test, so the runs cannot collide.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("marklign-cli-{name}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("sandbox");
        Sandbox(path)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(&path, contents).expect("write");
        path
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join(name)).expect("read")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn ok(&self) -> bool {
        self.code == 0
    }
}

fn marklign<I, S>(arguments: I) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new(env!("CARGO_BIN_EXE_marklign"))
        .args(arguments)
        .output()
        .expect("runs");
    Run {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("utf-8 stdout"),
        stderr: String::from_utf8(output.stderr).expect("utf-8 stderr"),
    }
}

#[test]
fn formats_a_file_in_place_by_default() {
    let sandbox = Sandbox::new("write");
    let path = sandbox.write("notes.md", BROKEN);
    let run = marklign([&path]);
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "wrote to stdout: {:?}", run.stdout);
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert!(run.stderr.contains("reformatted"), "{}", run.stderr);
    assert!(run.stderr.contains("1 file reformatted."), "{}", run.stderr);
}

#[test]
fn an_already_formatted_file_is_left_alone_and_counted() {
    let sandbox = Sandbox::new("unchanged");
    let path = sandbox.write("notes.md", FORMATTED);
    let before = fs::metadata(&path).expect("metadata").modified().ok();

    let run = marklign([&path]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert!(
        run.stderr.contains("1 file left unchanged."),
        "{}",
        run.stderr
    );
    assert!(!run.stderr.contains("reformatted"), "{}", run.stderr);
    assert_eq!(
        fs::metadata(&path).expect("metadata").modified().ok(),
        before,
        "a file that needed nothing was rewritten"
    );
}

#[test]
fn formats_standard_input_to_standard_output() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_marklign"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(BROKEN.as_bytes())
        .expect("writes");
    let output = child.wait_with_output().expect("waits");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), FORMATTED);
}

#[test]
fn walks_a_directory_for_markdown_only() {
    let sandbox = Sandbox::new("walk");
    sandbox.write("notes.md", BROKEN);
    sandbox.write("deep/other.markdown", BROKEN);
    sandbox.write("deep/notes.txt", BROKEN);
    sandbox.write("target/ignored.md", BROKEN);
    sandbox.write(".hidden/ignored.md", BROKEN);

    let run = marklign([sandbox.path()]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert_eq!(sandbox.read("deep/other.markdown"), FORMATTED);
    assert_eq!(sandbox.read("deep/notes.txt"), BROKEN);
    assert_eq!(sandbox.read("target/ignored.md"), BROKEN);
    assert_eq!(sandbox.read(".hidden/ignored.md"), BROKEN);
}

#[test]
fn a_directory_is_reformatted_all_the_way_down() {
    let sandbox = Sandbox::new("recursive");
    let buried = [
        "top.md",
        "one/second.md",
        "one/two/third.markdown",
        "one/two/three/fourth.md",
        "one/two/three/four/fifth.md",
        // A document is the same document whatever case its extension is in.
        "one/SIXTH.MD",
        "dir with spaces/seventh.md",
    ];
    for name in buried {
        sandbox.write(name, BROKEN);
    }
    sandbox.write("one/two/notes.rst", BROKEN);

    // A trailing separator is how a shell completes a directory name.
    let mut argument = sandbox.path().as_os_str().to_owned();
    argument.push("/");
    let run = marklign([argument]);

    assert!(run.ok(), "{}", run.stderr);
    for name in buried {
        assert_eq!(sandbox.read(name), FORMATTED, "{name} was not reformatted");
    }
    assert_eq!(sandbox.read("one/two/notes.rst"), BROKEN);
    assert!(
        run.stderr.contains("7 files reformatted."),
        "{}",
        run.stderr
    );

    // And again: a formatter that has run has nothing left to do.
    let run = marklign([sandbox.path().as_os_str(), "--check".as_ref()]);
    assert!(run.ok(), "{}", run.stderr);
}

/// One directory the walk cannot read must not cost the rest of the project
/// its formatting, and must still be loud enough for CI to notice.
#[test]
#[cfg(unix)]
fn an_unreadable_directory_is_reported_and_the_rest_is_formatted() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("unreadable");
    sandbox.write("open.md", BROKEN);
    let locked = sandbox.write("locked/inside.md", BROKEN);
    let directory = locked.parent().expect("parent").to_owned();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o000)).expect("lock");

    let run = marklign([sandbox.path()]);
    // Restore before asserting, so a failure does not leave a sandbox that
    // cannot be cleaned up.
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).expect("unlock");

    assert_eq!(run.code, INTERNAL_ERROR);
    assert!(run.stderr.contains("locked"), "{}", run.stderr);
    assert_eq!(sandbox.read("open.md"), FORMATTED);
}

#[test]
fn a_walk_honors_gitignore_but_a_named_file_is_still_formatted() {
    let sandbox = Sandbox::new("gitignore");
    // A .gitignore is only consulted when a repository owns the files.
    fs::create_dir_all(sandbox.path().join(".git")).expect("git dir");
    sandbox.write(".gitignore", "vendor/\ndraft.md\n");
    sandbox.write("notes.md", BROKEN);
    sandbox.write("draft.md", BROKEN);
    sandbox.write("vendor/library.md", BROKEN);

    let run = marklign([sandbox.path()]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert_eq!(sandbox.read("draft.md"), BROKEN);
    assert_eq!(sandbox.read("vendor/library.md"), BROKEN);

    // Naming a file is a decision the formatter does not second-guess.
    let run = marklign([sandbox.path().join("draft.md")]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("draft.md"), FORMATTED);
}

#[test]
fn include_and_exclude_patterns_select_files() {
    let sandbox = Sandbox::new("patterns");
    sandbox.write("notes.md", BROKEN);
    sandbox.write("page.mdx", BROKEN);
    sandbox.write("vendor/library.md", BROKEN);

    let run = marklign([
        sandbox.path().as_os_str(),
        "--include".as_ref(),
        r"\.(md|mdx)$".as_ref(),
        "--extend-exclude".as_ref(),
        "vendor/".as_ref(),
    ]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert_eq!(sandbox.read("page.mdx"), FORMATTED);
    assert_eq!(sandbox.read("vendor/library.md"), BROKEN);

    // --force-exclude applies to a file named on the command line too.
    let run = marklign([
        sandbox.path().join("vendor/library.md").as_os_str(),
        "--force-exclude".as_ref(),
        "vendor/".as_ref(),
    ]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("vendor/library.md"), BROKEN);
    assert!(run.stderr.contains("nothing to do"), "{}", run.stderr);
}

#[test]
fn an_invalid_pattern_is_reported_as_such() {
    let sandbox = Sandbox::new("bad-pattern");
    let path = sandbox.write("notes.md", BROKEN);
    let run = marklign([path.as_os_str(), "--include".as_ref(), "(".as_ref()]);
    assert_eq!(run.code, INTERNAL_ERROR);
    assert!(run.stderr.contains("--include"), "{}", run.stderr);
    assert_eq!(sandbox.read("notes.md"), BROKEN);
}

#[test]
fn check_reports_every_file_and_fails() {
    let sandbox = Sandbox::new("check");
    sandbox.write("broken.md", BROKEN);
    sandbox.write("fine.md", FORMATTED);

    let run = marklign([sandbox.path().as_os_str(), "--check".as_ref()]);
    assert_eq!(run.code, 1);
    assert!(
        run.stdout.is_empty(),
        "check wrote to stdout: {:?}",
        run.stdout
    );
    assert!(run.stderr.contains("would reformat"), "{}", run.stderr);
    assert!(run.stderr.contains("broken.md"), "{}", run.stderr);
    assert!(!run.stderr.contains("fine.md"), "{}", run.stderr);
    assert!(
        run.stderr
            .contains("1 file would be reformatted, 1 file left unchanged."),
        "{}",
        run.stderr
    );
    assert_eq!(sandbox.read("broken.md"), BROKEN, "check modified a file");

    let run = marklign([
        sandbox.path().join("fine.md").as_os_str(),
        "--check".as_ref(),
    ]);
    assert!(run.ok(), "{}", run.stderr);
}

#[test]
fn diff_prints_the_change_without_making_it() {
    let sandbox = Sandbox::new("diff");
    let path = sandbox.write("notes.md", BROKEN);

    let run = marklign([path.as_os_str(), "--diff".as_ref()]);
    assert_eq!(run.code, 1);
    assert_eq!(sandbox.read("notes.md"), BROKEN, "diff modified a file");
    assert!(run.stdout.contains("--- "), "{}", run.stdout);
    assert!(run.stdout.contains("+++ "), "{}", run.stdout);
    assert!(run.stdout.contains("-a\n"), "{}", run.stdout);
    assert!(run.stdout.contains("+a = b\n"), "{}", run.stdout);

    let run = marklign([
        sandbox.write("fine.md", FORMATTED).as_os_str(),
        "--diff".as_ref(),
    ]);
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

#[test]
fn quiet_says_nothing_and_verbose_names_every_file() {
    let sandbox = Sandbox::new("verbosity");
    sandbox.write("broken.md", BROKEN);
    sandbox.write("fine.md", FORMATTED);

    let run = marklign([
        sandbox.path().as_os_str(),
        "--check".as_ref(),
        "-q".as_ref(),
    ]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.is_empty(), "{}", run.stderr);

    let run = marklign([
        sandbox.path().as_os_str(),
        "--check".as_ref(),
        "-v".as_ref(),
    ]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("fine.md is already formatted"),
        "{}",
        run.stderr
    );
}

#[test]
fn several_paths_are_all_formatted() {
    let sandbox = Sandbox::new("several");
    let first = sandbox.write("first.md", BROKEN);
    let second = sandbox.write("nested/second.md", BROKEN);
    let run = marklign([&first, &second]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(sandbox.read("first.md"), FORMATTED);
    assert_eq!(sandbox.read("nested/second.md"), FORMATTED);
    assert!(
        run.stderr.contains("2 files reformatted."),
        "{}",
        run.stderr
    );
}

#[test]
fn a_directory_without_markdown_is_not_a_failure() {
    let sandbox = Sandbox::new("empty");
    sandbox.write("notes.txt", BROKEN);
    let run = marklign([sandbox.path()]);
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stderr.contains("nothing to do"), "{}", run.stderr);
}

#[test]
fn a_missing_file_is_reported_by_name() {
    let run = marklign(["./does-not-exist.md"]);
    assert_eq!(run.code, INTERNAL_ERROR);
    assert!(run.stderr.contains("does-not-exist.md"), "{}", run.stderr);
}

#[test]
fn math_width_and_style_reach_the_formatter() {
    let sandbox = Sandbox::new("options");
    let path = sandbox.write("notes.md", "$$\nalpha + beta + gamma + delta\n$$\n");

    let run = marklign([path.as_os_str(), "--math-width".as_ref(), "20".as_ref()]);
    assert!(run.ok(), "{}", run.stderr);
    let narrow = sandbox.read("notes.md");
    assert!(narrow.contains("alpha + beta +\n"), "actual: {narrow:?}");

    let run = marklign([path.as_os_str(), "--math-width".as_ref(), "8".as_ref()]);
    assert_eq!(run.code, INTERNAL_ERROR);
    assert!(run.stderr.contains("--math-width"), "{}", run.stderr);

    let clause = sandbox.write("clause.md", "$$\nu = g\n\\qquad\n\\text{on } X\n$$\n");
    let run = marklign([
        clause.as_os_str(),
        "--math-style".as_ref(),
        "compact".as_ref(),
    ]);
    assert!(run.ok(), "{}", run.stderr);
    assert_eq!(
        sandbox.read("clause.md"),
        "$$\nu = g \\qquad \\text{on } X\n$$\n"
    );
}
