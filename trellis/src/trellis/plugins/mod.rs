pub mod callouts;
pub mod encryption;
pub mod frontmatter;
pub mod markdown;
pub mod mermaid;
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
                // Order matters: FrontMatter must run first so filters can see metadata
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

    /// Run transformers in order while honoring filters.
    ///
    /// Filters are evaluated after the first transformer (FrontMatter) has
    /// populated metadata/frontmatter so they can read flags like `draft`.
    /// Returns `Ok(None)` when a filter excludes the page.
    pub fn transform(&self, mut page: Page) -> Result<Option<Page>> {
        // Run the first transformer (expected to be FrontMatter) before filters
        if let Some((first, rest)) = self.transformers.split_first() {
            page = first.transform(page)?;

            if !self.allow(&page) {
                return Ok(None);
            }

            for transformer in rest {
                page = transformer.transform(page)?;
            }
        }

        Ok(Some(page))
    }

    pub fn allow(&self, page: &Page) -> bool {
        self.filters.iter().all(|f| f.include(page))
    }
}

pub use frontmatter::DraftFilter;
