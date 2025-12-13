mod callouts;
mod encryption;
mod frontmatter;
mod markdown;
mod mermaid;
pub mod traits;

use anyhow::Result;

use crate::trellis::types::Page;

use self::encryption::EncryptContent;
use self::frontmatter::FrontMatter;
use self::markdown::MarkdownRenderer;
use self::traits::{Filter, Transformer};

pub struct PluginRegistry {
    transformers: Vec<Box<dyn Transformer>>,
    filters: Vec<Box<dyn Filter>>,
}

impl PluginRegistry {
    pub fn bare_minimum() -> Self {
        Self {
            transformers: vec![
                Box::new(FrontMatter),
                Box::new(MarkdownRenderer),
                Box::new(EncryptContent),
            ],
            filters: vec![],
        }
    }

    pub fn with_filters(mut self, filters: Vec<Box<dyn Filter>>) -> Self {
        self.filters = filters;
        self
    }

    pub fn transform(&self, mut page: Page) -> Result<Page> {
        for transformer in &self.transformers {
            page = transformer.transform(page)?;
        }
        Ok(page)
    }

    pub fn allow(&self, page: &Page) -> bool {
        self.filters.iter().all(|f| f.include(page))
    }
}

pub use frontmatter::DraftFilter;
