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

This is still an early-stage formatter, **not a general TeX parser**. The math pass deliberately leaves expressions with `\begin`/`\end` environments, explicit `\\` line breaks, alignment markers (`&`), comments, TeX definition or metadata commands, or unmatched braces untouched. It does not format inline `$...$` math or the interiors of braced arguments. Comrak may independently normalize Markdown syntax and surrounding blank lines.

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
- `--math-style readable|compact` selects equation layout; default: `readable`.
- `--math-width N` controls preferred display-equation source width; default: `88`, minimum: `20`. Indivisible tokens and equations whose safe breakpoints do not fit may exceed it.

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
```

Tests cover the full Robin example, Dirichlet and Neumann conditions, operator-only input lines, unary minus, safe TeX token joining, unsupported constructs, math in Markdown versus code fences, front matter, and formatting idempotence. The CI workflow runs formatting, tests, and a CLI smoke test.

A future environment-aware LaTeX parser could safely support `align`, `cases`, comments, and more sophisticated line-break aesthetics. Those transformations are intentionally not attempted here.

Licensed under MIT.
