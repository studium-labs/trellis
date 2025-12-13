use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::trellis::types::{Page, PageMetadata};

use super::traits::{Filter, Transformer};

pub struct FrontMatter;

impl Transformer for FrontMatter {
    fn transform(&self, mut page: Page) -> Result<Page> {
        let content = page.content.clone();
        let mut lines = content.lines();

        let first_line = lines.next();
        if first_line != Some("---") {
            return Ok(page);
        }

        let mut fm_lines = vec![];
        for line in lines.by_ref() {
            if line.trim() == "---" {
                break;
            }
            fm_lines.push(line);
        }

        let remainder: String = lines.collect::<Vec<&str>>().join("\n");
        let yaml_str = fm_lines.join("\n");

        if !yaml_str.is_empty() {
            let parsed: HashMap<String, serde_yaml::Value> =
                serde_yaml::from_str(&yaml_str).context("parsing frontmatter")?;
            let mut meta = PageMetadata::default();

            if let Some(serde_yaml::Value::String(title)) = parsed.get("title") {
                meta.title = Some(title.clone());
            }
            if let Some(serde_yaml::Value::String(desc)) = parsed.get("description") {
                meta.description = Some(desc.clone());
            }
            if let Some(created) = parsed.get("created").and_then(as_datetime) {
                meta.created = Some(created);
            }
            if let Some(updated) = parsed.get("updated").and_then(as_datetime) {
                meta.updated = Some(updated);
            }
            if let Some(tags) = parsed.get("tags").and_then(as_string_list) {
                meta.tags = Some(tags);
            }

            page.meta = meta;
            page.frontmatter = parsed;
        }

        page.content = remainder;
        Ok(page)
    }
}

fn as_datetime(value: &serde_yaml::Value) -> Option<DateTime<Utc>> {
    match value {
        serde_yaml::Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .ok(),
        _ => None,
    }
}

fn as_string_list(value: &serde_yaml::Value) -> Option<Vec<String>> {
    match value {
        serde_yaml::Value::Sequence(seq) => Some(
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
        ),
        _ => None,
    }
}

pub struct DraftFilter;

impl Filter for DraftFilter {
    fn include(&self, page: &Page) -> bool {
        page.frontmatter
            .get("draft")
            .and_then(|v| v.as_bool())
            .map(|draft| !draft)
            .unwrap_or(true)
    }
}
