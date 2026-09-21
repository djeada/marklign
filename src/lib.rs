//! Opinionated Markdown formatting with syntax-aware display-math layout.

mod environments;
mod math;
mod preparse;

use comrak::{Arena, Options, format_commonmark, nodes::NodeValue, parse_document};

/// How Marklign lays out ordinary display equations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MathStyle {
    /// Reflow broken source lines, keeping `\qquad` with the clause it introduces.
    #[default]
    Readable,
    /// Prefer a single line unless the expression exceeds `math_width`.
    Compact,
}

/// Narrowest accepted `math_width`; below this even short terms cannot fit.
pub const MIN_MATH_WIDTH: usize = 20;

/// Default preferred width of a display-equation source line.
pub const DEFAULT_MATH_WIDTH: usize = 88;

/// Formatting preferences. Long indivisible TeX tokens may exceed `math_width`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatOptions {
    pub math_style: MathStyle,
    pub math_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            math_style: MathStyle::Readable,
            math_width: DEFAULT_MATH_WIDTH,
        }
    }
}

/// Format a Markdown document with the default readable math style.
pub fn format_document(input: &str) -> Result<String, std::fmt::Error> {
    format_document_with_options(input, FormatOptions::default())
}

/// Format Markdown, leaving code blocks, raw HTML, and inline math alone.
///
/// Every display equation that owns its source lines is formatted and held
/// aside during Markdown parsing: to a block parser an isolated `=` line
/// inside one is a setext heading underline, which would destroy the
/// equation. Each is restored afterwards with the delimiters its author
/// chose. Display math that shares a line with prose is reflowed in the
/// syntax tree instead. Inline `\(...\)` spans, which a CommonMark parser
/// reads as escaped parentheses, are held aside and put back verbatim.
/// Unknown environments, comments, metadata, and malformed groups are left
/// unchanged by the math pass rather than risking changed mathematical
/// meaning.
pub fn format_document_with_options(
    input: &str,
    preferences: FormatOptions,
) -> Result<String, std::fmt::Error> {
    let preferences = FormatOptions {
        math_width: preferences.math_width.max(MIN_MATH_WIDTH),
        ..preferences
    };

    let options = comrak_options();
    let held = preparse::extract(input, preferences, &options);
    let inline = preparse::extract_inline(&held.text, &options);
    let rendered = render(&inline.text, preferences, &options)?;
    let restored = inline
        .restore(&rendered)
        .and_then(|rendered| held.restore(&rendered));
    let output = match restored {
        Some(output) => output,
        // A placeholder did not survive rendering. Rather than emit a
        // document with an equation missing, format without extraction.
        None => render(input, preferences, &options)?,
    };
    Ok(with_line_endings_of(output, input))
}

/// Keep the document's own line endings. A Markdown parser reads CRLF and
/// writes LF, which would rewrite every line of a document written on
/// Windows, and leave `--check` unable to ever pass there.
fn with_line_endings_of(output: String, input: &str) -> String {
    let carriage_returns = match input.find('\n') {
        Some(at) => input[..at].ends_with('\r'),
        None => false,
    };
    if !carriage_returns {
        return output;
    }
    output.replace("\r\n", "\n").replace('\n', "\r\n")
}

pub(crate) fn comrak_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.math_dollars = true;
    // Comrak escapes a literal bracket in prose as `\[`, which the LaTeX math
    // extension then reads back as display math: `\[Optional\]` would become
    // `$$Optional$$`. The extension also restyles `\(x\)` as `$x$`, which
    // Marklign promises not to do. It handles both delimiter pairs itself.
    options.extension.math_latex = false;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.alerts = true;
    options.extension.front_matter_delimiter = Some("---".into());
    options.render.width = 0;
    options.render.prefer_fenced = true;
    options
}

fn render(
    markdown: &str,
    preferences: FormatOptions,
    options: &Options<'_>,
) -> Result<String, std::fmt::Error> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, options);

    for node in root.descendants() {
        if let NodeValue::Math(ref mut math) = node.data_mut().value {
            if math.display_math && math.dollar_math {
                math.literal =
                    environments::format_environment(&math.literal).unwrap_or_else(|| {
                        math::format_display_math(
                            &math.literal,
                            preferences.math_style,
                            preferences.math_width,
                        )
                    });
            }
        }
    }

    let mut output = String::new();
    format_commonmark(root, options, &mut output)?;
    Ok(tidy(&output, options))
}

/// Two blemishes of the Markdown serializer, cleaned up outside code blocks
/// and raw HTML, where every character is the author's:
///
/// * Comrak separates a list from a following code block with an HTML
///   comment. No author of Markdown wrote it and, because Marklign always
///   emits fenced code, none is needed: a fence at the margin ends the list.
/// * A blank line inside a list item or block quote keeps the container's
///   prefix, leaving trailing whitespace behind.
fn tidy(rendered: &str, options: &Options<'_>) -> String {
    const SEPARATOR: &str = "<!-- end list -->";

    let verbatim = preparse::verbatim_lines(rendered, options);
    let lines: Vec<&str> = rendered.split_inclusive('\n').collect();
    let mut output = String::with_capacity(rendered.len());
    let mut index = 0;

    while index < lines.len() {
        if matches!(
            verbatim[index],
            Some(preparse::Verbatim::Code | preparse::Verbatim::FrontMatter)
        ) {
            output.push_str(lines[index]);
            index += 1;
            continue;
        }

        let line = lines[index].trim_end();
        if let Some(prefix) = line
            .strip_suffix(SEPARATOR)
            .filter(|_| verbatim[index] == Some(preparse::Verbatim::Html))
        {
            let blank = lines.get(index + 1).filter(|line| line.trim().is_empty());
            let skipped = if blank.is_some() { 2 } else { 1 };
            let fenced = lines
                .get(index + skipped)
                .and_then(|next| next.trim_end().strip_prefix(prefix))
                .is_some_and(|code| code.starts_with("```") || code.starts_with("~~~"));
            if fenced {
                index += skipped;
                continue;
            }
        }

        // Trailing whitespace is content on a line that has content: two
        // spaces at its end are a hard line break.
        if line.chars().all(|ch| matches!(ch, '>' | ' ' | '\t')) {
            output.push_str(line.trim_end_matches([' ', '\t']));
            output.push('\n');
        } else {
            output.push_str(lines[index]);
        }
        index += 1;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{FormatOptions, MathStyle, format_document, format_document_with_options};
    use crate::math::format_display_math;

    const ROBIN: &str = "\\alpha u\n+\n\\beta\\frac{\\partial u}{\\partial n} =\ng\n\\qquad\n\\text{on } \\partial\\Omega.";
    const ROBIN_READABLE: &str = "\\alpha u + \\beta \\frac{\\partial u}{\\partial n} = g\n\\qquad \\text{on } \\partial\\Omega.";

    #[test]
    fn robin_boundary_condition_is_not_left_broken() {
        assert_eq!(
            format_display_math(ROBIN, MathStyle::Readable, 88),
            ROBIN_READABLE
        );
    }

    #[test]
    fn compact_style_keeps_short_equation_on_one_line() {
        assert_eq!(
            format_display_math(ROBIN, MathStyle::Compact, 88),
            ROBIN_READABLE.replace('\n', " ")
        );
    }

    #[test]
    fn joins_isolated_operators_and_keeps_unary_minus() {
        assert_eq!(
            format_display_math("a\n+\nb\n=\n-c", MathStyle::Readable, 88),
            "a + b = -c"
        );
        assert_eq!(
            format_display_math("a\n-\nb", MathStyle::Readable, 88),
            "a - b"
        );
    }

    #[test]
    fn formats_dirichlet_and_neumann() {
        assert_eq!(
            format_display_math(
                "u=g\n\\qquad\n\\text{on } \\partial\\Omega.",
                MathStyle::Readable,
                88
            ),
            "u = g\n\\qquad \\text{on } \\partial\\Omega."
        );
        assert_eq!(
            format_display_math(
                "\\frac{\\partial u}{\\partial n}=g",
                MathStyle::Readable,
                88
            ),
            "\\frac{\\partial u}{\\partial n} = g"
        );
    }

    #[test]
    fn leaves_opaque_groups_and_command_names_intact() {
        assert_eq!(
            format_display_math("\\alpha\nu+\\frac{a + b}{c - d}=g", MathStyle::Readable, 88),
            "\\alpha u + \\frac{a + b}{c - d} = g"
        );
        assert_eq!(
            format_display_math("\\partial\\Omega = g", MathStyle::Readable, 88),
            "\\partial\\Omega = g"
        );
    }

    /// No wrapped line may begin with a character Markdown reads as a list
    /// item, heading, quote, or table row, and none may be an operator by
    /// itself or leave an equality hanging at the end of a line.
    fn assert_markdown_safe(output: &str) {
        for line in output.lines() {
            assert!(!matches!(line.trim(), "+" | "-" | "="), "{output:?}");
            assert!(
                !line.starts_with(['-', '+', '*', '>', '#', '=', '|']),
                "{output:?}"
            );
            assert!(!line.trim_end().ends_with('='), "{output:?}");
        }
    }

    #[test]
    fn wrapped_lines_never_open_with_markdown_syntax() {
        assert_markdown_safe(&format_display_math(ROBIN, MathStyle::Readable, 24));
        assert_markdown_safe(&format_display_math(
            "\\text{Var}(X) = \\left(\\frac{1}{2}\\right)(0 - 50)^2 + \\left(\\frac{1}{2}\\right)(100 - 50)^2",
            MathStyle::Readable,
            60,
        ));
    }

    #[test]
    fn wraps_a_long_sum_after_its_operators() {
        let output = format_display_math(
            "alpha + beta + gamma + delta + epsilon = zeta",
            MathStyle::Readable,
            20,
        );
        assert_eq!(output, "alpha + beta +\ngamma + delta +\nepsilon = zeta");
        assert_markdown_safe(&output);
    }

    #[test]
    fn prefers_a_break_outside_brackets() {
        assert_eq!(
            format_display_math(
                "f(alpha + beta) + g(gamma + delta)",
                MathStyle::Readable,
                24
            ),
            "f(alpha + beta) +\ng(gamma + delta)"
        );
    }

    #[test]
    fn skips_explicit_tex_line_breaks_alignments_and_comments() {
        for input in [
            r"a &= b \\ c &= d",
            r"\begin{aligned} a &= b \\ \end{aligned}",
            "a% commentary\n+ b",
            "\\text{unclosed\n+ b",
            "\\tag{1} x=y",
        ] {
            assert_eq!(format_display_math(input, MathStyle::Readable, 88), input);
        }
    }

    #[test]
    fn document_formats_robin_and_preserves_inline_math() {
        let input = format!("For $\\Omega$:\n\n$$\n{ROBIN}\n$$\n");
        let output = format_document(&input).unwrap();
        assert!(output.contains("$\\Omega$"));
        assert!(output.contains(ROBIN_READABLE));
        assert!(!output.contains("\n+\n"));
        assert!(!output.contains("\n=\n"));
    }

    #[test]
    fn document_can_select_compact_style() {
        let output = format_document_with_options(
            &format!("$$\n{ROBIN}\n$$\n"),
            FormatOptions {
                math_style: MathStyle::Compact,
                math_width: 88,
            },
        )
        .unwrap();
        assert!(output.contains(&ROBIN_READABLE.replace('\n', " ")));
    }

    #[test]
    fn document_formats_aligned_environment_and_is_idempotent() {
        let input =
            "# Math\n\n$$\n\\begin{aligned}\na\n&\n=\nb\n+\nc \\\\\nd &= e\n\\end{aligned}\n$$\n";
        let once = format_document(input).unwrap();
        assert!(
            once.contains("\\begin{aligned}\n  a &= b + c \\\\\n  d &= e\n\\end{aligned}"),
            "actual: {once:?}"
        );
        assert_eq!(once, format_document(&once).unwrap());
    }

    #[test]
    fn document_preserves_unsupported_nested_environment_and_code() {
        let nested = r"\begin{aligned}\begin{cases}x&1\end{cases}\end{aligned}";
        let input = format!("$$\n{nested}\n$$\n\n```latex\n{nested}\n```\n");
        let output = format_document(&input).unwrap();
        assert_eq!(output.matches(nested).count(), 2);
    }

    #[test]
    fn does_not_rewrite_fenced_code() {
        let output = format_document("```latex\na\n+\nb\n```\n").unwrap();
        assert!(output.contains("a\n+\nb"));
    }

    #[test]
    fn preserves_front_matter() {
        let input = "---\ntitle: PDE notes\n---\n\n# Introduction\n";
        let output = format_document(input).unwrap();
        assert!(output.starts_with("---\ntitle: PDE notes\n---"));
    }

    #[test]
    fn formatting_is_idempotent_for_full_boundary_conditions() {
        // `include_str!` hands over the bytes on disk, which a checkout on
        // Windows gives CRLF endings; the expectations below are written with
        // LF, and CRLF has a test of its own.
        let input = include_str!("../examples/boundary_conditions.md").replace("\r\n", "\n");
        let once = format_document(&input).unwrap();
        assert_eq!(once, format_document(&once).unwrap());
        assert!(once.contains(ROBIN_READABLE));
    }
}
