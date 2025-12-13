use anyhow::Result;

use crate::trellis::types::Page;

pub trait Transformer: Send + Sync {
    fn transform(&self, page: Page) -> Result<Page>;
}

pub trait Filter: Send + Sync {
    fn include(&self, page: &Page) -> bool;
}

pub trait Emitter: Send + Sync {
    fn emit(&self, _page: &Page) -> Result<()> {
        Ok(())
    }
}
