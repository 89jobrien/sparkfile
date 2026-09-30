# Sparkfile — Agent Operating Guide

Sparkfile is a Rust CLI that scaffolds new projects with consistent workspace
conventions. It generates boilerplate code, configuration files, and documentation
based on named presets (`rust-cli`, `repo-site`).

## Build, Lint, and Test Commands

### Quick Reference

```bash
# Format code
just fmt

# Verify formatting
just fmt-check

# Check compilation
just check

# Lint with clippy
just lint

# Run tests
just test

# Full local CI gate (format + check + lint + test)
just gate
```

### Running Individual Tests

```bash
# Run all tests
cargo test

# Run a specific test by name
cargo test test_name

# Run with output capture disabled
cargo test -- --nocapture

# Run a specific module's tests
cargo test --lib domain::
```

## Code Style Guidelines

### Rust Version & Toolchain

- **Rust Edition**: 2024
- **License**: MIT (single)

### Formatting (rustfmt)

All Rust code follows standard rustfmt defaults. Run `just fmt` to auto-format.

### Linting (clippy)

- **Strict linting**: `cargo clippy --all-targets -- -D warnings`
- Warnings are treated as errors and must be fixed before commits

### Naming Conventions

- **Structs/Enums**: PascalCase (`ProjectSpec`, `Preset`)
- **Functions/Methods/Variables**: snake_case (`generate`, `write_files`)
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case (`scaffold.rs`, `domain.rs`)

### Error Handling

- Primary error types: `SpecError`, `ScaffoldError`, `WriteError`
- Use `Result<T>` types with descriptive error variants
- Propagate errors to CLI layer for user-facing messages

## Project Structure

```
sparkfile/
├── src/
│   ├── main.rs              # CLI parsing and command wiring
│   ├── lib.rs               # Library root (exports public modules)
│   ├── domain.rs            # ProjectSpec, Preset, validation
│   ├── scaffold.rs          # YAML scaffold loading, interpolation
│   └── fs.rs                # Filesystem writing adapter
├── scaffolds/
│   └── rust-cli.yaml        # Default scaffold definition
├── Cargo.toml               # Package metadata
├── justfile                 # Build and test recipes
└── CLAUDE.md                # Project guidance
```

## Key Modules

**`domain`** — Project specifications, preset definitions, and validation logic.

**`scaffold`** — Loads YAML scaffold files, performs variable interpolation,
and generates a list of files to create.

**`fs`** — Writes generated files to disk with proper error handling.

**`main`** — CLI argument parsing via a simple state machine; dispatches to
domain and scaffold layers.

## CLI Interface

```bash
sparkfile new <preset> <name> [--description <text>] [--root <path>]
```

**Arguments:**

- `<preset>`: Preset name (`rust-cli`, `repo-site`)
- `<name>`: Project name (becomes project directory and Cargo package name)

**Options:**

- `--description <text>`: Project description (default: "{name} project.")
- `--root <path>`: Target directory (default: current directory)

**Example:**

```bash
sparkfile new rust-cli my-app --description "A CLI tool" --root ~/projects
```

## The `repo-site` preset

Contract-bound to `repo-reference-site`'s `shared.css`. Two rules, both enforced
by tests rather than by convention:

- It defines **all 62 tokens** that layer references — the 61 in `shared.css`
  plus the signature-only `--color-ink-2`. A token renamed on either side would
  otherwise surface only at `validate-css.mjs` time, in a different repo, much
  later.
- Its generated `tokens.css` and `signature.css` carry **no `SHARED-LAYER`
  sentinels**, because `assemble-css.sh` composes them around the shared layer
  and refuses to run if they collide.

It emits `@font-face` blocks for IBM Plex Sans and IBM Plex Mono, and must be
paired with `fetch-fonts.sh` using the same three families:

```bash
bash <repo-reference-site>/scripts/fetch-fonts.sh <repo> \
  "ibm-plex-sans:var" "ibm-plex-mono:400" "ibm-plex-mono:700"
```

A `@font-face` pointing at a file that was never fetched fails validation, not
just the browser — that was the first end-to-end failure.

## Conventions

- Rust edition 2024. Do not change.
- Treat clippy warnings as errors (`-D warnings`).
- Keep scaffold YAML definitions small and explicit.
- Do not initialize git or mutate parent workspace files from generated
  projects in v1.
- Generated projects should be minimal but complete — include CLAUDE.md,
  justfile, Cargo.toml, and a basic main.rs.

## Commit Guidelines

Format commits using conventional style:

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.

**Example:**

```
feat(scaffold): add rust-cli preset support
fix(fs): handle existing file creation errors
docs: update CLI usage in README
```

Before committing, run `just gate` to verify all checks pass.
