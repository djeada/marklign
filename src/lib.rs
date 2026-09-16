//! Markdown formatting with deliberately conservative display-math changes.

use comrak::{Arena, Options, format_commonmark, nodes::NodeValue, parse_document};

/// Format Markdown while recognizing math, fenced code, tables, and front matter.
///
/// Only display-math lines containing a simple ASCII identifier on either side
/// of a single `=` are modified. Inline and more complex LaTeX are left alone.
pub fn format_document(input: &str) -> Result<String, std::fmt::Error> {
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.front_matter_delimiter = Some("---".into());
    options.render.width = 0;
    options.render.prefer_fenced = true;

    let arena = Arena::new();
    let root = parse_document(&arena, input, &options);

    for node in root.descendants() {
        if let NodeValue::Math(ref mut math) = node.data_mut().value {
            if math.display_math && math.dollar_math {
                math.literal = format_display_math(&math.literal);
            }
        }
    }

    let mut output = String::new();
    format_commonmark(root, &options, &mut output)?;
    Ok(output)
}

fn format_display_math(input: &str) -> String {
    input
        .split_inclusive('\n')
        .map(|part| {
            let (line, newline) = match part.strip_suffix('\n') {
                Some(line) => (line, "\n"),
                None => (part, ""),
            };
            format!("{}{}", format_simple_equation(line), newline)
        })
        .collect()
}

/// Avoid rewriting commands, alignments, relations, or multi-token expressions.
fn format_simple_equation(line: &str) -> String {
    let trimmed = line.trim();
    let Some((lhs, rhs)) = trimmed.split_once('=') else {
        return line.to_owned();
    };
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    if lhs.is_empty()
        || rhs.is_empty()
        || !lhs.chars().all(|ch| ch.is_ascii_alphabetic())
        || !rhs.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return line.to_owned();
    }

    let indentation = &line[..line.len() - line.trim_start().len()];
    format!("{indentation}{lhs} = {rhs}")
}

#[cfg(test)]
mod tests {
    use super::{format_display_math, format_document, format_simple_equation};

    #[test]
    fn spaces_only_simple_equations() {
        assert_eq!(format_simple_equation("u=g"), "u = g");
        assert_eq!(format_simple_equation("  u  =g  "), "  u = g");
        assert_eq!(format_simple_equation("a==b"), "a==b");
    }

    #[test]
    fn preserves_complex_latex() {
        let input = "\\alpha u\n+\n\\beta\\frac{\\partial u}{\\partial n} =\ng\n\\qquad\n\\text{on } \\partial\\Omega.\n";
        assert_eq!(format_display_math(input), input);
    }

    #[test]
    fn preserves_line_breaks_in_math() {
        assert_eq!(format_display_math("u=g\n\\qquad\nx=y\n"), "u = g\n\\qquad\nx = y\n");
    }

    #[test]
    fn recognizes_display_math_without_touching_inline_math() {
        let output = format_document("For $\\Omega$, use:\n\n$$\nu=g\n$$\n").unwrap();
        assert!(output.contains("$\\Omega$"));
        assert!(output.contains("u = g"));
    }

    #[test]
    fn does_not_rewrite_fenced_code() {
        let output = format_document("```latex\nu=g\n```\n").unwrap();
        assert!(output.contains("u=g"));
        assert!(!output.contains("u = g"));
    }

    #[test]
    fn preserves_front_matter() {
        let input = "---\ntitle: PDE notes\n---\n\n# Introduction\n";
        let output = format_document(input).unwrap();
        assert!(output.starts_with("---\ntitle: PDE notes\n---"));
    }

    #[test]
    fn formatting_is_idempotent() {
        let input = "### Boundary conditions\n\nFor $\\Omega$:\n\n$$\nu=g\n$$\n";
        let once = format_document(input).unwrap();
        assert_eq!(once, format_document(&once).unwrap());
    }
}
