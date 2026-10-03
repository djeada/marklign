//! Undo the Markdown serializer's precautions where they change nothing.
//!
//! Comrak escapes punctuation it cannot prove harmless without the context
//! around it: `### 1\. Setup`, `![nozzle\_flow](...)`, `\~400 m/s`. Each is
//! correct and each renders exactly as the bare character would, but the
//! author did not write it and a diff full of them hides the changes that
//! matter. It also separates two adjacent lists with `<!-- end list -->`
//! even where a change of list type already keeps them apart.
//!
//! Rather than re-deriving where Markdown would read a bare character as
//! syntax, each candidate is tried and kept only if the document still
//! renders to the same HTML. That check is what protects mathematics, code
//! spans, link references, and tables: there the character matters, so
//! removing its escape changes what is rendered.

use comrak::{Options, markdown_to_html};

use crate::preparse::{self, Verbatim};

/// Punctuation whose escape is tried. Each means the same to GitHub as to
/// Comrak, so an HTML match here is a match there. Brackets are not among
/// them: `\[...\]` is display mathematics to GitHub, and Comrak, with its
/// LaTeX math extension off, would call the bare brackets equivalent.
const NEEDLESS: &[u8] = b"._#>~<";

const SEPARATOR: &str = "<!-- end list -->";

pub(crate) fn tidy(document: &str, options: &Options<'_>) -> String {
    let mut options = options.clone();
    // Raw HTML must reach the comparison, or every comment and tag would
    // render as the same placeholder.
    options.render.r#unsafe = true;
    let lines = drop_separators(document, &options);
    drop_escapes(lines, &options).concat()
}

fn html(lines: &[String], options: &Options<'_>) -> String {
    markdown_to_html(&lines.concat(), options)
}

/// A rendered document with the list separators taken out, so that two
/// renderings compare equal exactly when they hold the same lists.
fn without_separators(rendered: &str) -> String {
    rendered.replace(&format!("{SEPARATOR}\n"), "")
}

/// Remove each `<!-- end list -->` whose lists stay apart without it.
fn drop_separators(document: &str, options: &Options<'_>) -> Vec<String> {
    let mut lines: Vec<String> = document.split_inclusive('\n').map(str::to_owned).collect();
    if !document.contains(SEPARATOR) {
        return lines;
    }
    let target = without_separators(&html(&lines, options));
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim() != SEPARATOR {
            index += 1;
            continue;
        }
        let blank = lines
            .get(index + 1)
            .is_some_and(|line| line.trim().is_empty());
        let mut trial = lines.clone();
        trial.drain(index..index + 1 + usize::from(blank));
        if without_separators(&html(&trial, options)) == target {
            lines = trial;
        } else {
            index += 1;
        }
    }
    lines
}

/// Remove each escape that changes nothing, a line at a time: all of a
/// line's escapes at once when that is safe, otherwise one by one.
fn drop_escapes(mut lines: Vec<String>, options: &Options<'_>) -> Vec<String> {
    let verbatim = preparse::verbatim_lines(&lines.concat(), options);
    let mut target = None;

    for index in 0..lines.len() {
        if matches!(
            verbatim.get(index),
            Some(Some(
                Verbatim::Code | Verbatim::Html | Verbatim::FrontMatter
            ))
        ) {
            continue;
        }
        let positions = candidates(&lines[index]);
        if positions.is_empty() {
            continue;
        }
        let target = target.get_or_insert_with(|| html(&lines, options));

        let original = lines[index].clone();
        lines[index] = without(&original, &positions);
        if html(&lines, options) == *target {
            continue;
        }
        lines[index] = original;
        if positions.len() == 1 {
            continue;
        }
        // Right to left, so the positions still to come stay valid.
        for &position in positions.iter().rev() {
            let original = lines[index].clone();
            lines[index] = without(&original, &[position]);
            if html(&lines, options) != *target {
                lines[index] = original;
            }
        }
    }
    lines
}

/// Byte offsets of the backslashes escaping a character in `NEEDLESS`.
fn candidates(line: &str) -> Vec<usize> {
    let bytes = line.as_bytes();
    let mut positions = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\\' {
            if NEEDLESS.contains(&bytes[index + 1]) {
                positions.push(index);
            }
            // An escaped backslash is not the start of another escape.
            index += 2;
        } else {
            index += 1;
        }
    }
    positions
}

fn without(line: &str, positions: &[usize]) -> String {
    let mut output = String::with_capacity(line.len());
    let mut last = 0;
    for &position in positions {
        output.push_str(&line[last..position]);
        last = position + 1;
    }
    output.push_str(&line[last..]);
    output
}

#[cfg(test)]
mod tests {
    use super::{candidates, tidy};

    fn tidied(input: &str) -> String {
        tidy(input, &crate::comrak_options())
    }

    #[test]
    fn finds_escapes_but_not_escaped_backslashes() {
        assert_eq!(candidates(r"1\. a\_b \\_ \$"), vec![1, 5]);
    }

    #[test]
    fn drops_escapes_that_render_the_same() {
        assert_eq!(tidied("### 1\\. Setup\n"), "### 1. Setup\n");
        assert_eq!(tidied("![a\\_b](x.png)\n"), "![a_b](x.png)\n");
        assert_eq!(
            tidied("about \\~400 m/s, GM \\> 6\n"),
            "about ~400 m/s, GM > 6\n"
        );
    }

    #[test]
    fn keeps_escapes_that_matter() {
        for input in [
            "1\\. not a list\n",
            "\\# not a heading\n",
            "\\> not a quote\n",
            "\\[x\\]\n\n[x]: https://example.com\n",
            "\\[a = b\\]\n",
            "$a\\_b$\n",
            "`a\\_b`\n",
            "\\~a~\n",
        ] {
            assert_eq!(tidied(input), input, "{input:?}");
        }
        // Escaping the opener alone is enough to keep emphasis away.
        assert_eq!(tidied("a \\_b\\_ c\n"), "a \\_b_ c\n");
    }

    #[test]
    fn drops_only_the_list_separators_that_change_nothing() {
        // A change of list type ends the list on its own.
        assert_eq!(
            tidied("- a\n\n<!-- end list -->\n\n1. b\n"),
            "- a\n\n1. b\n"
        );
        // Two bullet lists would merge into one.
        let kept = "- a\n\n<!-- end list -->\n\n- b\n";
        assert_eq!(tidied(kept), kept);
    }
}
