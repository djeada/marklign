//! End-to-end behavior of the command-line interface.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const BROKEN: &str = "x\n\n$$\na\n=\nb\n$$\n";
const FORMATTED: &str = "x\n\n$$\na = b\n$$\n";

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

fn marklign<I, S>(arguments: I) -> (bool, String, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new(env!("CARGO_BIN_EXE_marklign"))
        .args(arguments)
        .output()
        .expect("runs");
    (
        output.status.success(),
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        String::from_utf8(output.stderr).expect("utf-8 stderr"),
    )
}

#[test]
fn prints_a_single_file_without_changing_it() {
    let sandbox = Sandbox::new("print");
    let path = sandbox.write("notes.md", BROKEN);
    let (success, stdout, _) = marklign([&path]);
    assert!(success);
    assert_eq!(stdout, FORMATTED);
    assert_eq!(sandbox.read("notes.md"), BROKEN, "input was modified");
}

#[test]
fn prints_an_already_formatted_file_too() {
    let sandbox = Sandbox::new("print-formatted");
    let path = sandbox.write("notes.md", FORMATTED);
    let (success, stdout, _) = marklign([&path]);
    assert!(success);
    assert_eq!(stdout, FORMATTED);
}

#[test]
fn formats_standard_input() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_marklign"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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

    let (success, _, stderr) = marklign([sandbox.path().as_os_str(), "--write".as_ref()]);
    assert!(success, "{stderr}");
    assert_eq!(sandbox.read("notes.md"), FORMATTED);
    assert_eq!(sandbox.read("deep/other.markdown"), FORMATTED);
    assert_eq!(sandbox.read("deep/notes.txt"), BROKEN);
    assert_eq!(sandbox.read("target/ignored.md"), BROKEN);
    assert_eq!(sandbox.read(".hidden/ignored.md"), BROKEN);
}

#[test]
fn check_reports_every_file_and_fails() {
    let sandbox = Sandbox::new("check");
    sandbox.write("broken.md", BROKEN);
    sandbox.write("fine.md", FORMATTED);

    let (success, stdout, stderr) = marklign([sandbox.path().as_os_str(), "--check".as_ref()]);
    assert!(!success);
    assert!(stdout.is_empty(), "check wrote to stdout: {stdout:?}");
    assert!(stderr.contains("broken.md"), "{stderr}");
    assert!(!stderr.contains("fine.md"), "{stderr}");
    assert!(stderr.contains("1 of 2 files need formatting"), "{stderr}");
    assert_eq!(sandbox.read("broken.md"), BROKEN, "check modified a file");

    let (success, _, _) = marklign([
        sandbox.path().join("fine.md").as_os_str(),
        "--check".as_ref(),
    ]);
    assert!(success);
}

#[test]
fn several_inputs_need_write_or_check() {
    let sandbox = Sandbox::new("several");
    let first = sandbox.write("first.md", BROKEN);
    let second = sandbox.write("second.md", BROKEN);
    let (success, _, stderr) = marklign([&first, &second]);
    assert!(!success);
    assert!(stderr.contains("single input"), "{stderr}");
}

#[test]
fn a_missing_file_is_reported_by_name() {
    let (success, _, stderr) = marklign(["./does-not-exist.md"]);
    assert!(!success);
    assert!(stderr.contains("does-not-exist.md"), "{stderr}");
}

#[test]
fn math_width_and_style_reach_the_formatter() {
    let sandbox = Sandbox::new("options");
    let path = sandbox.write("notes.md", "$$\nalpha + beta + gamma + delta\n$$\n");

    let (success, narrow, _) = marklign([path.as_os_str(), "--math-width".as_ref(), "20".as_ref()]);
    assert!(success);
    assert!(narrow.contains("alpha + beta +\n"), "actual: {narrow:?}");

    let (success, _, stderr) = marklign([path.as_os_str(), "--math-width".as_ref(), "8".as_ref()]);
    assert!(!success);
    assert!(stderr.contains("--math-width"), "{stderr}");

    let (success, compact, _) = marklign([
        sandbox
            .write("clause.md", "$$\nu = g\n\\qquad\n\\text{on } X\n$$\n")
            .as_os_str(),
        "--math-style".as_ref(),
        "compact".as_ref(),
    ]);
    assert!(success);
    assert_eq!(compact, "$$\nu = g \\qquad \\text{on } X\n$$\n");
}

#[test]
fn write_and_check_cannot_be_combined() {
    let sandbox = Sandbox::new("conflict");
    let path = sandbox.write("notes.md", BROKEN);
    let (success, _, stderr) = marklign([path.as_os_str(), "--write".as_ref(), "--check".as_ref()]);
    assert!(!success);
    assert!(stderr.contains("cannot be used with"), "{stderr}");
}
