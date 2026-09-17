# marklign

Beautiful Markdown. Precise mathematics.

**marklign** is an opinionated Rust Markdown formatter with special care for display LaTeX. It parses Markdown with [Comrak](https://github.com/kivikakk/comrak) and treats existing source newlines in ordinary `$$...$$` equations as formatting noise, not as intentional mathematical layout. Explicit TeX line breaks (`\\`) are different and are preserved.

## The reason this exists

Input:

```markdown
$$
\alpha u
+
\beta\frac{\partial u}{\partial n} =
g
\qquad
\text{on } \partial\Omega.
$$
```

Default readable output:

```markdown
$$
\alpha u + \beta \frac{\partial u}{\partial n} = g
\qquad \text{on } \partial\Omega.
$$
```

There is a second reason. A `$$` delimiter is an *inline* construct, so a Markdown block parser reaches a line holding only `=` or `-` before it ever sees the equation around it, and reads that line as a setext heading underline. Run most Markdown formatters over

```markdown
$$
u
=
g
$$
```

and the equation comes back as a heading followed by loose prose. Marklign formats every equation that owns its source lines *before* parsing the document, holds it aside, and puts it back afterwards, so the mathematics cannot be reinterpreted as prose.

## Layout rules for ordinary display math

- Join orphaned `+`, `-`, and `=` lines into coherent expressions. Never deliberately produce a line consisting solely of one of these operators.
- Put `=` with the preceding expression and its right-hand side when possible; exceed the preferred width rather than splitting at the equality sign.
- Distinguish unary minus (`x = -y`) from subtraction (`x - y`).
- Separate operators and argument-taking commands such as `\beta \frac{a}{b}`; do not concatenate TeX control words with following identifiers or alter nested braced arguments.
- **No wrapped line may begin with `-`, `+`, `*`, `>`, `#`, `=`, or `|`.** Markdown reads such a line as a list item, heading, quote, or table row, which would break the equation in every other renderer. A sum therefore wraps *after* its operator, and where no safe break exists the line simply runs long.
- Prefer breaking outside brackets over breaking inside them.
- In the default **readable** style, put `\qquad` at the beginning of its accompanying contextual clause, not on a line alone. In **compact** style, prefer a single line when it fits.
- Wrap at safe token boundaries instead of splitting inside `\frac{...}{...}`, `\text{...}`, or other grouped arguments. The width is a preference, not an excuse for broken mathematics.

## Delimiters and block shape

- `$$...$$` and `\[...\]` blocks are both recognized, and each keeps the delimiters the author chose. Marklign reflows mathematics; it does not restyle delimiters.
- An equation written as a block keeps its delimiters on their own lines. An equation written on one line stays on one line, unless reflowing it no longer fits, in which case the delimiters move onto their own lines.
- Equations inside list items and block quotes are formatted too, and come back with their container's indentation or `>` markers.
- Inline math (`$...$`) and the interiors of braced arguments are never touched.

## Row-aware LaTeX environments

A standalone `aligned`, `split`, or `cases` environment occupying an entire display-math block is formatted row by row. Marklign preserves explicit TeX row breaks (`\\`) and top-level alignment tabs (`&`) instead of accidentally joining rows into a different equation.

Input:

```latex
$$
\begin{aligned}
\alpha u
&
=
g
+
h \\
\beta u
&=
k
\end{aligned}
$$
```

Output:

```latex
$$
\begin{aligned}
  \alpha u &= g + h \\
  \beta u &= k
\end{aligned}
$$
```

Each row is kept on one source line, indented by two spaces, to preserve its alignment. A row may exceed `--math-width`: unlike plain display math, Marklign does not introduce new explicit TeX row breaks to satisfy a width preference. Nested braced arguments and escaped `\&` do not become column boundaries.

**Conservative boundaries:** Only complete standalone environments with one of these three names are recognized. Nested or unknown environments, comments, optional row spacing such as `\\[2pt]`, starred row breaks, metadata/definition commands, malformed groups, and other constructs that cannot safely be understood are left unchanged by the math pass.

## What else Marklign leaves alone

Code blocks, raw HTML blocks, and front matter are located by asking the parser where they are, not by re-deriving fence and indentation rules, so their contents are reproduced character for character — including math that only looks like math:

````markdown
```latex
$$
a
=
b
$$
```
````

Comrak's own normalization of the surrounding Markdown still applies (list markers, emphasis characters, link style, blank lines). Two cosmetic artifacts of it are cleaned up: the `<!-- end list -->` comment it inserts before fenced code, and the container indentation it leaves on blank lines inside lists and quotes.

## Install and run

Requires Rust 1.85 or newer. Prebuilt binaries for Linux, macOS, and Windows are attached to each [release](https://github.com/djeada/marklign/releases).

```sh
cargo install --path .
```

```sh
marklign notes.md                   # print the formatted document; the file is untouched
marklign notes.md --write           # rewrite the file, only if it needs it
marklign docs --check               # exit 1 if any file under docs/ needs formatting
marklign docs notes.md --write      # any number of files and directories
cat notes.md | marklign -           # read standard input
marklign notes.md --math-style compact
marklign notes.md --math-width 72
```

- Without `--write` or `--check`, the formatted document goes to standard output, which requires a single input.
- `--write` rewrites each file that needs formatting and names it on standard error.
- `--check` changes nothing, names each file that needs formatting, and exits 1 if any does. Suitable for CI.
- `--write` and `--check` cannot be combined.
- A directory is searched for `.md` and `.markdown` files, skipping hidden entries, `target`, and `node_modules`. A file named explicitly is formatted whatever it is called.
- `--math-style readable|compact` selects layout for ordinary equations; default: `readable`. Environment rows use compact layout to preserve explicit alignment.
- `--math-width N` controls preferred ordinary display-equation source width; default: `88`, minimum: `20`. Indivisible tokens, environment rows, and equations whose safe breakpoints do not fit may exceed it.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Tests cover the layout rules, the environment pass, the CLI, and whole-document integrity: equations that would otherwise become headings, prose brackets that are not mathematics, hard line breaks, alerts, code blocks, front matter, and idempotence.

`examples/html_equivalence.rs` checks the property that matters most over a corpus of real documents: formatting must not change what a document renders to, beyond whitespace inside mathematics.

```sh
cargo run --example html_equivalence -- $(find ~/notes -name '*.md')
```

Marklign is still an early-stage formatter, **not a general TeX parser**. Full environment parsing, conditional alignment, nested expressions, and user configuration remain future work.

Licensed under MIT.
