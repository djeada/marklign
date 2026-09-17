# Changelog

All notable changes to marklign are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
