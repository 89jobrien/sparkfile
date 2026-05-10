# CLAUDE.md

Guidance for working on `sparkfile`.

## Project

`sparkfile` is a Rust CLI that scaffolds new projects with consistent workspace conventions.

## Commands

- `just fmt`: format Rust code
- `just fmt-check`: verify Rust formatting
- `just check`: run `cargo check --all-targets`
- `just lint`: run `cargo clippy --all-targets -- -D warnings`
- `just test`: run `cargo test`
- `just gate`: run the full local validation gate

## Architecture

- `src/domain.rs`: project specs, presets, and validation
- `src/scaffold.rs`: YAML scaffold definition loading, interpolation, and generated file planning
- `src/fs.rs`: filesystem writing adapter
- `src/main.rs`: CLI parsing and command wiring
- `scaffolds/rust-cli.yaml`: default scaffold definition for the `rust-cli` preset

## Conventions

- Rust edition 2024.
- Treat warnings as errors with `cargo clippy -- -D warnings`.
- Keep scaffold YAML small and explicit.
- Do not initialize git or mutate parent workspace files from generated project creation in v1.
