//! Markdown from the repo -> the in-app help. One source: the files below are rendered at
//! build time into `apps/desktop/src/lib/docs/content.json` (generated, committed, checked
//! by `check --ci`), so the app always carries the docs of its own version and works offline.
//!
//! Two groups: `hilfe` (for the gang) and `projekt` (status, changelog, ADRs, spikes …).
//! `docs/hilfe/glossar.md` is also the glossary: every `##` heading is a term, the first
//! paragraph its tooltip text (see `Info.svelte`).
//!
//! Links between the files become `doc:<id>#<anchor>` and the UI navigates inside the help;
//! other relative links become `repo:<path>` (shown, not clickable); http(s) links open in
//! the system browser.

use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const OUT: &str = "apps/desktop/src/lib/docs/content.json";
const GLOSSARY: &str = "docs/hilfe/glossar.md";

/// (group, repo path, title override — `None` = the file's H1). Order = order in the help
/// navigation. A path ending in `/` lists the folder (README first), so a new ADR or spike
/// needs no change here.
const PAGES: &[(&str, &str, Option<&str>)] = &[
    ("hilfe", "docs/mittesten.md", None),
    ("projekt", "docs/testabend.md", None),
    ("hilfe", "docs/hilfe/so-funktioniert-es.md", None),
    ("hilfe", GLOSSARY, None),
    ("projekt", "TASKS.md", Some("Status")),
    ("projekt", "CHANGELOG.md", None),
    ("projekt", "README.md", Some("Überblick")),
    ("projekt", "docs/01-vision-v1.md", None),
    ("projekt", "docs/00-grobstruktur.md", None),
    ("projekt", "docs/ui/", None),
    ("projekt", "docs/adr/", None),
    ("projekt", "docs/spikes/", None),
    ("projekt", "docs/dev-setup.md", None),
    ("projekt", "docs/homeserver.md", None),
    ("projekt", "CLAUDE.md", Some("Arbeitsregeln (Claude Code)")),
];

#[derive(Serialize)]
struct Content {
    generated: &'static str,
    pages: Vec<Page>,
    glossary: BTreeMap<String, Entry>,
}

#[derive(Serialize)]
struct Page {
    id: String,
    group: String,
    title: String,
    source: String,
    html: String,
    /// Plain text for the search box.
    text: String,
}

#[derive(Serialize)]
struct Entry {
    term: String,
    short: String,
    html: String,
}

pub fn generate(root: &Path) -> Result<(), String> {
    let mut sources: Vec<(&str, String, Option<&str>)> = Vec::new();
    for (group, path, title) in PAGES {
        if let Some(dir) = path.strip_suffix('/') {
            let mut files: Vec<String> = fs::read_dir(root.join(dir))
                .map_err(|e| format!("{dir}: {e}"))?
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".md"))
                .collect();
            files.sort_by_key(|name| (name != "README.md", name.clone()));
            for name in files {
                sources.push((group, format!("{dir}/{name}"), *title));
            }
        } else {
            sources.push((group, (*path).to_string(), *title));
        }
    }

    let ids: Vec<(String, String)> = sources
        .iter()
        .map(|(_, path, _)| (path.clone(), page_id(path)))
        .collect();

    let mut pages = Vec::new();
    for (group, path, title) in &sources {
        let markdown = fs::read_to_string(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        let rendered = render(&markdown, path, &ids);
        let title = title
            .map(str::to_string)
            .or(rendered.title)
            .unwrap_or_else(|| path.clone());
        pages.push(Page {
            id: page_id(path),
            group: (*group).to_string(),
            title,
            source: path.clone(),
            html: rendered.html,
            text: rendered.text,
        });
    }

    let glossary_md =
        fs::read_to_string(root.join(GLOSSARY)).map_err(|e| format!("{GLOSSARY}: {e}"))?;
    let glossary = glossary(&glossary_md, &ids);

    let content = Content {
        generated: "by `cargo xtask docs` from docs/, TASKS.md, CHANGELOG.md — do not edit",
        pages,
        glossary,
    };
    let mut json = serde_json::to_string_pretty(&content).map_err(|e| e.to_string())?;
    json.push('\n');

    let out = root.join(OUT);
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{OUT}: {e}"))?;
    }
    if fs::read_to_string(&out).ok().as_deref() != Some(json.as_str()) {
        fs::write(&out, json).map_err(|e| format!("{OUT}: {e}"))?;
        println!("wrote {OUT}");
    }
    Ok(())
}

/// `docs/adr/001-variante-b-stack.md` -> `adr/001-variante-b-stack`, `TASKS.md` -> `tasks`.
fn page_id(path: &str) -> String {
    path.strip_prefix("docs/")
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_lowercase()
}

struct Rendered {
    title: Option<String>,
    html: String,
    text: String,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
}

/// Markdown -> HTML with heading ids, rewritten links, the H1 as title and the plain text.
fn render(markdown: &str, source: &str, ids: &[(String, String)]) -> Rendered {
    let mut events: Vec<Event> = Parser::new_ext(markdown, options()).collect();
    let mut title = None;
    let mut text = String::new();
    let mut seen_ids: Vec<String> = Vec::new();

    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level;
                let heading = plain_text(&events[i + 1..], &TagEnd::Heading(level));
                if title.is_none() && level == pulldown_cmark::HeadingLevel::H1 {
                    title = Some(heading.clone());
                }
                let mut id = slug(&heading);
                let mut n = 1;
                while seen_ids.contains(&id) {
                    n += 1;
                    id = format!("{}-{n}", slug(&heading));
                }
                seen_ids.push(id.clone());
                if let Event::Start(Tag::Heading { id: slot, .. }) = &mut events[i] {
                    *slot = Some(CowStr::from(id));
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let rewritten = rewrite_link(dest_url, source, ids);
                if let Event::Start(Tag::Link { dest_url, .. }) = &mut events[i] {
                    *dest_url = CowStr::from(rewritten);
                }
            }
            // The app carries no image files, so a picture (concept screens in docs/ui)
            // becomes a `repo:` link with its alt text — or the file name when there is none.
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title: img_title,
                id,
            }) => {
                let rewritten = rewrite_link(dest_url, source, ids);
                let file_name = dest_url.rsplit('/').next().unwrap_or(dest_url).to_string();
                events[i] = Event::Start(Tag::Link {
                    link_type: *link_type,
                    dest_url: CowStr::from(rewritten),
                    title: img_title.clone(),
                    id: id.clone(),
                });
                if matches!(events.get(i + 1), Some(Event::End(TagEnd::Image))) {
                    events.insert(i + 1, Event::Text(CowStr::from(file_name)));
                }
            }
            Event::End(TagEnd::Image) => events[i] = Event::End(TagEnd::Link),
            Event::Text(t) | Event::Code(t) => text.push_str(t),
            // Word boundaries only at breaks and block ends — inline ends (emphasis, links,
            // code) must not split "*Laufzeit*." into "Laufzeit .".
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::End(end) if !inline_end(end) => text.push(' '),
            _ => {}
        }
        i += 1;
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    Rendered {
        title,
        html: out,
        text: text.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

fn inline_end(end: &TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link | TagEnd::Image
    )
}

/// Text of the events up to the given end tag (exclusive).
fn plain_text(events: &[Event], end: &TagEnd) -> String {
    let mut s = String::new();
    for event in events {
        match event {
            Event::End(e) if e == end => break,
            Event::Text(t) | Event::Code(t) => s.push_str(t),
            Event::SoftBreak | Event::HardBreak => s.push(' '),
            _ => {}
        }
    }
    s.trim().to_string()
}

/// Relative `.md` links -> `doc:<id>[#anchor]`; other relative links -> `repo:<path>`.
fn rewrite_link(dest: &str, source: &str, ids: &[(String, String)]) -> String {
    let absolute = dest.contains("://") || dest.starts_with("mailto:") || dest.starts_with('#');
    if absolute || dest.starts_with('/') {
        return dest.to_string();
    }
    let (path, anchor) = match dest.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (dest, None),
    };
    let base = source.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let resolved = normalize(&format!("{base}/{path}"));
    if let Some((_, id)) = ids.iter().find(|(p, _)| *p == resolved) {
        match anchor {
            Some(a) => format!("doc:{id}#{a}"),
            None => format!("doc:{id}"),
        }
    } else {
        format!("repo:{resolved}")
    }
}

/// `docs/adr/../00-grobstruktur.md` -> `docs/00-grobstruktur.md`.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// Heading text -> anchor: ASCII, lowercase, hyphens; umlauts transliterated so the ids
/// are stable in URLs and in `term="…"` attributes.
pub fn slug(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    let mut dash = true;
    for c in text.chars() {
        let mapped: &str = match c {
            'ä' | 'Ä' => "ae",
            'ö' | 'Ö' => "oe",
            'ü' | 'Ü' => "ue",
            'ß' => "ss",
            c if c.is_ascii_alphanumeric() => {
                s.push(c.to_ascii_lowercase());
                dash = false;
                continue;
            }
            _ => "",
        };
        if mapped.is_empty() {
            if !dash {
                s.push('-');
                dash = true;
            }
        } else {
            s.push_str(mapped);
            dash = false;
        }
    }
    s.trim_end_matches('-').to_string()
}

/// The glossary file split at its `##` headings: slug -> term, tooltip text, full entry.
fn glossary(markdown: &str, ids: &[(String, String)]) -> BTreeMap<String, Entry> {
    let mut entries = BTreeMap::new();
    for section in markdown.split("\n## ").skip(1) {
        let (heading, body) = section.split_once('\n').unwrap_or((section, ""));
        let term = heading.trim().to_string();
        let body = body.trim();
        let first = body.split("\n\n").next().unwrap_or("");
        let short = render(first, GLOSSARY, ids).text;
        let html = render(body, GLOSSARY, ids).html;
        entries.insert(slug(&term), Entry { term, short, html });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<(String, String)> {
        ["docs/00-grobstruktur.md", "docs/adr/README.md", "TASKS.md"]
            .iter()
            .map(|p| (p.to_string(), page_id(p)))
            .collect()
    }

    #[test]
    fn page_ids_drop_docs_prefix_and_extension() {
        assert_eq!(
            page_id("docs/adr/001-variante-b-stack.md"),
            "adr/001-variante-b-stack"
        );
        assert_eq!(page_id("TASKS.md"), "tasks");
    }

    #[test]
    fn slugs_are_ascii_and_stable() {
        assert_eq!(
            slug("Wenn etwas komisch klingt"),
            "wenn-etwas-komisch-klingt"
        );
        assert_eq!(
            slug("Rauschunterdrückung (RNNoise)"),
            "rauschunterdrueckung-rnnoise"
        );
        assert_eq!(slug("  48 kHz / Abtastrate  "), "48-khz-abtastrate");
    }

    #[test]
    fn headings_get_ids_and_the_first_h1_is_the_title() {
        let r = render(
            "# Hallo Welt\n\n## Abschnitt\n\n## Abschnitt\n",
            "docs/x.md",
            &ids(),
        );
        assert_eq!(r.title.as_deref(), Some("Hallo Welt"));
        assert!(r.html.contains("<h1 id=\"hallo-welt\">"));
        assert!(r.html.contains("<h2 id=\"abschnitt\">"));
        assert!(r.html.contains("<h2 id=\"abschnitt-2\">"));
        assert_eq!(r.text, "Hallo Welt Abschnitt Abschnitt");
    }

    #[test]
    fn links_between_pages_become_doc_links() {
        let ids = ids();
        assert_eq!(
            rewrite_link("../00-grobstruktur.md#ziele", "docs/adr/README.md", &ids),
            "doc:00-grobstruktur#ziele"
        );
        assert_eq!(rewrite_link("TASKS.md", "README.md", &ids), "doc:tasks");
        assert_eq!(
            rewrite_link("adr/README.md", "docs/dev-setup.md", &ids),
            "doc:adr/readme"
        );
        assert_eq!(
            rewrite_link(".claude/settings.json", "README.md", &ids),
            "repo:.claude/settings.json"
        );
        assert_eq!(
            rewrite_link("https://x.y/z", "README.md", &ids),
            "https://x.y/z"
        );
        assert_eq!(rewrite_link("#ziele", "README.md", &ids), "#ziele");
    }

    #[test]
    fn images_become_repo_links_with_alt_text_or_file_name() {
        let r = render(
            "![Shell mit Zonen](img/00-shell.png) und ![](img/01-call.png)",
            "docs/ui/01-konzept.md",
            &ids(),
        );
        assert!(r
            .html
            .contains("<a href=\"repo:docs/ui/img/00-shell.png\">Shell mit Zonen</a>"));
        assert!(r
            .html
            .contains("<a href=\"repo:docs/ui/img/01-call.png\">01-call.png</a>"));
        assert!(!r.html.contains("<img"));
    }

    #[test]
    fn glossary_entries_take_the_first_paragraph_as_tooltip() {
        let md = "# Glossar\n\nIntro.\n\n## Jitter\n\nSchwankung der *Laufzeit*.\n\nMehr: [Wikipedia](https://de.wikipedia.org/wiki/Jitter)\n\n## Ersetzte Samples\n\nText.\n";
        let g = glossary(md, &ids());
        let jitter = &g["jitter"];
        assert_eq!(jitter.term, "Jitter");
        assert_eq!(jitter.short, "Schwankung der Laufzeit.");
        assert!(jitter
            .html
            .contains("href=\"https://de.wikipedia.org/wiki/Jitter\""));
        assert_eq!(g["ersetzte-samples"].short, "Text.");
    }
}
