use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use log::debug;
use serde::Serialize;
use walkdir::WalkDir;

use crate::trellis::plugins::frontmatter::FrontMatter;
use crate::trellis::plugins::traits::Transformer;
use crate::trellis::types::{Page, slug_from_path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentIndexEntry {
    slug: String,
    file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

pub fn generate_content_index(
    content_root: &Path,
    cache_root: &Path,
    ignore_patterns: &[String],
) -> Result<()> {
    let mut entries: BTreeMap<String, ContentIndexEntry> = BTreeMap::new();

    for entry in WalkDir::new(content_root)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), content_root, ignore_patterns))
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        if entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            != Some(true)
        {
            continue;
        }

        let slug = slug_from_path(entry.path(), content_root);
        let file_path = entry
            .path()
            .strip_prefix(content_root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        let content = fs::read_to_string(entry.path()).with_context(|| {
            format!(
                "reading markdown for content index at {}",
                entry.path().display()
            )
        })?;

        let mut page = Page::new(slug.clone(), entry.path().to_path_buf(), content);
        // Reuse frontmatter parsing to extract title/tags.
        page = FrontMatter
            .transform(page)
            .context("parsing frontmatter for content index")?;

        let title = page.frontmatter.title.clone().or_else(|| {
            // fallback: use last segment of slug
            slug.rsplit('/').next().map(|s| s.replace('-', " "))
        });

        let tags = page.frontmatter.tags.clone();

        // Minimal link extraction (wikilinks + markdown links) – best-effort.
        let links = extract_links(&page.content);

        entries.insert(
            slug.clone(),
            ContentIndexEntry {
                slug,
                file_path,
                title,
                links: if links.is_empty() { None } else { Some(links) },
                tags,
            },
        );
    }

    let static_dir = cache_root.join("static");
    fs::create_dir_all(&static_dir)
        .with_context(|| format!("creating static dir at {}", static_dir.display()))?;

    let json_path = static_dir.join("content-index.json");
    let json = serde_json::to_string(&entries)?;
    fs::write(&json_path, json)
        .with_context(|| format!("writing content index to {}", json_path.display()))?;

    debug!("content-index.json written to {}", json_path.display());
    Ok(())
}

fn is_ignored(path: &Path, root: &Path, patterns: &[String]) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };

    rel.components().any(|comp| {
        comp.as_os_str()
            .to_str()
            .map(|s| patterns.iter().any(|p| p == s))
            .unwrap_or(false)
    })
}

fn extract_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();

    // [[wikilink]]
    let wiki = regex::Regex::new(r"\[\[([^\]\|#]+)").ok();
    if let Some(re) = wiki {
        for cap in re.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                links.push(clean_link_target(m.as_str()));
            }
        }
    }

    // markdown links [text](target)
    let md = regex::Regex::new(r"\[[^\]]*\]\(([^)]+)\)").ok();
    if let Some(re) = md {
        for cap in re.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                let target = m.as_str();
                if !(target.starts_with("http://") || target.starts_with("https://")) {
                    links.push(clean_link_target(target));
                }
            }
        }
    }

    links.sort();
    links.dedup();
    links
}

fn clean_link_target(raw: &str) -> String {
    let no_anchor = raw.split('#').next().unwrap_or(raw);
    let trimmed = no_anchor.trim().trim_matches('.');
    let mut cleaned = trimmed.trim_matches('/').to_string();

    if cleaned.ends_with(".md") {
        cleaned.truncate(cleaned.len() - 3);
    } else if cleaned.ends_with(".html") {
        cleaned.truncate(cleaned.len() - 5);
    }

    if cleaned.ends_with("/index") {
        cleaned.truncate(cleaned.len() - "/index".len());
    }

    if cleaned.is_empty() {
        ".".into()
    } else {
        cleaned
    }
}
