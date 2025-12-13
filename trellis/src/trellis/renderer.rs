use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::trellis::cache;
use crate::trellis::config::SiteConfig;
use crate::trellis::layout::{
    default_content_page_layout, default_list_page_layout, shared_layout,
};
use crate::trellis::plugins::{DraftFilter, PluginRegistry};
use crate::trellis::types::{Page, RenderedPage, slug_from_path};

pub struct RootzEngine {
    pub config: SiteConfig,
    pub shared_layout: crate::trellis::layout::SharedLayout,
    pub content_layout: crate::trellis::layout::PageLayout,
    pub list_layout: crate::trellis::layout::PageLayout,
    registry: PluginRegistry,
    content_root: PathBuf,
    cache_root: PathBuf,
}

impl RootzEngine {
    pub fn new(config: SiteConfig) -> Result<Self> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let content_root = manifest.join("../content");
        let cache_root = manifest.join("../.build");

        cache::ensure_cache_root(&cache_root)?;

        let shared = shared_layout(&config);
        let content_layout = default_content_page_layout();
        let list_layout = default_list_page_layout();

        Ok(Self {
            config,
            shared_layout: shared,
            content_layout,
            list_layout,
            registry: PluginRegistry::bare_minimum().with_filters(vec![Box::new(DraftFilter)]),
            content_root,
            cache_root,
        })
    }

    pub fn render_page(&self, slug: &str) -> Result<RenderedPage> {
        let source_path = self.source_path_for(slug);
        let cache_path = cache::cache_path(&self.cache_root, slug);
        let styles_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../quartz/quartz/styles");
        let styles_mtime = cache::newest_mtime_with_extension(&styles_root, "scss")
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let binary_mtime = cache::binary_mtime();

        let use_cache = source_path.exists()
            && cache_path.exists()
            && cache::cache_is_fresh(&source_path, &cache_path, &[styles_mtime, binary_mtime])?;

        let mut page = self.load_page(slug, &source_path)?;

        // Always parse frontmatter (and other metadata) even when reusing cached HTML.
        // We still run the transformer pipeline to populate PageMetadata/frontmatter.
        // If we are using the cache, we overwrite the freshly-rendered HTML with the cached HTML.
        page = self.registry.transform(page)?;

        if use_cache {
            page.html = Some(fs::read_to_string(&cache_path)?);
        }

        let rendered: RenderedPage = page.clone().into();

        if !use_cache {
            cache::write_cache(&cache_path, &rendered.html)?;
        }

        let mut rendered = rendered;
        rendered.cached = Some(use_cache);
        Ok(rendered)
    }

    fn source_path_for(&self, slug: &str) -> PathBuf {
        let mut path = self.content_root.join(slug);
        if path.extension().is_none() {
            path = path.with_extension("md");
        }
        path
    }

    fn load_page(&self, slug: &str, path: &Path) -> Result<Page> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("reading markdown at {}", path.display()))?;
            Ok(Page::new(slug.to_string(), path.to_path_buf(), content))
        } else {
            Ok(Page::new(slug.to_string(), path.to_path_buf(), "".into()))
        }
    }

    /// Pre-render all markdown files under the content root into cache, returning slugs.
    pub fn prebuild_all(&self) -> Result<Vec<String>> {
        let mut slugs = vec![];
        for entry in WalkDir::new(&self.content_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            if entry
                .path()
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
            {
                let slug = slug_from_path(entry.path(), &self.content_root);
                let _ = self.render_page(&slug)?;
                slugs.push(slug);
            }
        }
        Ok(slugs)
    }

    /// List cached slugs currently present in the build directory.
    pub fn cached_slugs(&self) -> Result<Vec<String>> {
        let mut slugs = vec![];
        for entry in WalkDir::new(&self.cache_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            if entry
                .path()
                .extension()
                .map(|ext| ext == "html")
                .unwrap_or(false)
            {
                if let Ok(rel) = entry.path().strip_prefix(&self.cache_root) {
                    let mut slug = rel.with_extension("").to_string_lossy().replace('\\', "/");
                    if slug.starts_with('/') {
                        slug.remove(0);
                    }
                    if slug.is_empty() {
                        slug = "index".into();
                    }
                    slugs.push(slug);
                }
            }
        }
        Ok(slugs)
    }
}
