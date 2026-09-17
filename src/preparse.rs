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

/// Placeholder stem: letters only, so Markdown never escapes or reflows it.
const TOKEN: &str = "MARKLIGNMATHBLOCK";

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

    let mut source = String::from(first);
    for (offset, line) in lines[open + 1..].iter().enumerate() {
        source.push('\n');
        // A fenced block can interrupt a paragraph, and its contents are not
        // mathematics however much they look like it.
        if verbatim[open + 1 + offset].is_some() {
            return None;
        }
        let line = strip_newline(line);
        // A blank line ends the enclosing Markdown block, so the equation was
        // never one block; leaving the container likewise ends it.
        if line.trim().is_empty() {
            return None;
        }
        let content = strip_container(line, depth)?;
        let Some(at) = close_at(content, closer) else {
            // A delimiter with prose after it is not a block to reflow.
            if content.contains(closer) {
                return None;
            }
            source.push_str(content);
            continue;
        };
        source.push_str(&content[..at]);
        let body = format_block(&source, preferences);
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

/// Apply the same layout rules the Markdown pass applies to inline math.
///
/// Whitespace picked up from the delimiter lines goes first: a `$$ ` opener
/// must not leave a blank first line in an equation kept verbatim, which on a
/// later run would read as the end of the block.
fn format_block(source: &str, preferences: FormatOptions) -> String {
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
    let source = body.join("\n");

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
    use super::extract;
    use crate::FormatOptions;

    fn formatted(input: &str) -> String {
        let held = extract(input, FormatOptions::default(), &crate::comrak_options());
        held.restore(&held.text).expect("placeholders survive")
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
    fn leaves_indented_code_unsupported_environments_and_broken_blocks_alone() {
        for input in [
            "    $$\n    a\n    =\n    b\n    $$\n",
            "$$\n\\begin{matrix}\na&b\n\\end{matrix}\n$$\n",
            "$$\na\n=\nb\n",
            "$$\n\n$$\n",
            "$$\na\n=\nb\n> $$\n",
        ] {
            assert_eq!(formatted(input), input, "{input:?}");
        }
    }
}
