//! Hold display-math blocks aside during Markdown block parsing.
//!
//! Math delimiters are an inline construct: Comrak's block parser reaches a
//! line containing only `=` or `-` inside an equation first and reads it as a
//! setext heading underline, which silently destroys the mathematics. Every
//! equation that owns its source lines is therefore formatted and replaced by
//! an opaque placeholder before parsing, then restored afterwards. Blocks
//! inside list items and block quotes are held aside too, and put back with
//! whatever prefix their container ended up with.

use comrak::{Arena, Options, nodes::NodeValue, parse_document};

use crate::{FormatOptions, environments, math};

/// A document whose standalone display-math blocks have been set aside.
pub(crate) struct BlockMath {
    /// Markdown to parse, with one placeholder line per extracted block.
    pub(crate) text: String,
    blocks: Vec<Held>,
    token: String,
}

/// A formatted equation waiting to be put back, with the delimiters the
/// author chose: Marklign reflows mathematics but never restyles delimiters.
struct Held {
    opener: &'static str,
    closer: &'static str,
    body: String,
    /// The source kept the delimiters on the equation's own line, which is
    /// honored as long as the formatted equation still fits on one line.
    one_line: bool,
    /// The closing delimiter carried a trailing hard line break.
    hard_break: bool,
}

/// Recognized display-math delimiter pairs.
const DELIMITERS: [(&str, &str); 2] = [("$$", "$$"), (r"\[", r"\]")];

/// Placeholder stems: letters only, so Markdown never escapes or reflows one.
/// An inline placeholder brackets its index with the stem on both sides; a
/// bare suffix would run into a digit of the prose that followed the span.
const TOKEN: &str = "MARKLIGNMATHBLOCK";
const INLINE_TOKEN: &str = "MARKLIGNMATHINLINE";

pub(crate) fn extract(input: &str, preferences: FormatOptions, options: &Options<'_>) -> BlockMath {
    let mut token = TOKEN.to_owned();
    while input.contains(&token) {
        token.push('X');
    }

    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    let verbatim = verbatim_lines(input, options);
    let mut text = String::with_capacity(input.len());
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if verbatim[index].is_none() {
            if let Some((prefix, held, end)) = block_at(&lines, index, &verbatim, preferences) {
                text.push_str(prefix);
                text.push_str(&token);
                text.push_str(&blocks.len().to_string());
                text.push('\n');
                blocks.push(held);
                index = end + 1;
                continue;
            }
        }

        text.push_str(lines[index]);
        index += 1;
    }

    BlockMath {
        text,
        blocks,
        token,
    }
}

impl BlockMath {
    /// Put the formatted equations back, or `None` if a placeholder did not
    /// survive rendering; the caller then formats without extraction rather
    /// than emitting a document with an equation missing.
    pub(crate) fn restore(&self, rendered: &str) -> Option<String> {
        if self.blocks.is_empty() {
            return Some(rendered.to_owned());
        }
        let mut output = String::with_capacity(rendered.len());
        let mut restored = vec![false; self.blocks.len()];

        for line in rendered.split_inclusive('\n') {
            match self.placeholder(strip_newline(line)) {
                Some((prefix, position)) => {
                    let held = &self.blocks[position];
                    if held.one_line && !held.body.contains('\n') {
                        output.push_str(prefix);
                        output.push_str(held.opener);
                        output.push_str(&held.body);
                        output.push_str(held.closer);
                    } else {
                        // A list marker belongs to the first line only;
                        // indentation and quote markers repeat below it.
                        let continuation = continuation_prefix(prefix);
                        for (offset, body_line) in std::iter::once(held.opener)
                            .chain(held.body.lines())
                            .enumerate()
                        {
                            let prefix = if offset == 0 { prefix } else { &continuation };
                            output.push_str(format!("{prefix}{body_line}").trim_end());
                            output.push('\n');
                        }
                        output.push_str(&continuation);
                        output.push_str(held.closer);
                    }
                    if held.hard_break {
                        output.push_str("  ");
                    }
                    output.push('\n');
                    restored[position] = true;
                }
                None => output.push_str(line),
            }
        }

        restored.iter().all(|&done| done).then_some(output)
    }

    /// Match a line holding nothing but a placeholder, returning the container
    /// prefix (indentation, block-quote or list markers) Comrak settled on.
    fn placeholder<'a>(&self, line: &'a str) -> Option<(&'a str, usize)> {
        let start = line.find(&self.token)?;
        let (prefix, rest) = line.split_at(start);
        if !prefix.chars().all(is_container_char) {
            return None;
        }
        let position: usize = rest[self.token.len()..].trim_end().parse().ok()?;
        (position < self.blocks.len()).then_some((prefix, position))
    }
}

/// A document whose inline `\(...\)` math spans have been set aside.
pub(crate) struct InlineMath {
    /// Markdown to parse, with one placeholder per extracted span.
    pub(crate) text: String,
    spans: Vec<String>,
    token: String,
}

/// Hold `\(...\)` inline math aside, to be put back character for character.
///
/// Comrak reads `\(` as an escaped parenthesis: the delimiters disappear and
/// the TeX between them is escaped as prose, so `\(\alpha\)` comes back as
/// `(\\alpha)`. Enabling its `math_latex` extension instead rewrites the span
/// as `$\alpha$`, restyling delimiters Marklign promises to keep, and reads
/// Comrak's own `\[` escaping of a literal bracket back as display math.
///
/// So the spans are simply preserved, whether or not the author meant
/// mathematics by them: either way the source said `\(`.
pub(crate) fn extract_inline(input: &str, options: &Options<'_>) -> InlineMath {
    let mut token = INLINE_TOKEN.to_owned();
    while input.contains(&token) {
        token.push('X');
    }

    let verbatim = verbatim_lines(input, options);
    let mut text = String::with_capacity(input.len());
    let mut spans = Vec::new();

    for (number, line) in input.split_inclusive('\n').enumerate() {
        if verbatim[number].is_some() {
            text.push_str(line);
        } else {
            hold_spans(line, &token, &mut spans, &mut text);
        }
    }

    InlineMath { text, spans, token }
}

impl InlineMath {
    /// Put the spans back, or `None` if a placeholder did not survive
    /// rendering; as with a held block, the caller then formats the document
    /// without extraction rather than emitting one with mathematics missing.
    pub(crate) fn restore(&self, rendered: &str) -> Option<String> {
        if self.spans.is_empty() {
            return Some(rendered.to_owned());
        }
        let mut output = String::with_capacity(rendered.len());
        let mut restored = vec![false; self.spans.len()];
        let mut rest = rendered;

        while let Some(start) = rest.find(&self.token) {
            let (before, tail) = rest.split_at(start);
            let tail = &tail[self.token.len()..];
            let end = tail.find(&self.token)?;
            let position: usize = tail[..end].parse().ok()?;
            output.push_str(before);
            output.push_str(self.spans.get(position)?);
            restored[position] = true;
            rest = &tail[end + self.token.len()..];
        }
        output.push_str(rest);

        restored.iter().all(|&done| done).then_some(output)
    }
}

/// Copy one line, replacing every `\(...\)` span with a placeholder. Inline
/// code and `$`-delimited math are stepped over: what they hold is theirs.
fn hold_spans(line: &str, token: &str, spans: &mut Vec<String>, text: &mut String) {
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if matches!(bytes[index], b'`' | b'$') {
            let end = delimited_end(line, index);
            text.push_str(&line[index..end]);
            index = end;
            continue;
        }
        if bytes[index] == b'\\' {
            // A backslash escape consumes the character after it, so `\\(` is
            // a literal backslash and a parenthesis rather than a delimiter.
            let opened = line[index + 1..].strip_prefix('(');
            if let Some(at) = opened.and_then(inline_close) {
                let end = index + 2 + at;
                text.push_str(&format!("{token}{}{token}", spans.len()));
                spans.push(line[index..end].to_owned());
                index = end;
                continue;
            }
            let escaped = line[index + 1..].chars().next().map_or(0, char::len_utf8);
            text.push_str(&line[index..index + 1 + escaped]);
            index += 1 + escaped;
            continue;
        }
        let character = line[index..].chars().next().expect("valid character");
        text.push(character);
        index += character.len_utf8();
    }
}

/// Offset just past the `\)` that closes a span whose contents begin `rest`,
/// or `None` when the line ends first: a span is never continued on the line
/// below, where a Markdown block may well have ended.
fn inline_close(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
        } else if bytes[index + 1] == b')' {
            return Some(index + 2);
        } else {
            index += 2;
        }
    }
    None
}

/// Offset just past the inline code span or `$`-delimited math opening at
/// `start`, or just past the delimiter run when nothing on the line closes it.
fn delimited_end(line: &str, start: usize) -> usize {
    let bytes = line.as_bytes();
    let delimiter = bytes[start];
    let mut open = start;
    while bytes.get(open) == Some(&delimiter) {
        open += 1;
    }
    // Comrak opens dollar math only on a delimiter the mathematics follows
    // immediately, and closes it only on one the mathematics runs up to.
    if delimiter == b'$' && bytes.get(open).is_none_or(u8::is_ascii_whitespace) {
        return open;
    }

    let width = open - start;
    let mut index = open;
    while index < bytes.len() {
        if bytes[index] != delimiter {
            index += 1;
            continue;
        }
        let mut close = index;
        while bytes.get(close) == Some(&delimiter) {
            close += 1;
        }
        // A backtick run closes a code span only when it is the same length.
        if close - index == width && !(delimiter == b'$' && bytes[index - 1].is_ascii_whitespace())
        {
            return close;
        }
        index = close;
    }
    open
}

/// Recognize a display equation that owns its source lines, starting at
/// `open`; returns its container prefix, formatted contents, and last line.
fn block_at<'a>(
    lines: &[&'a str],
    open: usize,
    verbatim: &[Option<Verbatim>],
    preferences: FormatOptions,
) -> Option<(&'a str, Held, usize)> {
    let (prefix, opener, closer, first) = opening(strip_newline(lines[open]))?;
    let depth = prefix.matches('>').count();

    // An equation closed on its opening line is already one block to the
    // Markdown parser. Hold it aside anyway: reflowing it may need to move
    // the delimiters onto their own lines, and doing that here keeps a
    // second run of the formatter from finding anything left to change.
    if let Some(at) = close_at(first, closer) {
        let body = format_block(&first[..at], preferences);
        return (!body.is_empty()).then_some((
            prefix,
            Held {
                opener,
                closer,
                body,
                one_line: true,
                hard_break: is_hard_break(&first[at + closer.len()..]),
            },
            open,
        ));
    }
    // The delimiters closed on this line but prose follows them, as in the
    // escaped brackets Comrak writes around `[TODO: ...]`. That is not an
    // equation, and the search below must not run on to some later `\]` and
    // swallow the paragraphs in between.
    if first.contains(closer) {
        return None;
    }

    let mut source = String::from(first);
    // A blank line inside an equation would end the Markdown block the
    // equation lives in, so the delimiters no longer bracket one block. The
    // equation is still held aside -- otherwise Comrak reads its `=` and `*`
    // lines as headings and list items -- but it is put back verbatim: a
    // blank line is a layout intent this formatter does not understand.
    let mut blank = false;
    for (offset, line) in lines[open + 1..].iter().enumerate() {
        source.push('\n');
        // A fenced block can interrupt a paragraph, and its contents are not
        // mathematics however much they look like it.
        if verbatim[open + 1 + offset].is_some() {
            return None;
        }
        // Leaving the container ends the equation. A blank line in a quote
        // must still carry its `>` markers to stay inside it.
        let content = strip_container(strip_newline(line), depth)?;
        if content.trim().is_empty() {
            blank = true;
            continue;
        }
        let Some(at) = close_at(content, closer) else {
            // A delimiter with prose after it is not a block to reflow.
            if content.contains(closer) {
                return None;
            }
            source.push_str(content);
            continue;
        };
        source.push_str(&content[..at]);
        let body = if blank {
            trim_block(&source)
        } else {
            format_block(&source, preferences)
        };
        return (!body.is_empty()).then_some((
            prefix,
            Held {
                opener,
                closer,
                body,
                one_line: false,
                hard_break: is_hard_break(&content[at + closer.len()..]),
            },
            open + 1 + offset,
        ));
    }
    None
}

/// Two trailing spaces after the closing delimiter are a Markdown hard line
/// break, which is content rather than stray whitespace. A tab is not.
fn is_hard_break(trailing: &str) -> bool {
    trailing.chars().filter(|&ch| ch == ' ').count() >= 2
}

/// Offset of a closing delimiter that ends its line, or `None`.
fn close_at(content: &str, closer: &str) -> Option<usize> {
    let at = content.find(closer)?;
    let rest = &content[at + closer.len()..];
    (rest.trim().is_empty()).then_some(at)
}

/// Split an opening delimiter line into its container prefix, the delimiter
/// pair, and whatever mathematics follows on the same line.
fn opening(line: &str) -> Option<(&str, &'static str, &'static str, &str)> {
    let content = line.trim_start_matches([' ', '\t', '>']);
    let prefix = &line[..line.len() - content.len()];
    DELIMITERS
        .iter()
        .find_map(|&(opener, closer)| Some((prefix, opener, closer, content.strip_prefix(opener)?)))
}

/// Drop the whitespace an equation picked up from its delimiter lines, and
/// nothing else. A `$$ ` opener must not leave a blank first line in an
/// equation kept verbatim, which on a later run would read as a blank line
/// inside the block.
fn trim_block(source: &str) -> String {
    let lines: Vec<&str> = source.lines().map(str::trim_end).collect();
    let Some(first) = lines.iter().position(|line| !line.is_empty()) else {
        return String::new();
    };
    let last = lines
        .iter()
        .rposition(|line| !line.is_empty())
        .unwrap_or(first);
    let mut body = lines[first..=last].to_vec();
    body[0] = body[0].trim_start();
    body.join("\n")
}

/// Apply the same layout rules the Markdown pass applies to inline math.
fn format_block(source: &str, preferences: FormatOptions) -> String {
    let source = trim_block(source);
    if source.is_empty() {
        return source;
    }
    environments::format_environment(&source)
        .unwrap_or_else(|| {
            math::format_display_math(&source, preferences.math_style, preferences.math_width)
        })
        .trim_matches('\n')
        .to_owned()
}

/// Remove `depth` block-quote markers and any indentation, or `None` if the
/// line does not continue that container. A further `>` opens a quote, which
/// would end the block the equation lives in.
fn strip_container(line: &str, depth: usize) -> Option<&str> {
    let mut rest = line;
    for _ in 0..depth {
        rest = rest.trim_start_matches([' ', '\t']).strip_prefix('>')?;
    }
    let content = rest.trim_start_matches([' ', '\t']);
    (!content.starts_with('>')).then_some(content)
}

fn continuation_prefix(prefix: &str) -> String {
    prefix
        .chars()
        .map(|ch| {
            if ch == '>' || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

fn is_container_char(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '>' | '-' | '*' | '+' | '.' | ')') || ch.is_ascii_digit()
}

fn strip_newline(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}

/// What a Markdown parser reproduces character for character.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verbatim {
    Code,
    Html,
    FrontMatter,
}

/// Classify each line of a document, indexed from zero. Asking the parser
/// beats re-deriving the fence and indentation rules, which depend on the
/// enclosing list.
pub(crate) fn verbatim_lines(input: &str, options: &Options<'_>) -> Vec<Option<Verbatim>> {
    let arena = Arena::new();
    let root = parse_document(&arena, input, options);
    let mut lines = vec![None; input.lines().count() + 1];

    for node in root.descendants() {
        let data = node.data.borrow();
        let kind = match data.value {
            NodeValue::CodeBlock(_) => Verbatim::Code,
            NodeValue::HtmlBlock(_) => Verbatim::Html,
            NodeValue::FrontMatter(_) => Verbatim::FrontMatter,
            _ => continue,
        };
        for line in data.sourcepos.start.line..=data.sourcepos.end.line {
            if let Some(slot) = lines.get_mut(line.saturating_sub(1)) {
                *slot = Some(kind);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{extract, extract_inline};
    use crate::FormatOptions;

    fn formatted(input: &str) -> String {
        let held = extract(input, FormatOptions::default(), &crate::comrak_options());
        held.restore(&held.text).expect("placeholders survive")
    }

    /// The Markdown handed to the parser, with every span replaced by an
    /// index, so a test can say which spans were recognized and where.
    fn protected(input: &str) -> String {
        let inline = extract_inline(input, &crate::comrak_options());
        let held = inline.text.replace(&inline.token, "@");
        assert_eq!(
            inline.restore(&inline.text).as_deref(),
            Some(input),
            "spans do not round-trip"
        );
        held
    }

    #[test]
    fn rescues_an_equation_from_becoming_a_setext_heading() {
        assert_eq!(formatted("$$\nu\n=\ng\n$$\n"), "$$\nu = g\n$$\n");
    }

    #[test]
    fn normalizes_environment_rows_and_is_idempotent() {
        let expected = "$$\n\\begin{aligned}\n  a &= b \\\\\n  c &= d\n\\end{aligned}\n$$\n";
        let input = "$$\n\\begin{aligned}\na\n&\n=\nb \\\\\nc &= d\n\\end{aligned}\n$$\n";
        assert_eq!(formatted(input), expected);
        assert_eq!(formatted(expected), expected);
    }

    #[test]
    fn holds_blocks_inside_quotes_and_list_items() {
        assert_eq!(
            formatted("> $$\n> a\n> =\n> b\n> $$\n"),
            "> $$\n> a = b\n> $$\n"
        );
        assert_eq!(
            formatted("  $$\n  a\n  =\n  b\n  $$\n"),
            "  $$\n  a = b\n  $$\n"
        );
    }

    #[test]
    fn preserves_code_blocks_and_raw_html() {
        for input in [
            "```latex\n$$\na\n=\nb\n$$\n```\n",
            "~~~latex\n$$\na\n=\nb\n$$\n~~~\n",
            "> ```latex\n> $$\n> a\n> =\n> b\n> $$\n> ```\n",
            "<pre>\n$$\na\n=\nb\n$$\n</pre>\n",
        ] {
            assert_eq!(formatted(input), input, "{input:?}");
        }
    }

    #[test]
    fn keeps_an_equation_with_a_blank_line_verbatim_rather_than_reflowing_it() {
        // The delimiters no longer bracket one Markdown block, so the layout
        // the author meant by the blank line cannot be second-guessed; the
        // equation is held aside only to keep `= a` and `* b` out of the
        // block parser's hands.
        assert_eq!(formatted("$$\n= a\n\n* b\n  $$\n"), "$$\n= a\n\n* b\n$$\n");
        assert_eq!(
            formatted("> $$\n> = a\n>\n> * b\n> $$\n"),
            "> $$\n> = a\n>\n> * b\n> $$\n"
        );
    }

    #[test]
    fn holds_inline_latex_spans_aside_but_not_code_escapes_or_dollar_math() {
        assert_eq!(protected("rate \\(\\alpha\\) here\n"), "rate @0@ here\n");
        assert_eq!(protected("\\(a\\) and \\(b\\)\n"), "@0@ and @1@\n");
        for input in [
            // Inline code and dollar math own what is between their own
            // delimiters, and `\\` is an escaped backslash, not an opener.
            "`\\(x\\)` and $\\(x\\)$ and $$\\(x\\)$$\n",
            "escaped \\\\(x\\\\)\n",
            "```\n\\(x\\)\n```\n",
            // A span is never continued onto the next line.
            "unclosed \\(x\n",
        ] {
            assert_eq!(protected(input), input, "{input:?}");
        }
    }

    #[test]
    fn leaves_indented_code_unsupported_environments_and_broken_blocks_alone() {
        for input in [
            "    $$\n    a\n    =\n    b\n    $$\n",
            "$$\n\\begin{matrix}\na&b\n\\end{matrix}\n$$\n",
            "$$\na\n=\nb\n",
            "$$\n\n$$\n",
            "$$\na\n=\nb\n> $$\n",
            // The delimiters close on the opening line with prose after
            // them: a later `\]` must not be read as this one's closer.
            "\\[TODO: link\\].\n\nprose\n\nhere\\]\n",
        ] {
            assert_eq!(formatted(input), input, "{input:?}");
        }
    }
}
