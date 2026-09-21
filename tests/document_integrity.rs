//! Whole-document guarantees: an equation must survive Markdown parsing, and
//! prose must come back unchanged. Each case here is a defect Marklign had.

use marklign::format_document;

/// Formatting twice must equal formatting once.
fn formatted_once(input: &str) -> String {
    let output = format_document(input).expect("formats");
    assert_eq!(
        output,
        format_document(&output).expect("formats"),
        "not idempotent for {input:?}"
    );
    output
}

#[test]
fn an_isolated_equals_line_does_not_become_a_heading() {
    let output = formatted_once("Text\n\n$$\nu\n=\ng\n$$\n");
    assert_eq!(output, "Text\n\n$$\nu = g\n$$\n");
}

#[test]
fn an_isolated_minus_line_does_not_become_a_list_or_heading() {
    let output = formatted_once("Text\n\n$$\na\n-\nb\n$$\n");
    assert_eq!(output, "Text\n\n$$\na - b\n$$\n");
}

#[test]
fn equations_inside_lists_and_quotes_keep_their_container() {
    assert_eq!(
        formatted_once("- item\n\n  $$\n  a\n  =\n  b\n  $$\n"),
        "- item\n\n  $$\n  a = b\n  $$\n"
    );
    assert_eq!(
        formatted_once("> quote\n>\n> $$\n> a\n> =\n> b\n> $$\n"),
        "> quote\n>\n> $$\n> a = b\n> $$\n"
    );
}

#[test]
fn bracket_delimiters_are_formatted_and_kept() {
    let output = formatted_once("Text\n\n\\[\nu\n=\ng\n\\]\n");
    assert_eq!(output, "Text\n\n\\[\nu = g\n\\]\n");
}

#[test]
fn escaped_brackets_in_prose_are_not_mathematics() {
    for input in [
        "- `--output`: \\[Optional\\] Base path.\n",
        "Only an integer in \\[0,255\\] is allowed.\n",
    ] {
        assert_eq!(formatted_once(input), input);
    }
}

#[test]
fn delimiters_move_onto_their_own_lines_only_when_the_equation_wraps() {
    let short = "$$a = b$$\n";
    assert_eq!(formatted_once(short), short);

    let long = format!("$$\\alpha = {}$$\n", "\\beta + ".repeat(12));
    let wrapped = formatted_once(&long);
    assert!(wrapped.starts_with("$$\n"), "actual: {wrapped:?}");
    assert!(wrapped.ends_with("\n$$\n"), "actual: {wrapped:?}");
    for line in wrapped.lines() {
        assert!(
            !line.starts_with(['-', '+', '*', '>', '#', '=', '|']) || line == "$$",
            "Markdown would claim this line: {line:?}"
        );
    }
}

#[test]
fn a_hard_line_break_after_an_equation_is_kept() {
    let output = formatted_once("It is written as:\n\n$$u = g$$  \nwhere $u$ is a solution.\n");
    assert!(output.contains("$$u = g$$  \n"), "actual: {output:?}");
}

#[test]
fn a_list_before_fenced_code_gains_no_html_comment() {
    let output = formatted_once("- item\n- other\n\n```sql\nSELECT 1;\n```\n");
    assert!(!output.contains("end list"), "actual: {output:?}");
}

#[test]
fn github_alerts_survive() {
    let output = formatted_once("> [!NOTE]\n> Useful information.\n");
    assert!(output.contains("[!NOTE]"), "actual: {output:?}");
}

#[test]
fn code_blocks_and_raw_html_are_never_reflowed() {
    for input in [
        "```latex\n$$\na\n=\nb\n$$\n```\n",
        "<pre>\n$$\na\n=\nb\n$$\n</pre>\n",
    ] {
        assert_eq!(formatted_once(input), input);
    }
}

#[test]
fn front_matter_and_tables_are_preserved() {
    let input = "---\ntitle: PDE notes\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
    let output = formatted_once(input);
    assert!(output.starts_with("---\ntitle: PDE notes\n---"));
    assert!(output.contains("| a | b |"), "actual: {output:?}");
}

#[test]
fn a_document_keeps_its_own_line_endings() {
    let output = formatted_once("Text\r\n\r\n$$\r\nu\r\n=\r\ng\r\n$$\r\n");
    assert_eq!(output, "Text\r\n\r\n$$\r\nu = g\r\n$$\r\n");

    let unix = formatted_once("Text\n\n$$\nu\n=\ng\n$$\n");
    assert!(!unix.contains('\r'), "actual: {unix:?}");
}

/// A blank line ends the Markdown block the equation lives in, so its `=` and
/// `*` lines are left for the block parser to read as a heading and a list.
/// Marklign holds such an equation aside and puts it back verbatim.
#[test]
fn a_blank_line_inside_an_equation_does_not_hand_it_to_the_block_parser() {
    let output = formatted_once("$$\n= a\n\n* b\n  $$\n");
    assert_eq!(output, "$$\n= a\n\n* b\n$$\n");

    let tex = "$$\n\\frac{f(x+h) - f(x-h)}{2h}\n\n* \\text{error} = O(h^2)\n$$\n";
    assert_eq!(formatted_once(tex), tex);
}

#[test]
fn a_blank_line_is_survived_inside_quotes_lists_and_bracket_delimiters() {
    for input in [
        "> $$\n> = a\n>\n> * b\n> $$\n",
        "- item\n\n  $$\n  = a\n\n  * b\n  $$\n",
        "\\[\n= a\n\n* b\n\\]\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
}

/// Comrak escapes a prose bracket as `\[...\]`. Searching on past a blank
/// line for a closing delimiter must not let a paragraph that merely ends in
/// one swallow everything back to such a bracket.
#[test]
fn prose_brackets_do_not_open_an_equation_that_runs_on_past_a_blank_line() {
    let input = "See the policy\n\\[TODO: write it, then link it\\].\n\nOther prose.\n\nReport it here.\\]\n";
    assert_eq!(formatted_once(input), input);
}

/// A CommonMark parser reads `\(` as an escaped parenthesis: the delimiters
/// vanish and the TeX between them is escaped as prose.
#[test]
fn inline_latex_math_delimiters_are_preserved() {
    for input in [
        "The learning rate \\(\\alpha\\) controls convergence.\n",
        "A \\(x < y\\) and \\(a_1\\)2 pair.\n",
        "- \\(\\frac{1}{2}\\) of \\(n\\)\n",
        "| \\(a_1\\) | b |\n| --- | --- |\n| c | d |\n",
        "\\(= x\\)\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
}

/// Inline code, dollar math, and a genuine escaped backslash are none of them
/// a span to hold aside.
#[test]
fn inline_latex_delimiters_are_not_read_out_of_code_or_escapes() {
    for input in [
        "Code `\\(x\\)` span and $\\(q\\)$ math.\n",
        "Escaped \\\\(not math\\\\) here.\n",
        "```latex\n\\(\\alpha\\)\n```\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
}

#[test]
fn inline_math_and_a_held_equation_coexist() {
    let input = "Rate \\(\\alpha\\).\n\n$$\nu\n=\ng\n$$\n\nAnd \\(\\beta\\).\n";
    assert_eq!(
        formatted_once(input),
        "Rate \\(\\alpha\\).\n\n$$\nu = g\n$$\n\nAnd \\(\\beta\\).\n"
    );
}

/// A lone `=` line underlines the line above it as a setext heading, which no
/// renderer recovers from: the matrix comes back as an `<h1>` and a paragraph,
/// with the `\\` row breaks eaten as prose escapes. The reflow pass refuses
/// this block -- `\\` and `\begin` carry layout it cannot preserve -- so the
/// repair has to happen outside it.
#[test]
fn an_unreflowable_block_never_keeps_a_line_markdown_would_claim() {
    let input = "$$\n\\begin{bmatrix}\nQ&A^\\top\\\\\nA&0\n\\end{bmatrix}\n\\begin{bmatrix}\nx\\\\\n\\nu\n\\end{bmatrix}\n=\n\\begin{bmatrix}\n-c\\\\\nb\n\\end{bmatrix}.\n$$\n";
    let expected = "$$\n\\begin{bmatrix}\nQ&A^\\top\\\\\nA&0\n\\end{bmatrix}\n\\begin{bmatrix}\nx\\\\\n\\nu\n\\end{bmatrix} =\n\\begin{bmatrix}\n-c\\\\\nb\n\\end{bmatrix}.\n$$\n";
    assert_eq!(formatted_once(input), expected);

    // Every line the block parser would take away from the paragraph.
    for claimed in ["=", "---", "- b", "* b", "+ b", "> b", "# b", "| b", "***"] {
        let output = formatted_once(&format!(
            "$$\n\\begin{{bmatrix}}\na&b\n\\end{{bmatrix}}\n{claimed}\nc\n$$\n"
        ));
        for line in output.lines().skip(1) {
            assert!(
                !matches!(line.trim(), "=" | "-" | "---" | "***"),
                "{claimed:?} left a setext underline: {output:?}"
            );
            assert!(
                !line.starts_with(['-', '+', '*', '>', '#', '|']) || line.starts_with("\\"),
                "{claimed:?} left a block marker: {output:?}"
            );
        }
    }
}

/// A comment would swallow whatever was joined onto its line, so a block that
/// holds one is left exactly as it is.
#[test]
fn a_commented_equation_is_not_rearranged_to_please_markdown() {
    let input = "$$\na % why\n=\nb\n$$\n";
    assert_eq!(formatted_once(input), input);
}
