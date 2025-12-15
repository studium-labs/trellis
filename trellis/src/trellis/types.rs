use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Default)]
pub struct PageMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Page {
    pub slug: String,
    pub source_path: PathBuf,
    pub frontmatter: PageMetadata,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

impl Page {
    pub fn new(slug: String, source_path: PathBuf, content: String) -> Self {
        Self {
            slug,
            source_path,
            frontmatter: PageMetadata::default(),
            content,
            html: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedPage {
    pub slug: String,
    pub html: String,
    pub frontmatter: PageMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
}

impl From<Page> for RenderedPage {
    fn from(mut page: Page) -> Self {
        let html = page.html.take().unwrap_or_else(|| String::new());

        RenderedPage {
            slug: page.slug,
            html,
            frontmatter: page.frontmatter,
            cached: None,
        }
    }
}

pub fn slug_from_path(path: &Path, content_root: &Path) -> String {
    path.strip_prefix(content_root)
        .ok()
        .and_then(|p| p.with_extension("").to_str().map(|s| s.replace('\\', "/")))
        .unwrap_or_else(|| "index".to_string())
}
