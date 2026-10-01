//! Prose extraction from a repository's own markdown.
//!
//! `cargo metadata` says what is in the workspace; it says nothing about why
//! the project exists. That reasoning lives in the README, and duplicating it
//! into a generated page means it is out of date the moment either side
//! changes. So the guide page is assembled from the README rather than
//! written alongside it.
//!
//! The markdown handling is a deliberate subset: headings, fenced code, and
//! inline code. A full CommonMark implementation is not needed to move a
//! README's structure into HTML, and pulling one in would mean rendering
//! whatever HTML the source happens to contain.

use std::path::Path;

/// One heading and the prose beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub level: u8,
    pub title: String,
    pub body: String,
}

/// The markdown a repository carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Docs {
    /// First prose paragraph of the README, with the `# Title` and any badge
    /// lines stripped. This is the sentence that explains the project, and
    /// Cargo's `description` field is usually too terse to lead with.
    pub lede: Option<String>,
    /// `##`-level sections of the README, in document order.
    pub sections: Vec<Section>,
    /// File names in `docs/`, which the guide links to rather than inlines.
    pub docs_dir: Vec<String>,
}

impl Docs {
    /// Read the README and list `docs/` for a repository at `root`.
    ///
    /// A missing README is not an error: a library without one should still
    /// get a site from its manifest alone.
    pub fn load(root: &Path) -> Self {
        let readme = ["README.md", "README", "readme.md"]
            .iter()
            .map(|name| root.join(name))
            .find(|path| path.is_file())
            .and_then(|path| std::fs::read_to_string(path).ok());

        let mut docs = Self {
            lede: readme.as_deref().and_then(lede_of),
            sections: readme.as_deref().map(sectionize).unwrap_or_default(),
            docs_dir: std::fs::read_dir(root.join("docs"))
                .map(|entries| {
                    entries
                        .flatten()
                        .filter(|e| e.path().is_file())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|n| n.ends_with(".md"))
                        .collect()
                })
                .unwrap_or_default(),
        };
        docs.docs_dir.sort();
        docs
    }

    /// Sections whose title matches any of `wanted`, case-insensitively.
    pub fn sections_named(&self, wanted: &[&str]) -> Vec<&Section> {
        self.sections
            .iter()
            .filter(|section| wanted.iter().any(|w| section.title.eq_ignore_ascii_case(w)))
            .collect()
    }

    pub fn has_section(&self, title: &str) -> bool {
        self.sections
            .iter()
            .any(|s| s.title.eq_ignore_ascii_case(title))
    }
}

/// The first real prose paragraph: after the `# Title`, any badge or image
/// lines, and the table of contents a long README opens with.
fn lede_of(markdown: &str) -> Option<String> {
    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Skip badges, images, and HTML that a README opens with.
        if trimmed.starts_with("[!") || trimmed.starts_with("![") || trimmed.starts_with('<') {
            continue;
        }
        if trimmed.starts_with("```") {
            // skip a fenced block entirely
            for next in lines.by_ref() {
                if next.trim_start().starts_with("```") {
                    break;
                }
            }
            continue;
        }
        let mut lede = String::from(trimmed);
        // Absorb following prose lines, and stop at the next blank-separated
        // construct that is not prose.
        for next in lines.by_ref() {
            let t = next.trim();
            if t.is_empty() {
                break;
            }
            if t.starts_with('#') || t.starts_with("- ") || t.starts_with("```") {
                break;
            }
            lede.push(' ');
            lede.push_str(t);
        }
        if lede.len() > 40 {
            return Some(collapse(&lede));
        }
    }
    None
}

/// Split a document into `##`-level sections, keeping `###` inside its parent.
fn sectionize(markdown: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    let mut in_fence = false;

    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        let heading = if in_fence {
            None
        } else {
            // Count the leading hashes rather than stripping one: stripping a
            // single '#' from "## Install" leaves "# Install", which still
            // looks like a heading and has to be re-tested anyway.
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level == 0 || level > 6 {
                None
            } else {
                let title = trimmed[level..].trim();
                if title.is_empty() {
                    None
                } else {
                    Some((level as u8, title.to_string()))
                }
            }
        };

        match heading {
            Some((level, title)) if level >= 2 => {
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                current = Some(Section {
                    level,
                    title,
                    body: String::new(),
                });
            }
            Some(_) => {}
            None => {
                if let Some(section) = current.as_mut() {
                    section.body.push_str(trimmed);
                    section.body.push('\n');
                }
            }
        }
    }
    if let Some(section) = current {
        sections.push(section);
    }
    for section in &mut sections {
        section.body = section.body.trim().to_string();
    }
    sections.retain(|s| !s.body.is_empty());
    sections
}

fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render the markdown subset a README body needs into HTML.
///
/// Code fences become `<pre><code>` before escaping, so their contents are
/// shown verbatim rather than interpreted.
pub fn body_to_html(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    let mut fence_lang = String::new();
    let mut code = String::new();
    let mut prose = String::new();
    let mut table: Vec<String> = Vec::new();

    let flush_prose = |prose: &mut String, out: &mut String| {
        if !prose.trim().is_empty() {
            out.push_str("<p>");
            out.push_str(&inline(prose.trim()));
            out.push_str("</p>\n");
        }
        prose.clear();
    };

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            if in_fence {
                // A shell example is something a reader would type, so it
                // becomes a terminal rather than a code block. That is the
                // difference crux makes across fourteen commands on its CLI
                // page, and it is what makes a run look runnable.
                if is_shell(&fence_lang) {
                    out.push_str(&terminal_html(&code));
                } else {
                    out.push_str("<pre><code>");
                    out.push_str(&escape(&code));
                    out.push_str("</code></pre>\n");
                }
                code.clear();
                fence_lang.clear();
            } else {
                flush_prose(&mut prose, &mut out);
                fence_lang = line.trim_start().trim_start_matches('`').trim().to_string();
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            code.push_str(line);
            code.push('\n');
            continue;
        }

        // A pipe table is a block, not a paragraph, and `<table>` nested in
        // `<p>` is not valid HTML — so a table row has to break the paragraph
        // open, and the rows after it collect until the table ends.
        if is_table_row(line) {
            flush_prose(&mut prose, &mut out);
            table.push(line.to_string());
            continue;
        }
        if !table.is_empty() {
            out.push_str(&render_table(&table));
            table.clear();
        }

        prose.push_str(line);
        prose.push('\n');
    }

    if !table.is_empty() {
        out.push_str(&render_table(&table));
    }
    flush_prose(&mut prose, &mut out);

    // A trailing unterminated fence would otherwise be dropped silently.
    if in_fence && !code.trim().is_empty() {
        if is_shell(&fence_lang) {
            out.push_str(&terminal_html(&code));
        } else {
            out.push_str("<pre><code>");
            out.push_str(&escape(&code));
            out.push_str("</code></pre>\n");
        }
    }
    out
}

/// Fence languages that describe running something rather than showing source.
fn is_shell(lang: &str) -> bool {
    matches!(
        lang.trim().to_ascii_lowercase().as_str(),
        "bash" | "sh" | "shell" | "zsh" | "console" | "nu" | "nushell" | ""
    )
}

/// Render a shell example as a terminal component.
///
/// The label is the first command in the example, which is the thing a reader
/// is looking for when they scan the page; the body is the example with the
/// prompt and the comments marked up the way a captured session would look.
fn terminal_html(code: &str) -> String {
    let label = command_label(code);
    let mut body = String::new();
    let mut prompted = false;

    for line in code.trim_end().lines() {
        let trimmed = line.trim();
        if !prompted
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('$')
        {
            body.push_str("<span class=\"prompt\">$</span> ");
            prompted = true;
        } else if !prompted && trimmed.starts_with('$') {
            // Already carries its own prompt.
            prompted = true;
            body.push_str(&escape(line));
            body.push('\n');
            continue;
        }
        if trimmed.starts_with('#') {
            body.push_str("<span class=\"cmt\">");
            body.push_str(&escape(line));
            body.push_str("</span>\n");
            continue;
        }
        body.push_str(&escape(line));
        body.push('\n');
    }

    format!(
        "<div class=\"terminal\">\n  <div class=\"terminal-bar\"><span class=\"label\">{}</span></div>\n  <pre>{}</pre>\n</div>\n",
        escape(&label),
        body
    )
}

/// The first runnable command in a shell example, for the terminal's label.
///
/// Continuation lines are folded in, because a command broken across two
/// lines with a trailing backslash is one command and the label saying
/// otherwise would be a small lie.
fn command_label(code: &str) -> String {
    let mut label = String::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('$') {
            // A prompt: the command follows it.
            let command = trimmed.trim_start_matches('$').trim();
            if !command.is_empty() {
                label = command.to_string();
            }
            continue;
        }
        if label.is_empty() {
            label = trimmed.trim_start_matches('$').trim().to_string();
        } else {
            label.push(' ');
            label.push_str(trimmed.trim_start_matches('$').trim());
        }
        if !label.ends_with('\\') {
            break;
        }
        label.pop();
        label.pop();
    }
    if label.is_empty() {
        return "shell".to_string();
    }
    // Long invocations become unreadable as a label; the body still has them.
    if label.chars().count() > 68 {
        let mut short: String = label.chars().take(65).collect();
        short.push_str("...");
        return short;
    }
    label
}

/// A pipe-table row: starts with `|`, or has a `|` with dashes around it.
fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.matches('|').count() >= 2
}

/// The `|---|---|` row that marks a table's column headings.
fn is_table_separator(line: &str) -> bool {
    let t: String = line.trim().chars().filter(|c| !c.is_whitespace()).collect();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| c == '|' || c == '-' || c == ':')
}

/// Render a collected run of pipe rows as an HTML table.
///
/// Alignment is read from the separator row and applied as inline styles:
/// the shared layer has no alignment utility, and a GitHub-style
/// `:---:` means something worth keeping.
fn render_table(rows: &[String]) -> String {
    let mut out = String::from("<div class=\"table-wrap\">\n<table>\n");
    let mut index = 0;

    // Header, if the second row is a separator.
    let has_header = rows.len() > 1 && is_table_separator(&rows[1]);
    if has_header {
        out.push_str("<thead>\n<tr>");
        for cell in split_row(&rows[0]) {
            out.push_str(&format!("<th>{}</th>", inline(&cell)));
        }
        out.push_str("</tr>\n</thead>\n<tbody>\n");
        index = 2;
    } else {
        out.push_str("<tbody>\n");
    }

    for row in rows.iter().skip(index) {
        if is_table_separator(row) {
            continue;
        }
        out.push_str("<tr>");
        for cell in split_row(row) {
            out.push_str(&format!("<td>{}</td>", inline(&cell)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n</div>\n");
    out
}

fn split_row(row: &str) -> Vec<String> {
    row.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

/// Inline code spans, after escaping everything else.
fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        out.push_str(&escape(&rest[..start]));
        match rest[start + 1..].find('`') {
            Some(end) => {
                out.push_str("<code>");
                out.push_str(&escape(&rest[start + 1..start + 1 + end]));
                out.push_str("</code>");
                rest = &rest[start + end + 2..];
            }
            None => {
                out.push_str(&escape(&rest[start..]));
                return out;
            }
        }
    }
    out.push_str(&escape(rest));
    out
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = r#"
# kiln

[![CI](https://example.com/badge.svg)](https://example.com)

A CLI for firing things, with previews and an undo.

## Install

```bash
cargo install kiln
```

Run it with `kiln fire`.

## Safety

Never overwrites. See the [guide](docs/guide.md).
"#;

    #[test]
    fn lede_skips_title_and_badges() {
        let lede = lede_of(README).expect("a lede");
        assert!(lede.starts_with("A CLI for firing things"), "{lede}");
        assert!(!lede.contains("badge"), "{lede}");
        assert!(!lede.contains("kiln\n"), "{lede}");
    }

    #[test]
    fn lede_is_none_without_prose() {
        assert_eq!(lede_of("# Title\n\n## Install\n"), None);
    }

    #[test]
    fn sections_split_on_h2_and_keep_bodies() {
        let sections = sectionize(README);
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["Install", "Safety"]);
        assert!(sections[0].body.contains("cargo install kiln"));
        assert!(sections[1].body.contains("Never overwrites"));
    }

    #[test]
    fn headings_inside_fences_are_not_sections() {
        let md = "# t\n\n## Real\n\n```sh\n## not a heading\n```\n\n## Also real\n\ntext\n";
        let sections = sectionize(md);
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["Real", "Also real"]);
    }

    #[test]
    fn body_renders_fences_and_inline_code() {
        // A non-shell fence stays a code block; shell fences become terminals,
        // which shell_examples_become_terminal_components covers.
        let html = body_to_html("Run `kiln fire` now.\n\n```toml\nkey = \"<value>\"\n```\n");
        assert!(html.contains("<code>kiln fire</code>"), "{html}");
        assert!(html.contains("<pre><code>"), "{html}");
        assert!(html.contains("key = &quot;&lt;value&gt;&quot;"), "{html}");
        assert!(html.contains("<p>"), "{html}");
    }

    #[test]
    fn code_fence_contents_are_not_escaped_as_markup() {
        // A `<` inside a fence must survive as literal text.
        let html = body_to_html("```\na < b && c > d\n```\n");
        assert!(html.contains("a &lt; b &amp;&amp; c &gt; d"), "{html}");
    }

    #[test]
    fn unterminated_fence_is_still_emitted() {
        let html = body_to_html("```\nunclosed\n");
        assert!(html.contains("unclosed"), "{html}");
    }

    #[test]
    fn escapes_markup_in_prose() {
        let html = body_to_html("A <script>alert(1)</script> tag.\n");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(!html.contains("<script>"), "{html}");
    }

    #[test]
    fn tables_render_as_real_tables() {
        let html = body_to_html("| Tool | Used for |\n| --- | --- |\n| `pandoc` | Conversion |\n");
        assert!(html.contains("<table>"), "{html}");
        assert!(html.contains("<th>Tool</th>"), "{html}");
        assert!(html.contains("<td><code>pandoc</code></td>"), "{html}");
        assert!(
            !html.contains("| ---"),
            "separator must not survive: {html}"
        );
    }

    #[test]
    fn a_table_is_never_nested_inside_a_paragraph() {
        let html = body_to_html("Intro line.\n\n| a | b |\n| - | - |\n| 1 | 2 |\n");
        assert!(!html.contains("<p>.*<table"), "{html}");
        // the paragraph closed before the table opened
        let p_end = html.find("</p>").expect("a paragraph");
        let t_start = html.find("<table").expect("a table");
        assert!(p_end < t_start, "table opened inside the paragraph: {html}");
    }

    #[test]
    fn table_without_a_header_row_has_no_thead() {
        let html = body_to_html("| 1 | 2 |\n| 3 | 4 |\n");
        assert!(html.contains("<table>"), "{html}");
        assert!(!html.contains("<thead>"), "{html}");
        assert!(html.contains("<td>1</td>"), "{html}");
    }

    #[test]
    fn fences_still_win_over_pipe_rows() {
        // A pipe line inside a fence is code, not a table.
        let html = body_to_html("```\n| not | a table |\n| --- | --- |\n```\n");
        assert!(!html.contains("<table"), "{html}");
        assert!(html.contains("| not | a table |"), "{html}");
    }

    #[test]
    fn shell_examples_become_terminal_components() {
        let html = body_to_html("Run it:\n\n```bash\ncrux list .\n```\n");
        assert!(html.contains("class=\"terminal\""), "{html}");
        assert!(
            html.contains("<span class=\"label\">crux list .</span>"),
            "{html}"
        );
        assert!(html.contains("<span class=\"prompt\">$</span>"), "{html}");
        assert!(!html.contains("<pre><code>"), "{html}");
    }

    #[test]
    fn non_shell_fences_stay_code_blocks() {
        let html = body_to_html("```rust\nfn main() {}\n```\n");
        assert!(html.contains("<pre><code>"), "{html}");
        assert!(!html.contains("class=\"terminal\""), "{html}");
    }

    #[test]
    fn every_shell_dialect_is_a_terminal() {
        for lang in ["bash", "sh", "shell", "zsh", "nu", "console", ""] {
            let html = body_to_html(&format!("```{lang}\ncmd --flag\n```\n"));
            assert!(html.contains("class=\"terminal\""), "lang {lang}: {html}");
        }
    }

    #[test]
    fn terminal_label_folds_continuations() {
        let label = command_label("crux run a.crux \\\n  b.json --dry-run\n");
        assert_eq!(label, "crux run a.crux b.json --dry-run");
    }

    #[test]
    fn terminal_label_skips_comments_and_finds_the_command() {
        let label = command_label("# install first\n# then run\ncargo install kiln\n");
        assert_eq!(label, "cargo install kiln");
    }

    #[test]
    fn terminal_label_accepts_an_existing_prompt() {
        assert_eq!(
            command_label("$ hj handoff --summary x\n"),
            "hj handoff --summary x"
        );
    }

    #[test]
    fn very_long_commands_are_truncated_in_the_label() {
        let long = "cargo run --release --bin thing -- --with-a-flag --and-another --and-more";
        let label = command_label(long);
        assert!(label.ends_with("..."), "{label}");
        assert!(label.chars().count() <= 68, "{}", label.len());
    }

    #[test]
    fn an_all_comment_example_still_gets_a_label() {
        assert_eq!(command_label("# just a comment\n"), "shell");
    }

    #[test]
    fn docs_load_tolerates_a_missing_readme() {
        let tmp = std::env::temp_dir().join("docs-no-readme-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp");
        let docs = Docs::load(&tmp);
        assert!(docs.lede.is_none());
        assert!(docs.sections.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
