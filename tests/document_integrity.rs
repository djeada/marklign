//! Whole-document guarantees: an equation must survive Markdown parsing, and
//! prose must come back unchanged. Each case here is a defect Marklign had.

use marklign::{
    FormatOptions, MathFences, OneLineMath, format_document, format_document_with_options,
};

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
        "- item\n\n  ```math\n  a = b\n  ```\n"
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
fn a_one_line_equation_moves_its_delimiters_onto_their_own_lines() {
    // `$$...$$` inside a paragraph line is display math only to renderers
    // that look for it there; GitHub's mobile app shows the TeX instead.
    let input = "continuity:\n\n$$\\nabla \\cdot \\vec{v} = 0$$\n";
    let output = formatted_once(input);
    assert_eq!(
        output,
        "continuity:\n\n$$\n\\nabla \\cdot \\vec{v} = 0\n$$\n"
    );
    assert_eq!(formatted_once(&output), output);

    assert_eq!(formatted_once("> $$a = b$$\n"), "> $$\n> a = b\n> $$\n");
    assert_eq!(formatted_once("$$|x| = 1$$\n"), "$$\n|x| = 1\n$$\n");

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
fn a_one_line_equation_markdown_would_claim_stays_beside_its_delimiter() {
    // Alone on a line, `* x` is a list item and `# x` a heading.
    for input in ["$$* x$$\n", "$$# x$$\n", "$$> x$$\n"] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
}

#[test]
fn a_one_line_equation_stays_on_its_line_on_request() {
    let preferences = FormatOptions {
        one_line_math: OneLineMath::Keep,
        ..FormatOptions::default()
    };
    let short = "$$a = b$$\n";
    assert_eq!(
        format_document_with_options(short, preferences).unwrap(),
        short
    );
}

#[test]
fn a_block_is_a_paragraph_of_its_own() {
    // GitHub leaves a `$$` block with prose directly above or below it as
    // TeX source, so blank lines keep it apart, and a hard line break after
    // it, which no longer continues a paragraph, goes.
    assert_eq!(
        formatted_once("It is written as:\n$$u = g$$  \nwhere $u$ is a solution.\n"),
        "It is written as:\n\n$$\nu = g\n$$\n\nwhere $u$ is a solution.\n"
    );
    assert_eq!(
        formatted_once("> q:\n> $$a = b$$\n> more\n"),
        "> q:\n>\n> $$\n> a = b\n> $$\n>\n> more\n"
    );
    assert_eq!(
        formatted_once("$$a = b$$\n$$c = d$$\n"),
        "$$\na = b\n$$\n\n$$\nc = d\n$$\n"
    );

    // An equation kept on its one line still carries its hard break.
    let preferences = FormatOptions {
        one_line_math: OneLineMath::Keep,
        ..FormatOptions::default()
    };
    let input = "It is written as:\n\n$$u = g$$  \nwhere $u$ is a solution.\n";
    assert_eq!(
        format_document_with_options(input, preferences).unwrap(),
        input
    );

    // Before a blank line, or at the end, it is only trailing whitespace.
    for input in ["$$u = g$$  \n\nwhere\n", "$$\nu = g\n$$  \n"] {
        let output = formatted_once(input);
        assert!(!output.contains("$$  "), "actual: {output:?}");
    }
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
        "Code `\\(x\\)` span and $`\\(q\\)`$ math.\n",
        "Escaped \\\\(not math\\\\) here.\n",
        "```latex\n\\(\\alpha\\)\n```\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
    // GitHub would read `$\(q\)$` as `$(q)$`, hence the backticks.
    assert_eq!(
        formatted_once("Code `\\(x\\)` span and $\\(q\\)$ math.\n"),
        "Code `\\(x\\)` span and $`\\(q\\)`$ math.\n"
    );
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
    let expected = "$$\n\\begin{bmatrix}\nQ&A^\\top\\\\\nA&0\n\\end{bmatrix}\n\\begin{bmatrix}\nx\\\\\n\\nu\n\\end{bmatrix} =\n\\begin{bmatrix}\n-c\\\\\nb\n\\end{bmatrix}\n$$\n";
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

#[test]
fn a_sentence_period_after_an_equation_is_dropped() {
    assert_eq!(formatted_once("$$\nE = mc^2.\n$$\n"), "$$\nE = mc^2\n$$\n");
    assert_eq!(formatted_once("$$a = b,$$\n"), "$$\na = b\n$$\n");
    assert_eq!(
        formatted_once("$$\n\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}.\n$$\n"),
        "$$\n\\begin{aligned}\n  a &= b \\\\\n  c &= d\n\\end{aligned}\n$$\n"
    );
}

/// `\,` is a thin space, `\right.` an invisible delimiter, and `...` an
/// ellipsis: none of them is a sentence ending.
#[test]
fn punctuation_that_is_mathematics_is_kept() {
    for input in [
        "$$\n1 + 2 + ...\n$$\n",
        "$$\n\\text{ends in a period.}\n$$\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
    // GitHub would read the `\,` as an escaped comma, hence the fence.
    assert_eq!(formatted_once("$$\nx \\,\n$$\n"), "```math\nx \\,\n```\n");
    // The reflow closes up `\left(` as it always does; the point here is the
    // `.`, which `\right` needs and which must survive.
    assert_eq!(
        formatted_once("$$\\left( a \\right.$$\n"),
        "$$\n\\left(a \\right.\n$$\n"
    );
}

#[test]
fn a_leading_equals_line_does_not_make_a_heading_of_the_delimiter() {
    // `$$` over a lone `=` is a setext heading holding the opening delimiter.
    let input = "$$\n=\n\\begin{pmatrix}\na\n\\end{pmatrix}\n$$\n";
    assert_eq!(
        formatted_once(input),
        "$$\n= \\begin{pmatrix}\na\n\\end{pmatrix}\n$$\n"
    );
    // With nothing below to take it, a claimed line stays beside the opener.
    assert_eq!(formatted_once("$$\n* x\n$$\n"), "$$* x$$\n");
}

/// Inside a list item or a `<details>` block GitHub reads `$$` as inline
/// math and parses the TeX as prose first; only a `math` fence renders there.
#[test]
fn equations_in_lists_and_details_become_math_fences() {
    assert_eq!(
        formatted_once("1. x\n\n   $$\n   a_i\n   =\n   b_j + c\n   $$\n"),
        "1. x\n\n   ```math\n   a_i = b_j + c\n   ```\n"
    );
    assert_eq!(
        formatted_once(
            "<details>\n<summary>A</summary>\n\n$$\nx = y\n$$\n\n</details>\n\n$$a = b$$\n"
        ),
        "<details>\n<summary>A</summary>\n\n```math\nx = y\n```\n\n</details>\n\n$$\na = b\n$$\n"
    );
    // A block quote is not a list item: the `$$` block renders there.
    assert_eq!(formatted_once("> $$a = b$$\n"), "> $$\n> a = b\n> $$\n");
    // `\[...\]` is the author's choice of delimiters, and is left as such.
    assert_eq!(
        formatted_once("- x\n\n  \\[a = b\\]\n"),
        "- x\n\n  \\[\n  a = b\n  \\]\n"
    );

    let preferences = FormatOptions {
        math_fences: MathFences::Never,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_document_with_options("- x\n\n  $$a = b$$\n", preferences).unwrap(),
        "- x\n\n  $$\n  a = b\n  $$\n"
    );
}

/// GitHub reads the TeX of a `$$` block as Markdown before rendering it, so
/// `\,` loses its backslash and `T^*`, `p^*` pair up as emphasis that comes
/// back as `T^_`, `p^_`. A fence is never read as Markdown.
#[test]
fn display_tex_github_would_read_as_markdown_becomes_a_math_fence() {
    assert_eq!(
        formatted_once("$$\n\\frac{T^*}{T_0} = 0.8333, \\qquad \\frac{p^*}{p_0} = 0.5283\n$$\n"),
        "```math\n\\frac{T^*}{T_0} = 0.8333,\n\\qquad \\frac{p^*}{p_0} = 0.5283\n```\n"
    );
    assert_eq!(
        formatted_once("> $$\n> \\int f \\, dx\n> $$\n"),
        "> ```math\n> \\int f \\, dx\n> ```\n"
    );
    // Row breaks ending their lines, `_` subscripts and a single `*` reach
    // GitHub's renderer unchanged; such a block keeps its `$$`.
    for input in [
        "$$\n\\begin{aligned}\n  a_i &= b \\\\\n  c^* &= d_{x}\n\\end{aligned}\n$$\n",
        "$$\n\\left(a\\right)_{x} + \\left(b\\right)_{y} < 1\n$$\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
    let preferences = FormatOptions {
        math_fences: MathFences::Never,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_document_with_options("$$\nx \\, y\n$$\n", preferences).unwrap(),
        "$$\nx \\, y\n$$\n"
    );
}

/// GitHub finds inline math in the text Markdown leaves, so a span must come
/// through Markdown unchanged and follow a space or `(`. Those that would not
/// are written `$`...`$`, which Markdown leaves alone; the rest are kept.
#[test]
fn inline_math_github_would_not_render_gets_backticks() {
    for (input, expected) in [
        // An escape loses its backslash.
        ("Thin $a \\, b$ space.\n", "Thin $`a \\, b`$ space.\n"),
        // The `_` after `}` and the `_` before `{` pair as emphasis.
        (
            "Both $\\mathbf{u}_i$ and $u_{j}$ break.\n",
            "Both $`\\mathbf{u}_i`$ and $`u_{j}`$ break.\n",
        ),
        // As do two `*`, across spans.
        (
            "Starred $T^*$ and $p^*$.\n",
            "Starred $`T^*`$ and $`p^*`$.\n",
        ),
        // A `$` after a hyphen or a letter does not open; one before a
        // letter does not close.
        (
            "A moderate-$Re$ flow, $k$s and x$y$.\n",
            "A moderate-$`Re`$ flow, $`k`$s and x$`y`$.\n",
        ),
        // Nor does a `$` between two parentheses close.
        (
            "Variables ($p(x, t)$) and $p(x)$.\n",
            "Variables ($`p(x, t)`$) and $p(x)$.\n",
        ),
        // GitHub finds none inside emphasis or link text; strong is fine.
        (
            "*Figure: $f(x)$ and **$g$**.* **Bold $h$.** [The $k$ link](x.md)\n",
            "*Figure: $`f(x)`$ and **$`g`$**.* **Bold $h$.** [The $`k`$ link](x.md)\n",
        ),
        // In a heading and a table cell alike.
        (
            "## The $k$-$\\epsilon$ model\n",
            "## The $k$-$`\\epsilon`$ model\n",
        ),
    ] {
        assert_eq!(formatted_once(input), expected, "{input:?}");
    }
    for input in [
        "Plain $u_i$, $v_i$ and $a_{i}^{n} - b_{i}^{n}$ ($x$) render.\n",
        "**$q$** and $\\rho^*$ and $x < y$; $|u|$.\n",
        "| $a_1$ | $\\frac{b}{c}$ |\n| --- | --- |\n| x | y |\n",
        "Already $`a \\, b`$ safe.\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
    let preferences = FormatOptions {
        math_fences: MathFences::Never,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_document_with_options("Thin $a \\, b$.\n", preferences).unwrap(),
        "Thin $a \\, b$.\n"
    );
}

/// The Markdown serializer opens a block quote that starts with a code block
/// with blank `>` lines, two more on every run.
#[test]
fn a_block_quote_opening_with_code_gains_no_blank_lines() {
    for input in [
        "> ```python\n> x = 1\n> ```\n",
        "Text\n\n> ```python\n> x = 1\n> ```\n",
        "> > ```python\n> > x = 1\n> > ```\n",
    ] {
        assert_eq!(formatted_once(input), input, "{input:?}");
    }
}

/// A line of an equation must not end in a control space: the trailing space
/// goes, and the backslash left before the line break is a Markdown hard line
/// break, which cuts the equation in two. One already broken that way is
/// repaired.
#[test]
fn a_control_space_never_ends_a_wrapped_line() {
    let expected = "$$\ns_2 - s_1 = 1004.5 \\ln \\frac{600}{288.15} - 287 \\ln 10 = 736.7 -\n660.8 = 75.9\\ \\text{J/(kg K)}\n$$\n";
    for input in [
        "$$\ns_2 - s_1 = 1004.5 \\ln \\frac{600}{288.15} - 287 \\ln 10 = 736.7 - 660.8 = 75.9\\ \\text{J/(kg K)}\n$$\n",
        "$$\ns_2 - s_1 = 1004.5 \\ln \\frac{600}{288.15} - 287 \\ln 10 = 736.7 - 660.8 = 75.9\\\n\\text{J/(kg K)}\n$$\n",
    ] {
        assert_eq!(formatted_once(input), expected, "{input:?}");
    }
}

/// A sign that is itself a superscript or subscript is not spaced as an
/// operator: `y^+` must not come back as `y^ +`.
#[test]
fn a_scripted_sign_stays_with_its_script() {
    let input = "$$\n\\begin{aligned}\n  y^+ &= \\frac{y u_\\tau}{\\nu} \\\\\n  u_- &= c^{-} \\approx 1\n\\end{aligned}\n$$\n";
    assert_eq!(formatted_once(input), input);
    assert_eq!(
        formatted_once("$$\ny^+ \\approx 30\n$$\n"),
        "$$\ny^+ \\approx 30\n$$\n"
    );
}
