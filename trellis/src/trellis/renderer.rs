use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use log::debug;
use walkdir::WalkDir;

use crate::trellis::cache;
use crate::trellis::config::{SiteConfig, theme_hash};
use crate::trellis::layout::{
    default_content_page_layout, default_list_page_layout, shared_layout,
};
use crate::trellis::plugins::{DraftFilter, PluginRegistry};
use crate::trellis::types::{Page, RenderedPage, slug_from_path};

pub struct TrellisEngine {
    pub config: SiteConfig,
    pub shared_layout: crate::trellis::layout::SharedLayout,
    pub content_layout: crate::trellis::layout::PageLayout,
    pub list_layout: crate::trellis::layout::PageLayout,
    registry: PluginRegistry,
    content_root: PathBuf,
    cache_root: PathBuf,
}

impl TrellisEngine {
    pub fn new(config: SiteConfig) -> Result<Self> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let content_root = resolve_path(&manifest, &config.paths.content_root);
        let cache_root = resolve_path(&manifest, &config.paths.cache_root);

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
        if self.is_ignored_slug(slug) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("slug {slug} is ignored by configuration"),
            )
            .into());
        }
        let source_path = self.source_path_for(slug);
        let cache_path = cache::cache_path(&self.cache_root, slug);
        let styles_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/assets/styles");
        let styles_mtime = cache::newest_mtime_with_extension(&styles_root, "scss")
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let binary_mtime = cache::binary_mtime();
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.yml");
        let config_mtime = fs::metadata(&config_path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let theme_hash = theme_hash(&self.config.configuration.theme);
        let theme_mtime = cache::update_hash_marker(&self.cache_root, "theme", &theme_hash)
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let use_cache = source_path.exists()
            && cache_path.exists()
            && cache::cache_is_fresh(
                &source_path,
                &cache_path,
                &[styles_mtime, binary_mtime, config_mtime, theme_mtime],
            )?;

        let page = self.load_page(slug, &source_path)?;

        // Always parse frontmatter (and other metadata) even when reusing cached HTML.
        // We still run the transformer pipeline to populate PageMetadata/frontmatter.
        // If we are using the cache, we overwrite the freshly-rendered HTML with the cached HTML.
        let Some(mut page) = self.registry.transform(page)? else {
            bail!("page filtered out by plugins: {slug}");
        };

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

    /// Check if a source markdown file exists for the given slug.
    /// Cached HTML without a source is treated as missing.
    pub fn page_exists(&self, slug: &str) -> bool {
        if self.is_ignored_slug(slug) {
            return false;
        }
        let source_path = self.source_path_for(slug);
        source_path.exists()
    }

    fn source_path_for(&self, slug: &str) -> PathBuf {
        let mut path = self.content_root.join(slug);
        if path.extension().is_none() {
            path = path.with_extension("md");
        }
        path
    }

    /// Path to the configured cache root (build output).
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Path to the configured content root.
    pub fn content_root(&self) -> &Path {
        &self.content_root
    }

    fn load_page(&self, slug: &str, path: &Path) -> Result<Page> {
        if self.is_ignored_slug(slug) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("slug {slug} is ignored by configuration"),
            )
            .into());
        }
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("reading markdown at {}", path.display()))?;

            if content.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("empty markdown for slug {slug}"),
                )
                .into());
            }

            Ok(Page::new(slug.to_string(), path.to_path_buf(), content))
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing markdown for slug {slug}"),
            )
            .into())
        }
    }

    /// Pre-render all markdown files under the content root into cache, returning slugs.
    pub fn prebuild_all(&self) -> Result<Vec<String>> {
        let mut slugs = vec![];
        for entry in WalkDir::new(&self.content_root)
            .into_iter()
            .filter_entry(|e| !self.is_ignored_path(e.path()))
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
                // Skip filtered pages (e.g., draft notes) during prebuild
                match self.render_page(&slug) {
                    Ok(_) => slugs.push(slug),
                    Err(err) => {
                        // Only log filter-related skips; bubble up real errors
                        if err.to_string().contains("page filtered out by plugins") {
                            debug!("Skipping filtered page {slug}");
                        } else {
                            return Err(err);
                        }
                    }
                }
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

fn resolve_path(base: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        base.join(path)
    }
}

impl TrellisEngine {
    fn is_ignored_slug(&self, slug: &str) -> bool {
        let path = self.source_path_for(slug);
        self.is_ignored_path(&path)
    }

    fn is_ignored_path(&self, path: &Path) -> bool {
        let patterns = &self.config.configuration.ignore_patterns;
        let Ok(rel) = path.strip_prefix(&self.content_root) else {
            return false;
        };

        rel.components().any(|comp| {
            comp.as_os_str()
                .to_str()
                .map(|s| patterns.iter().any(|p| p == s))
                .unwrap_or(false)
        })
    }
}
