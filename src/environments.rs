//! Row-aware formatting for a deliberately small set of display-math environments.
//!
//! Only an entire `aligned`, `split`, or `cases` environment is eligible. Row
//! delimiters and alignment tabs are scanned at brace depth zero, so braced
//! command arguments are never split. Anything ambiguous is left unchanged.

use crate::{MathStyle, math};

/// Return `None` if this is not a supported, safely understood environment.
pub(crate) fn format_environment(input: &str) -> Option<String> {
    let source = input.trim();
    let (open, close) = [
        (r"\begin{aligned}", r"\end{aligned}"),
        (r"\begin{split}", r"\end{split}"),
        (r"\begin{cases}", r"\end{cases}"),
    ]
    .into_iter()
    .find(|(open, close)| source.starts_with(open) && source.ends_with(close))?;
    let body = source.strip_prefix(open)?.strip_suffix(close)?;

    // Comments, nested environments, metadata, and row-break options need a
    // fuller parser. Preserve their source rather than guessing at semantics.
    if body.contains('%')
        || body.contains(r"\begin")
        || body.contains(r"\end")
        || body.contains(r"\tag")
        || body.contains(r"\label")
        || body.contains(r"\intertext")
        || body.contains(r"\verb")
        || body.contains(r"\def")
        || body.contains(r"\newcommand")
        || body.contains(r"\\[")
        || body.contains(r"\\*")
        || body.contains('\r')
    {
        return None;
    }

    let rows = split_top_level(body, Delimiter::Row)?;
    if rows.is_empty() || rows.iter().any(|row| row.trim().is_empty()) {
        return None;
    }

    let mut formatted_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let cells = split_top_level(row, Delimiter::Column)?;
        if cells.iter().all(|cell| cell.trim().is_empty()) {
            return None;
        }
        let mut formatted = String::new();
        for (index, cell) in cells.iter().enumerate() {
            let cell = cell.trim();
            // Preserve deliberately empty leading alignment columns. Refuse
            // other empty cells, whose intended column placement is unclear.
            if cell.is_empty() && index != 0 {
                return None;
            }
            let value = math::format_display_math(cell, MathStyle::Compact, usize::MAX);
            if value.contains('\n') {
                return None;
            }
            if index > 0 {
                if value.starts_with('=') {
                    formatted.push_str(" &");
                } else {
                    formatted.push_str(" & ");
                }
            }
            formatted.push_str(&value);
        }
        // Reject incomplete source rows rather than emitting an orphaned
        // operator or silently joining expressions across an explicit `\\`.
        if formatted
            .trim_end()
            .chars()
            .last()
            .is_some_and(|ch| matches!(ch, '+' | '-' | '='))
        {
            return None;
        }
        formatted_rows.push(format!("  {}", formatted.trim_end()));
    }

    Some(format!(
        "{open}\n{}\n{close}",
        formatted_rows.join(&format!(" {}\n", r"\\"))
    ))
}

#[derive(Clone, Copy)]
enum Delimiter {
    Row,
    Column,
}

/// Split only at top-level `\\` or `&`. ASCII delimiter offsets are always
/// UTF-8 boundaries even when the surrounding expression contains Unicode.
fn split_top_level(input: &str, delimiter: Delimiter) -> Option<Vec<&str>> {
    let bytes = input.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            if index + 1 >= bytes.len() {
                return None;
            }
            if bytes[index + 1] == b'\\' {
                // The outer row scan handles top-level row delimiters; a row
                // break inside a braced argument is deliberately unsupported.
                if depth > 0 {
                    return None;
                }
                if matches!(delimiter, Delimiter::Row) {
                    parts.push(&input[start..index]);
                    index += 2;
                    start = index;
                    continue;
                }
                // A column must not contain a row delimiter either.
                return None;
            }
            // Skip escaped braces, `\&`, and complete control symbols/words.
            index += 2;
            if bytes[index - 1].is_ascii_alphabetic() {
                while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                    index += 1;
                }
            }
            continue;
        }
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            b'&' if depth == 0 && matches!(delimiter, Delimiter::Column) => {
                parts.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if depth != 0 {
        return None;
    }
    parts.push(&input[start..]);
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::format_environment;

    #[test]
    fn formats_aligned_rows_without_losing_alignment() {
        let input = "\\begin{aligned}\n\\alpha u\n&\n=\ng\n+\nh \\\\\n\\beta u\n&=\nk\n\\end{aligned}";
        let expected = "\\begin{aligned}\n  \\alpha u &= g + h \\\\\n  \\beta u &= k\n\\end{aligned}";
        assert_eq!(format_environment(input).as_deref(), Some(expected));
        assert_eq!(format_environment(expected).as_deref(), Some(expected));
    }

    #[test]
    fn formats_cases_without_moving_column_markers() {
        let input = "\\begin{cases}\nx^2&x>0 \\\\\n0&\\text{otherwise}\n\\end{cases}";
        let expected = "\\begin{cases}\n  x^2 & x>0 \\\\\n  0 & \\text{otherwise}\n\\end{cases}";
        assert_eq!(format_environment(input).as_deref(), Some(expected));
    }

    #[test]
    fn keeps_escaped_ampersands_inside_groups() {
        let input = r"\begin{aligned} \text{A \& B} &= c \end{aligned}";
        let expected = "\\begin{aligned}\n  \\text{A \\& B} &= c\n\\end{aligned}";
        assert_eq!(format_environment(input).as_deref(), Some(expected));
    }

    #[test]
    fn leaves_unsupported_or_ambiguous_environments_unchanged() {
        for input in [
            r"\begin{matrix}a&b\\c&d\end{matrix}",
            r"\begin{aligned}\begin{cases}a&b\end{cases}\end{aligned}",
            r"\begin{aligned}a&=b\\[2pt]c&=d\end{aligned}",
            r"\begin{aligned}a&=b% comment\end{aligned}",
            r"\begin{aligned}a&=b\\\end{aligned}",
            r"\begin{aligned}a&={b\end{aligned}",
        ] {
            assert_eq!(format_environment(input), None, "{input}");
        }
    }
}
