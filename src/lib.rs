//! Scaffold projects, and build verifiable agent harness plugins.
//!
//! Three presets of scaffolding, plus a site generator that reads a Rust
//! workspace's own `cargo metadata` rather than a hand-written page:
//!
//! - `rust-cli` — a CLI project skeleton
//! - `repo-site` — a GitHub Pages reference site, contract-bound to
//!   repo-reference-site's shared component layer
//! - [`sitegen`] — multi-page sites whose sections are derived from
//!   `cargo metadata`, so a page cannot drift from the crate it documents

pub mod codemeta;
pub mod docs;
pub mod domain;
pub mod fs;
pub mod help;
pub mod scaffold;
pub mod sitegen;
