//! Conservative, token-aware layout of ordinary display math.
//!
//! TeX environments, comments, alignment markers, and explicit line breaks are
//! left unchanged: those need an environment-aware parser, not a whitespace pass.

use crate::MathStyle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Command,
    Group,
    Operator,
    Atom,
    Punctuation,
}

struct Token<'a> {
    text: &'a str,
    kind: Kind,
    spaced: bool,
}

/// Reflow a plain display equation; never interpret existing source newlines as
/// mathematical line breaks (unlike explicit TeX `\\`).
pub(crate) fn format_display_math(input: &str, style: MathStyle, width: usize) -> String {
    if input.trim().is_empty() || !safe_to_reflow(input) {
        return input.to_owned();
    }

    // Newlines between TeX tokens are insignificant in this subset. Joining
    // with a space is essential: joining `\alpha` and `u` without one would
    // produce a different control sequence (`\alphau`).
    let joined = input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = tokenize(&joined);
    if tokens.is_empty() || ambiguous_operators(&tokens) {
        return input.to_owned();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    if style == MathStyle::Readable {
        for index in 1..tokens.len().saturating_sub(1) {
            // Treat \qquad as a semantic separation in the readable style,
            // but keep it attached to the expression that follows it.
            if tokens[index].text == "\\qquad"
                && tokens[index - 1].text != "\\qquad"
                && tokens[index + 1].text != "\\qquad"
                && tokens[index + 1].text != "\\quad"
            {
                lines.extend(wrap(&tokens[start..index], width));
                start = index;
            }
        }
    }
    lines.extend(wrap(&tokens[start..], width));
    lines.join("\n")
}

fn safe_to_reflow(input: &str) -> bool {
    // These constructs carry line/layout or lexical semantics that a basic
    // tokenizer cannot safely preserve while reflowing.
    if input.contains("\\\\")
        || input.contains('&')
        || input.contains('%')
        || input.contains("\\begin")
        || input.contains("\\end")
        || input.contains("\\verb")
        || input.contains("\\def")
        || input.contains("\\newcommand")
        || input.contains("\\tag")
        || input.contains("\\label")
        || input.contains("\\intertext")
        || input.contains('\r')
    {
        return false;
    }

    let mut depth = 0usize;
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // An escaped brace is not a grouping delimiter.
            if matches!(chars.clone().next(), Some('{') | Some('}')) {
                chars.next();
            }
        } else if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            if depth == 0 {
                return false;
            }
            depth -= 1;
        }
    }
    depth == 0
}

fn tokenize(input: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut position = 0;
    let mut spaced = false;

    while position < input.len() {
        let ch = input[position..].chars().next().expect("valid character");
        if ch.is_whitespace() {
            position += ch.len_utf8();
            spaced = true;
            continue;
        }
        let start = position;
        position += ch.len_utf8();
        let kind = match ch {
            '\\' => {
                // TeX control words consume letters; control symbols consume
                // exactly one character. Groups remain atomic and opaque.
                if let Some(next) = input[position..].chars().next() {
                    if next.is_ascii_alphabetic() {
                        while let Some(letter) = input[position..].chars().next() {
                            if !letter.is_ascii_alphabetic() {
                                break;
                            }
                            position += letter.len_utf8();
                        }
                    } else {
                        position += next.len_utf8();
                    }
                }
                Kind::Command
            }
            '{' => {
                let mut depth = 1;
                while depth > 0 && position < input.len() {
                    let next = input[position..].chars().next().expect("valid character");
                    position += next.len_utf8();
                    if next == '\\' {
                        if let Some(escaped) = input[position..].chars().next() {
                            if escaped == '{' || escaped == '}' {
                                position += escaped.len_utf8();
                            }
                        }
                    } else if next == '{' {
                        depth += 1;
                    } else if next == '}' {
                        depth -= 1;
                    }
                }
                Kind::Group
            }
            '+' | '-' | '=' => Kind::Operator,
            letter if letter.is_alphanumeric() => {
                while let Some(next) = input[position..].chars().next() {
                    if !next.is_alphanumeric() {
                        break;
                    }
                    position += next.len_utf8();
                }
                Kind::Atom
            }
            _ => Kind::Punctuation,
        };
        tokens.push(Token {
            text: &input[start..position],
            kind,
            spaced,
        });
        spaced = false;
    }
    tokens
}

fn unary(tokens: &[Token<'_>], index: usize) -> bool {
    matches!(tokens[index].text, "+" | "-")
        && (index == 0
            || tokens[index - 1].kind == Kind::Operator
            || matches!(tokens[index - 1].text, "(" | "["))
}

fn ambiguous_operators(tokens: &[Token<'_>]) -> bool {
    tokens.windows(2).enumerate().any(|(index, pair)| {
        pair[0].kind == Kind::Operator
            && pair[1].kind == Kind::Operator
            && !(matches!(pair[1].text, "+" | "-") && unary(tokens, index + 1))
    })
}

fn separator(tokens: &[Token<'_>], index: usize) -> &'static str {
    if index == 0 {
        return "";
    }
    let previous = &tokens[index - 1];
    let current = &tokens[index];

    if current.kind == Kind::Operator {
        return if unary(tokens, index) && matches!(previous.text, "(" | "[") {
            ""
        } else {
            " "
        };
    }
    if previous.kind == Kind::Operator {
        return if unary(tokens, index - 1) { "" } else { " " };
    }
    if matches!(
        current.text,
        "." | "," | ";" | ":" | "!" | "?" | ")" | "]" | "^" | "_" | "'"
    ) || matches!(previous.text, "(" | "[" | "^" | "_")
    {
        return "";
    }
    if current.kind == Kind::Group && previous.kind == Kind::Command {
        return "";
    }
    if matches!(
        current.text,
        "\\frac" | "\\sqrt" | "\\text" | "\\operatorname"
    ) {
        return " ";
    }
    // Required by TeX: `\alpha u` cannot become `\alphau`.
    if previous.kind == Kind::Command
        && current
            .text
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        return " ";
    }
    if current.spaced { " " } else { "" }
}

fn render(tokens: &[Token<'_>], start: usize, end: usize) -> String {
    let mut output = String::new();
    for index in start..end {
        if index > start {
            output.push_str(separator(tokens, index));
        }
        output.push_str(tokens[index].text);
    }
    output
}

fn can_break_before(tokens: &[Token<'_>], index: usize) -> bool {
    if index == 0 || tokens[index - 1].kind == Kind::Operator {
        return false;
    }
    // If a sum must wrap, keep the + or - with the NEXT term: no dangling
    // operator at line end, and never start a line with an equality sign.
    if matches!(tokens[index].text, "+" | "-") {
        return index + 1 < tokens.len() && !unary(tokens, index);
    }
    tokens[index].kind != Kind::Operator
        && tokens[index].kind != Kind::Group
        && separator(tokens, index) == " "
}

fn wrap(tokens: &[Token<'_>], width: usize) -> Vec<String> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    'outer: while start < tokens.len() {
        let mut last_break = None;
        for end in start + 1..=tokens.len() {
            if end - 1 > start
                && can_break_before(tokens, end - 1)
                && render(tokens, start, end - 1).chars().count() <= width
            {
                last_break = Some(end - 1);
            }
            if render(tokens, start, end).chars().count() > width {
                if let Some(split) = last_break {
                    lines.push(render(tokens, start, split));
                    start = split;
                    continue 'outer;
                }
            }
        }
        lines.push(render(tokens, start, tokens.len()));
        break;
    }
    lines
}
