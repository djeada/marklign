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

The formatter's layout rules for **ordinary display math**:

- Join orphaned `+`, `-`, and `=` lines into coherent expressions. Never deliberately produce a line consisting solely of one of these operators.
- Put `=` with the preceding expression and its right-hand side when possible; exceed the preferred width rather than splitting at the equality sign.
- Distinguish unary minus (`x = -y`) from subtraction (`x - y`).
- Separate operators and argument-taking commands such as `\beta \frac{a}{b}`; do not concatenate TeX control words with following identifiers or alter nested braced arguments.
- In the default **readable** style, put `\qquad` at the beginning of its accompanying contextual clause, not on a line alone. In **compact** style, prefer a single line when it fits.
- Wrap at safe token boundaries instead of splitting inside `\frac{...}{...}`, `\text{...}`, or other grouped arguments. The width is a preference, not an excuse for broken mathematics.

## Row-aware LaTeX environments

A standalone `aligned`, `split`, or `cases` environment occupying an entire `$$...$$` block is formatted row by row. Marklign preserves explicit TeX row breaks (`\\`) and top-level alignment tabs (`&`) instead of accidentally joining rows into a different equation.

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

**Conservative boundaries:** Only complete standalone environments with one of these three names are recognized. Nested or unknown environments, comments, optional row spacing such as `\\[2pt]`, starred row breaks, metadata/definition commands, malformed groups, and other constructs that cannot safely be understood are left unchanged by the math pass. Inline math and the interiors of braced arguments are not formatted. Comrak may independently normalize surrounding Markdown and blank lines.

## Install and run

Requires Rust 1.85 or newer.

```sh
cargo build
cargo run -- examples/boundary_conditions.md
cargo run -- examples/boundary_conditions.md --write
cargo run -- examples/boundary_conditions.md --check
cargo run -- examples/boundary_conditions.md --math-style compact
cargo run -- examples/boundary_conditions.md --math-width 72
```

After building, use `target/debug/marklign` in place of `cargo run --`.

- Without flags, formatted Markdown is printed to stdout; the input is unchanged.
- `--write` overwrites the file only if it needs formatting.
- `--check` makes no changes and exits 1 if formatting is needed (0 otherwise).
- `--write` and `--check` cannot be combined.
- `--math-style readable|compact` selects layout for ordinary equations; default: `readable`. Environment rows use compact layout to preserve explicit alignment.
- `--math-width N` controls preferred ordinary display-equation source width; default: `88`, minimum: `20`. Indivisible tokens, environment rows, and equations whose safe breakpoints do not fit may exceed it.

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
```

Tests cover Robin, Dirichlet, Neumann, alignment-row normalization, cases, preserved escaped ampersands, unsupported nested environments, operator-only input lines, unary minus, safe TeX token joining, math in Markdown versus code fences, front matter, and formatting idempotence. CI runs formatting, tests, and a CLI smoke test.

Marklign is still an early-stage formatter, **not a general TeX parser**. Full environment parsing, conditional alignment, nested expressions, and user configuration remain future work.

Licensed under MIT.
