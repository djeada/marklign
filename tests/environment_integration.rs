use marklign::format_document;

#[test]
fn aligned_rows_survive_markdown_serialization() {
    let input = r#"# Equations

$$
\begin{aligned}
a
&
=
b
+
c \\
d &= e
\end{aligned}
$$
"#;
    let output = format_document(input).unwrap();
    let expected = r"\begin{aligned}
  a &= b + c \\
  d &= e
\end{aligned}";
    assert!(output.contains(expected), "actual Markdown: {output:?}");
    assert_eq!(output, format_document(&output).unwrap());
}

#[test]
fn cases_survive_markdown_serialization() {
    let input = r#"$$
\begin{cases}
x^2 & x>0 \\
0 & \text{otherwise}
\end{cases}
$$
"#;
    let output = format_document(input).unwrap();
    assert!(output.contains("x^2 & x>0"), "actual Markdown: {output:?}");
    assert!(output.contains(r"\text{otherwise}"));
    assert_eq!(output, format_document(&output).unwrap());
}

#[test]
fn fenced_code_is_unchanged_but_math_after_the_fence_is_formatted() {
    for (open, close) in [("```latex", "```"), ("~~~latex", "~~~")] {
        let input = format!(
            "{open}\n$$\n\\begin{{aligned}}\na\n&\n=\nb\n\\end{{aligned}}\n$$\n{close}\n\n$$\n\\begin{{aligned}}\na\n&\n=\nb\n\\end{{aligned}}\n$$\n"
        );
        let output = format_document(&input).unwrap();
        assert!(output.contains("\\begin{aligned}\na\n&\n=\nb\n\\end{aligned}"));
        assert!(output.contains("\\begin{aligned}\n  a &= b\n\\end{aligned}"));
        assert_eq!(output, format_document(&output).unwrap());
    }
}
