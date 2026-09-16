//! Opinionated Markdown formatting with syntax-aware display-math layout.

mod environments;
mod math;

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

/// Formatting preferences. Long indivisible TeX tokens may exceed `math_width`.
#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    pub math_style: MathStyle,
    pub math_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            math_style: MathStyle::Readable,
            math_width: 88,
        }
    }
}

/// Format a Markdown document with the default readable math style.
pub fn format_document(input: &str) -> Result<String, std::fmt::Error> {
    format_document_with_options(input, FormatOptions::default())
}

/// Format Markdown without changing math inside code fences or inline math.
///
/// Ordinary `$$` display equations are reflowed by a TeX-aware tokenizer.
/// Standalone `aligned`, `split`, and `cases` environments have their explicit
/// rows and alignment columns preserved, while each safe row is normalized.
/// Unknown environments, comments, metadata, and malformed groups are left
/// unchanged by the math pass rather than risking changed mathematical meaning.
pub fn format_document_with_options(
    input: &str,
    preferences: FormatOptions,
) -> Result<String, std::fmt::Error> {
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
                math.literal =
                    environments::format_environment(&math.literal).unwrap_or_else(|| {
                        math::format_display_math(
                            &math.literal,
                            preferences.math_style,
                            preferences.math_width.max(20),
                        )
                    });
            }
        }
    }

    let mut output = String::new();
    format_commonmark(root, &options, &mut output)?;
    Ok(output)
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

    #[test]
    fn never_generates_an_operator_only_line() {
        let output = format_display_math(ROBIN, MathStyle::Readable, 24);
        for line in output.lines() {
            assert!(!matches!(line.trim(), "+" | "-" | "="));
            assert!(!line.trim_start().starts_with('='));
            let last = line.trim_end().chars().last();
            assert!(!last.is_some_and(|ch| matches!(ch, '+' | '-' | '=')));
        }
    }

    #[test]
    fn wraps_a_long_sum_with_operators_attached_to_terms() {
        let output = format_display_math(
            "alpha + beta + gamma + delta + epsilon = zeta",
            MathStyle::Readable,
            20,
        );
        assert!(output.contains('\n'));
        for line in output.lines() {
            assert!(!matches!(line.trim(), "+" | "-" | "="));
            assert!(!line.starts_with('='));
        }
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
        assert!(once.contains("\\begin{aligned}\n  a &= b + c \\\\\n  d &= e\n\\end{aligned}"));
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
        let input = include_str!("../examples/boundary_conditions.md");
        let once = format_document(input).unwrap();
        assert_eq!(once, format_document(&once).unwrap());
        assert!(once.contains(ROBIN_READABLE));
    }
}
