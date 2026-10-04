//! Mathematics GitHub would read as Markdown.
//!
//! GitHub does not hand `$` and `$$` mathematics to its Markdown parser as
//! mathematics. It renders the document as Markdown first and looks for the
//! delimiters in the text that comes out, so the TeX between them has
//! already been read as prose: `\,` and `\{` lose their backslashes as
//! escapes, `\\` inside a line becomes `\`, two `*` pair up as emphasis, and
//! an inline span that emphasis cuts in two, or that does not follow a space,
//! is not found at all. A ```` ```math ```` fence and a `` $`...`$ `` span
//! are code to the Markdown parser, and reach the renderer as written.

use comrak::{
    Arena, Options,
    nodes::{AstNode, NodeValue},
    parse_document,
};

/// Whether GitHub would change the TeX of a `$$` block before rendering it.
///
/// The block is a paragraph of its own, so a delimiter Markdown pairs, such
/// as `*`, must occur twice to do any harm. Emphasis made of `_` is the one
/// case GitHub repairs, writing it back as `_`; `*` emphasis comes back as
/// `_` too, which is why `T^*` and `p^*` turn into `T^_` and `p^_`. A `\\`
/// row break at the end of its line is the one escape GitHub leaves alone.
pub(crate) fn display_reads_as_markdown(body: &str) -> bool {
    let bytes = body.as_bytes();
    let count = |byte: u8| bytes.iter().filter(|&&each| each == byte).count();
    if count(b'*') >= 2 || count(b'`') >= 2 || count(b'~') >= 2 || body.contains("](") {
        return true;
    }

    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                let rest = &bytes[index + 1..];
                if rest.first() == Some(&b'\\') && ends_line(&rest[1..]) {
                    index += 2;
                    continue;
                }
                // A hard line break once trailing spaces go, or an escape.
                if ends_line(rest) || rest[0].is_ascii_punctuation() {
                    return true;
                }
                index += 2;
            }
            b'<' if opens_html(&bytes[index + 1..]) => return true,
            b'&' if is_entity(&bytes[index + 1..]) => return true,
            b'$' => return true,
            _ => index += 1,
        }
    }
    false
}

/// Nothing but spaces before the end of the line or of the equation.
fn ends_line(rest: &[u8]) -> bool {
    rest.iter()
        .find(|&&byte| byte != b' ' && byte != b'\t')
        .is_none_or(|&byte| byte == b'\n')
}

/// A `<` Markdown could read as the start of a tag, a comment, or an autolink:
/// one followed by a letter, `/`, `!`, or `?`, with a `>` somewhere after it.
fn opens_html(rest: &[u8]) -> bool {
    rest.first()
        .is_some_and(|&byte| byte.is_ascii_alphabetic() || matches!(byte, b'/' | b'!' | b'?'))
        && rest.contains(&b'>')
}

/// An entity or numeric character reference such as `&lt;` or `&#42;` after
/// an `&`. Markdown decodes them; an alignment `&` is not one.
fn is_entity(rest: &[u8]) -> bool {
    let (digits, body): (fn(&u8) -> bool, &[u8]) = match rest {
        [b'#', b'x' | b'X', body @ ..] => (u8::is_ascii_hexdigit, body),
        [b'#', body @ ..] => (u8::is_ascii_digit, body),
        [first, ..] if first.is_ascii_alphabetic() => (u8::is_ascii_alphanumeric, rest),
        _ => return false,
    };
    let length = body.iter().take_while(|byte| digits(byte)).count();
    length > 0 && body.get(length) == Some(&b';')
}

/// Write each inline `$...$` span GitHub would not render as written as a
/// `` $`...`$ `` span instead; returns whether any was rewritten.
///
/// Whether GitHub finds a span depends on the paragraph around it, so the
/// question is put to a Markdown parser as GitHub puts it: `markdown` is
/// parsed again without mathematics, and each span of `root` must come out
/// of that parse character for character, within one run of text, after a
/// space or `(`, and before something that is not a letter, digit, or `_`
/// (nor a `)` when the TeX ends in one).
/// A run ends wherever emphasis, code, a link, or raw HTML begins or ends,
/// and the text of `*emphasis*` and of a link holds no mathematics at all.
///
/// Rewriting a span takes its `*` and `_` out of the paragraph, which can
/// pair the delimiters left behind differently, so the caller repeats the
/// check on its output until nothing changes.
pub(crate) fn protect_inline_math<'a>(
    root: &'a AstNode<'a>,
    markdown: &str,
    options: &Options<'_>,
) -> bool {
    let mut prose = options.clone();
    prose.extension.math_dollars = false;
    prose.extension.math_code = false;
    let arena = Arena::new();
    let plain = parse_document(&arena, markdown, &prose);

    let parsed = inline_containers(root);
    let read = inline_containers(plain);
    // Mathematics is inline content, so both parses must agree on the blocks.
    // Should they not, nothing is rewritten on a guess.
    if parsed.len() != read.len() {
        return false;
    }

    let mut changed = false;
    for (container, prose) in parsed.into_iter().zip(read) {
        let spans: Vec<_> = container
            .descendants()
            .filter(|node| {
                matches!(
                    node.data.borrow().value,
                    NodeValue::Math(ref math) if math.dollar_math && !math.display_math
                )
            })
            .collect();
        if spans.is_empty() {
            continue;
        }

        let mut runs = vec![String::new()];
        text_runs(prose, &mut runs);
        let mut cursor = (0, 0);
        for span in spans {
            let mut data = span.data_mut();
            let NodeValue::Math(ref mut math) = data.value else {
                continue;
            };
            let written = format!("${}$", math.literal);
            match find_rendered(&runs, cursor, &written) {
                Some(found) => cursor = found,
                // A backtick inside would end the code span early.
                None if !math.literal.contains('`') => {
                    math.dollar_math = false;
                    changed = true;
                }
                None => {}
            }
        }
    }
    changed
}

/// The blocks that hold inline content, in document order.
fn inline_containers<'a>(root: &'a AstNode<'a>) -> Vec<&'a AstNode<'a>> {
    root.descendants()
        .filter(|node| {
            matches!(
                node.data.borrow().value,
                NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::TableCell
            )
        })
        .collect()
}

/// Split a block's inline content into the runs of text GitHub searches for
/// delimiters: text and soft line breaks run on; any other inline ends a run
/// where it begins and again where it ends. GitHub looks for none inside
/// `*emphasis*` or the text of a link, so what those hold is left out.
fn text_runs<'a>(node: &'a AstNode<'a>, runs: &mut Vec<String>) {
    for child in node.children() {
        let data = child.data.borrow();
        match data.value {
            NodeValue::Text(ref text) => runs.last_mut().expect("a run").push_str(text),
            NodeValue::SoftBreak => runs.last_mut().expect("a run").push('\n'),
            NodeValue::Emph | NodeValue::Link(_) | NodeValue::Image(_) => {
                runs.push(String::new());
            }
            _ => {
                drop(data);
                runs.push(String::new());
                text_runs(child, runs);
                runs.push(String::new());
            }
        }
    }
}

/// Find `written` in the runs at or after `cursor` where GitHub would render
/// it, returning the position just past it.
fn find_rendered(
    runs: &[String],
    (start_run, start_at): (usize, usize),
    written: &str,
) -> Option<(usize, usize)> {
    for (number, run) in runs.iter().enumerate().skip(start_run) {
        let mut from = if number == start_run { start_at } else { 0 };
        while let Some(offset) = run.get(from..).and_then(|rest| rest.find(written)) {
            let at = from + offset;
            let end = at + written.len();
            let before = run[..at].chars().next_back();
            let after = run[end..].chars().next();
            let opens = before.is_none_or(|ch| ch.is_ascii_whitespace() || ch == '(');
            // Nor does GitHub close one on `$` between two parentheses, as in
            // `($p(x)$)`.
            let closes = after.is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_'))
                && !(written.ends_with(")$") && after == Some(')'));
            if opens && closes {
                return Some((number, end));
            }
            from = at + 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::display_reads_as_markdown;

    #[test]
    fn display_tex_github_would_read_as_markdown() {
        for body in [
            r"\int f \, dx",
            r"\{ x \}",
            r"a \; b \! c",
            r"\|u\|",
            r"50 \%",
            r"\frac{T^*}{T_0} = \frac{p^*}{p_0}",
            r"\begin{bmatrix} a \\ b \end{bmatrix}",
            r"a \\[4pt] b",
            "a \\\nb",
            "a \\  \nb",
            r"x <b> y",
            r"[0, 1](x)",
            r"a~~b~~c",
            r"a &lt; b",
            r"a &#42; b",
            r"\text{\$5}",
        ] {
            assert!(display_reads_as_markdown(body), "{body:?}");
        }
    }

    #[test]
    fn display_tex_github_leaves_alone() {
        for body in [
            r"\frac{T_0}{T} = 1 + \frac{\gamma - 1}{2} M^2",
            r"\rho^* = 0.6339",
            "\\begin{aligned}\n  a &= b \\\\\n  c &= d\n\\end{aligned}",
            "a \\\\  \nb",
            r"\left(a\right)_{x} + \left(b\right)_{y} \quad \sum_{i} x_{i}",
            r"0 < x < 1, \quad x > 0",
            r"a \cdot b \ c",
        ] {
            assert!(!display_reads_as_markdown(body), "{body:?}");
        }
    }
}
