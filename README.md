# marklign

Beautiful Markdown. Precise mathematics.

**marklign** is an early-stage Rust CLI for formatting Markdown containing mathematical notation. It uses [Comrak](https://github.com/kivikakk/comrak) to parse and serialize Markdown, recognizes `$...$` and `$$...$$` math, and makes only one deliberately safe math transformation so far: a display-math line consisting of `u=g`-style ASCII identifiers becomes `u = g`.

Complex LaTeX (including commands such as `\frac`, `\text`, `\qquad`, and multiline operators) and inline math are not rewritten by the math-specific pass. Markdown serialization may still normalize surrounding syntax and blank lines. This is a foundation, **not** a complete LaTeX formatter.

## Install and run

Requires Rust 1.85 or newer.

```sh
cargo build
cargo run -- examples/boundary_conditions.md
cargo run -- examples/boundary_conditions.md --write
cargo run -- examples/boundary_conditions.md --check
```

After building, use `target/debug/marklign` in place of `cargo run --`.

- Without flags, the formatted file is printed to stdout and the original is unchanged.
- `--write` overwrites the file when formatting changes are needed.
- `--check` makes no changes and returns exit code 1 if formatting is needed (0 otherwise). This is useful in CI.
- `--write` and `--check` cannot be used together.

## Example

Input:

```markdown
### Boundary conditions
For $\Omega$:

$$
u=g
$$
```

Output includes normalized Markdown spacing and the simple display-math rule:

```markdown
### Boundary conditions

For $\Omega$:

$$
u = g
$$
```

See [`examples/boundary_conditions.md`](examples/boundary_conditions.md) for a larger LaTeX sample.

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
```

The unit tests cover simple equations, preservation of complex math and code fences, inline math, front matter, and idempotence. CI also exercises the CLI's `--write` and `--check` modes. Formatting beyond simple equations, configurable styles, and a general-purpose LaTeX tokenizer are future work.

Licensed under MIT.
