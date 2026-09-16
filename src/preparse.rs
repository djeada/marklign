//! Protect recognized display-math environments from Markdown block parsing.
//!
//! A source line containing only `=` can become a Markdown setext heading
//! before Comrak recognizes its surrounding `$$` delimiters. Format recognized
//! top-level environments before parsing, and skip fenced-code contents.

use crate::environments;

pub(crate) fn normalize_environments(input: &str) -> String {
    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    let mut output = String::with_capacity(input.len());
    let mut fence: Option<(u8, usize)> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim_end_matches(['\r', '\n']);

        if let Some((marker, length)) = fence {
            output.push_str(lines[index]);
            if closes_fence(line, marker, length) {
                fence = None;
            }
            index += 1;
            continue;
        }
        if let Some(open) = opens_fence(line) {
            fence = Some(open);
            output.push_str(lines[index]);
            index += 1;
            continue;
        }

        // Only unindented, complete, standalone dollar blocks are eligible.
        // This deliberately excludes math inside blockquotes, lists, and code.
        if line == "$$" {
            if let Some(end) = ((index + 1)..lines.len())
                .find(|&end| lines[end].trim_end_matches(['\r', '\n']) == "$$")
            {
                let body = lines[index + 1..end].concat();
                if let Some(formatted) = environments::format_environment(&body) {
                    output.push_str(lines[index]);
                    output.push_str(&formatted);
                    output.push('\n');
                    output.push_str(lines[end]);
                } else {
                    for original in &lines[index..=end] {
                        output.push_str(original);
                    }
                }
                index = end + 1;
                continue;
            }
        }

        output.push_str(lines[index]);
        index += 1;
    }
    output
}

fn opens_fence(line: &str) -> Option<(u8, usize)> {
    let indentation = line.bytes().take_while(|&byte| byte == b' ').count();
    if indentation > 3 {
        return None;
    }
    let rest = line.get(indentation..)?.as_bytes();
    let marker = *rest.first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = rest.iter().take_while(|&&byte| byte == marker).count();
    if length < 3 || (marker == b'`' && rest[length..].contains(&b'`')) {
        return None;
    }
    Some((marker, length))
}

fn closes_fence(line: &str, marker: u8, length: usize) -> bool {
    let indentation = line.bytes().take_while(|&byte| byte == b' ').count();
    if indentation > 3 {
        return false;
    }
    let rest = &line.as_bytes()[indentation..];
    let count = rest.iter().take_while(|&&byte| byte == marker).count();
    count >= length && rest[count..].iter().all(u8::is_ascii_whitespace)
}

#[cfg(test)]
mod tests {
    use super::normalize_environments;

    #[test]
    fn fixes_an_operator_only_line_before_markdown_parsing() {
        let input = "$$\n\\begin{aligned}\na\n&\n=\nb \\\\\nc &= d\n\\end{aligned}\n$$\n";
        let expected = "$$\n\\begin{aligned}\n  a &= b \\\\\n  c &= d\n\\end{aligned}\n$$\n";
        assert_eq!(normalize_environments(input), expected);
        assert_eq!(normalize_environments(expected), expected);
    }

    #[test]
    fn preserves_fenced_code_for_backticks_and_tildes() {
        for fence in ["```latex", "~~~latex"] {
            let input = format!("{fence}\n$$\n\\begin{{aligned}}\na\n&\n=\nb\n\\end{{aligned}}\n$$\n{fence}\n");
            assert_eq!(normalize_environments(&input), input);
        }
    }

    #[test]
    fn leaves_unsupported_environments_unchanged() {
        let input = "$$\n\\begin{matrix}\na&b\n\\end{matrix}\n$$\n";
        assert_eq!(normalize_environments(input), input);
    }
}
