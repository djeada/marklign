# Changelog

All notable changes to marklign are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
