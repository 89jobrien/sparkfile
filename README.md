# sparkfile

Project scaffolding CLI for creating new projects with consistent workspace conventions.

## Usage

- `sparkfile new rust-cli <name>`
- `sparkfile new rust-cli <name> --description "Short description"`
- `sparkfile new rust-cli <name> --root /path/to/workspace`

The `rust-cli` preset creates a Rust 2024 CLI skeleton with local guidance, validation commands, ignore rules, and handoff context.

Scaffold definitions are populated from YAML files in `scaffolds/`. The default `rust-cli` preset is defined in `scaffolds/rust-cli.yaml`.

## Development

- `just check`: run `cargo check --all-targets`
- `just lint`: run `cargo clippy --all-targets -- -D warnings`
- `just test`: run `cargo test`
- `just gate`: run the local validation gate
