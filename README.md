# jfmt

Java linting CLI powered by Tree-sitter.

Workspace crates:
- `libjfmt`: Core lint logic using `tree-sitter` and `tree-sitter-java`.
- `jfmtcli`: Command-line interface.

Current rules:
- no-wildcard-imports: Flags `import x.y.*;`.
- no-empty-statement: Flags stray `;` statements.
- max-line-length: Flags lines longer than configured length.
- indent-style: Flags tabs/spaces not matching configured style.

Usage:
- Build: `cargo build -p jfmtcli`
- Run (lint only): `target/debug/jfmtcli path/to/File.java [more.java]`
- Run with autofix: `target/debug/jfmtcli --fix path/to/File.java [more.java]`

Output format:
- `path:line:column: rule-id: message`

Configuration:
- Location: `jfmt.toml` discovered by walking up from current directory.
- Fields:
  - `indent_style`: `"tabs"` or `"spaces"` (default: `"spaces"`).
  - `indent_width`: integer, spaces per indent when using spaces (default: `4`).
  - `max_line_length`: integer (default: `100`).

Example `jfmt.toml`:

```
# Prefer spaces with width 4
indent_style = "spaces"
indent_width = 4

# Enforce 100-char lines
max_line_length = 100
```

Autofix
- Invoke with `--fix` to apply safe fixes in-place.
- Supported fixes:
  - `no-empty-statement`: removes stray `;` statements.
  - `indent-style`:
    - spaces mode: converts leading tabs to spaces (using `indent_width`).
    - tabs mode: converts leading spaces to tabs when divisible by `indent_width` (skips mixed/unaligned).
- Not auto-fixed: `no-wildcard-imports`, `max-line-length` (needs semantic changes).
# jfmt
configurable java formatting based on tree sitter
