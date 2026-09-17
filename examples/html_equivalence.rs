// Temporary harness: rendered-HTML equivalence over a corpus of files.
// Prose and structure must be identical; mathematics may differ only in
// whitespace, which is exactly what reflowing an equation changes.
use std::{env, fs};

fn html(input: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.math_dollars = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.alerts = true;
    options.extension.front_matter_delimiter = Some("---".into());
    comrak::markdown_to_html(input, &options)
}

const OPEN: &str = "<span data-math-style=";
const CLOSE: &str = "</span>";

fn split_math(rendered: &str) -> (String, String) {
    let (mut prose, mut math) = (String::new(), String::new());
    let mut rest = rendered;
    while let Some(at) = rest.find(OPEN) {
        prose.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.find(CLOSE).map_or(rest.len(), |e| e + CLOSE.len());
        math.push_str(&rest[..end]);
        rest = &rest[end..];
    }
    prose.push_str(rest);
    (
        prose.split_whitespace().collect::<Vec<_>>().join(" "),
        math.split_whitespace().collect(),
    )
}

fn main() {
    let mut prose_bad = 0;
    let mut math_bad = 0;
    for path in env::args().skip(1) {
        let Ok(input) = fs::read_to_string(&path) else {
            continue;
        };
        let formatted = marklign::format_document(&input).expect("format");
        let (prose_before, math_before) = split_math(&html(&input));
        let (prose_after, math_after) = split_math(&html(&formatted));
        for (label, before, after) in [
            ("PROSE", &prose_before, &prose_after),
            ("MATH", &math_before, &math_after),
        ] {
            if before == after {
                continue;
            }
            if label == "PROSE" {
                prose_bad += 1
            } else {
                math_bad += 1
            }
            let at = before
                .chars()
                .zip(after.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            let clip = |text: &String| {
                text.chars()
                    .skip(at.saturating_sub(70))
                    .take(150)
                    .collect::<String>()
            };
            println!(
                "{label} {path}\n  before: {}\n  after:  {}",
                clip(before),
                clip(after)
            );
        }
    }
    println!("prose mismatches: {prose_bad}, math mismatches: {math_bad}");
}
