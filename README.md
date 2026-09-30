# sparkfile

Project scaffolding CLI for creating new projects with consistent workspace conventions.

## Usage

- `sparkfile new rust-cli <name>`
- `sparkfile new rust-cli <name> --description "Short description"`
- `sparkfile new rust-cli <name> --root /path/to/workspace`
- `sparkfile new repo-site <name> --description "Short description"`

The `rust-cli` preset creates a Rust 2024 CLI skeleton with local guidance, validation commands, ignore rules, and handoff context.

The `repo-site` preset creates a GitHub Pages reference site: `site/index.html`, a complete `tokens.css` defining all 62 tokens the shared layer references, a `signature.css` stub, `.nojekyll`, and a deploy workflow. The generated site passes `validate-css.mjs` as-is; what it does not contain is real content. Every placeholder is marked `REPLACE`.

```bash
sparkfile new repo-site kiln --root ~/dev
bash ~/.agents/skills/repo-reference-site/scripts/fetch-fonts.sh ~/dev/kiln \
  "ibm-plex-sans:var" "ibm-plex-mono:400" "ibm-plex-mono:700"
bash ~/.agents/skills/repo-reference-site/scripts/assemble-css.sh ~/dev/kiln \
  ~/dev/kiln/site/tokens.css ~/dev/kiln/site/signature.css
```

Run it against a directory whose `site/` does not already exist: generated files never overwrite existing paths.

## Development

- `just check`: run `cargo check --all-targets`
- `just lint`: run `cargo clippy --all-targets -- -D warnings`
- `just test`: run `cargo test`
- `just gate`: run the local validation gate
