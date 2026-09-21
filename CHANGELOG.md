# Changelog

All notable changes to marklign are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.0 - 2026-09-21

Mathematics reaches the renderer as mathematics.

### Fixed

- Inline `\(...\)` math is preserved. A CommonMark parser reads `\(` as an
  escaped parenthesis, so `\(\alpha\)` came back as `(\\alpha)`, which is no
  longer mathematics. The spans are now held aside during parsing and put
  back character for character, with the delimiters the author wrote.
  Inline code, `$`-delimited math, code blocks, and a genuine `\\(` escape
  are left alone. ([issue 3](https://github.com/djeada/marklign/issues/3))
- A display-math block containing a blank line is no longer handed to the
  Markdown block parser, which read its `=` line as a heading underline and
  its `*` line as a list item. Such a block is held aside like any other, but
  put back verbatim: a blank line ends the Markdown block the delimiters were
  meant to bracket, so what the author meant by it is not something this
  formatter can safely reflow. ([issue 4](https://github.com/djeada/marklign/issues/4))
- An equation neither layout pass will touch no longer keeps a line a Markdown
  block parser claims. A lone `=` between two `\end{bmatrix}`/`\begin{bmatrix}`
  lines underlines the line above it as a setext heading, so the block rendered
  as an `<h1>` holding half the matrix and a paragraph holding the rest, with
  the `\\` row breaks eaten as prose escapes. Such a line is now joined to the
  line above it however the equation was laid out, which is the smallest change
  that keeps it an equation. A block holding a `%` comment or `\verb` is still
  left alone.
- A line of mathematics that is nothing but `>` no longer ends the block: `x`
  over `>` over `y` came back as a block quote.
- A paragraph whose delimiters close on its own first line, as in the
  `\[TODO: ...\].` Comrak writes around a prose bracket, no longer opens a
  search for a closing delimiter further down the document.

### Changed

- **A trailing `.` or `,` is dropped from a display equation by default.** It
  is prose that wandered into the mathematics, and a renderer sets it in math
  italic among the symbols. `--keep-trailing-punctuation` restores the old
  behavior. Only one mark, and only where it is really punctuation: `\,`,
  `\right.`, `...`, and a period inside `\text{...}` are untouched.
- `FormatOptions` has a third field, `trailing_punctuation`, so a literal
  construction of it needs `..FormatOptions::default()`.

## 0.2.0 - 2026-09-17

The command line now behaves the way Black's does.

### Changed

- **Files are reformatted in place by default.** `--write` is gone: running
  `marklign notes.md` rewrites the file, and `-` formats standard input to
  standard output. Printing a formatted document without writing it is
  `marklign - < notes.md`.
- Each run ends with a summary line counting the files reformatted, left
  unchanged, and failed, and names each reformatted file on standard error as
  `reformatted <path>`, or `would reformat <path>` when only looking.
- Exit codes follow Black: `0` success, `1` a file would be reformatted under
  `--check` or `--diff`, `123` a file could not be formatted or the command
  line was wrong. A directory that holds no Markdown is no longer an error.
- A directory walk honors `.gitignore`. A file named on the command line is
  still formatted, whatever `.gitignore` says.
- A path the walk cannot read is reported and the run continues over the rest
  of the project, rather than stopping before anything is formatted. The run
  still ends in `123`.

### Fixed

- A directory walk formats `README.MD` again: matching an extension by regular
  expression had quietly made the default case-sensitive.

### Added

- `--diff`, printing a unified diff instead of writing, combinable with
  `--check`.
- `--verbose`, which names the files that were already formatted, and
  `--quiet`, which prints nothing but errors.
- `--include`, `--exclude`, `--extend-exclude`, and `--force-exclude`: regular
  expressions over the path, matched as in Black, with `--force-exclude`
  applying to files named on the command line as a pre-commit hook needs.

## 0.1.0 - 2026-09-17

First release.

### Added

- Token-aware reflowing of ordinary display equations, in a `readable` style
  that keeps `\qquad` with the clause it introduces and a `compact` style that
  prefers one line, with a preferred width (`--math-width`).
- Row-aware formatting of standalone `aligned`, `split`, and `cases`
  environments, preserving explicit TeX row breaks and top-level alignment
  tabs.
- Recognition of both `$$...$$` and `\[...\]` display blocks, including ones
  inside list items and block quotes, each keeping the delimiters its author
  chose.
- A command line that accepts any number of files and directories as well as
  standard input, with `--write` and a CI-suitable `--check`.
- Prebuilt binaries for Linux (gnu and musl), macOS, and Windows, attached to
  each tagged release.

### Notes on correctness

A `$$` delimiter is an inline construct, so a Markdown block parser reaches a
line holding only `=` or `-` inside an equation first and reads it as a setext
heading underline. Marklign formats every equation that owns its source lines
before the document is parsed and holds it aside, so mathematics is never
reinterpreted as prose. Wrapped lines never begin with a character Markdown
would claim, which keeps the output correct in other renderers too.
