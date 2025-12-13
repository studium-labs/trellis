use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

use actix_web::{HttpResponse, HttpResponseBuilder, Responder, get, web};
use handlebars::Handlebars;
use log::error;
use serde::Serialize;
use serde_json::json;
use serde_yaml;
use std::fs;

use crate::trellis::bundler::{InlineScripts, inline_scripts};
use crate::trellis::config::google_font_href;
use crate::trellis::styles::compiled_styles;
use crate::trellis::types::{RenderedPage, slug_from_path};
use crate::trellis::{SiteConfig, TrellisEngine, trellis_engine};

use chrono::{Datelike, Utc};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn config(conf: &mut web::ServiceConfig) {
    let api_scope = web::scope("/api").service(healthcheck_handler);

    // Prebuild markdown to cache and collect slugs
    let engine = trellis_engine();
    let mut slugs: Vec<String> = engine.prebuild_all().unwrap_or_default();
    if let Ok(mut cached) = engine.cached_slugs() {
        slugs.append(&mut cached);
    }
    slugs.sort();
    slugs.dedup();

    let site_scope = web::scope("")
        .service(feed_handler)
        // Catch-all route keeps in sync with content changes without restart
        .route(
            "/{slug:.*}",
            web::get().to(
                move |path: web::Path<String>, hb: web::Data<Handlebars<'static>>| {
                    let slug = path.into_inner();
                    async move { render_slug(slug, hb).await }
                },
            ),
        );

    conf.service(api_scope);
    conf.service(site_scope);
}

#[get("/health")]
pub async fn healthcheck_handler() -> impl Responder {
    HttpResponse::Ok().json(json!({ "message": "pong" }))
}

async fn render_slug(slug: String, hb: web::Data<Handlebars<'static>>) -> impl Responder {
    let engine = trellis_engine();
    let raw_slug = slug;
    let trimmed = raw_slug.trim_matches('/');

    let canonical_slug = if trimmed.is_empty() {
        "index".to_string()
    } else if raw_slug.ends_with('/') {
        format!("{}/index", trimmed)
    } else {
        trimmed.to_string()
    };

    if !engine.page_exists(&canonical_slug) {
        return HttpResponse::NotFound().finish();
    }
    let page = match engine.render_page(&canonical_slug) {
        Ok(page) => page,
        Err(err) => {
            error!("failed to render page {}: {}", canonical_slug, err);
            return HttpResponse::NotFound().finish();
        }
    };

    let ctx = build_home_context(engine, page);
    let template = if canonical_slug == "index" {
        "index"
    } else {
        "page"
    };
    render(hb, template, json!(ctx), HttpResponse::Ok())
}

#[get("/feed")]
pub async fn feed_handler(hb: web::Data<Handlebars<'static>>) -> impl Responder {
    render(
        hb,
        "feed",
        json!({ "user": "Guest", "data": "your feed goes here" }),
        HttpResponse::Ok(),
    )
}

fn render(
    hb: web::Data<Handlebars<'static>>,
    template: &str,
    data: serde_json::Value,
    mut builder: HttpResponseBuilder,
) -> HttpResponse {
    match hb.render(template, &data) {
        Ok(body) => builder.content_type("text/html; charset=utf-8").body(body),
        Err(err) => HttpResponse::InternalServerError().body(format!("Template error: {}", err)),
    }
}

#[derive(Serialize, Clone)]
struct NavItem {
    title: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<NavLeaf>>,
}

#[derive(Serialize, Clone)]
struct NavLeaf {
    title: String,
    path: String,
}

#[derive(Serialize)]
struct HomeContext<'a> {
    site: SiteContext,
    nav: Vec<NavItem>,
    article: ArticleContext,
    explorer: ExplorerContext,
    layout: LayoutContext<'a>,
    configuration: &'a SiteConfig,
    styles: String,
    fonts_href: String,
    scripts: InlineScripts,
    footer: FooterContext,
}

#[derive(Serialize)]
struct SiteContext {
    name: String,
    tagline: Option<String>,
}

#[derive(Serialize)]
struct ExplorerContext {
    id: String,
    title: String,
    folder_default_state: String,
    folder_click_behavior: String,
    use_saved_state: bool,
    data_fns_json: String,
}

#[derive(Serialize)]
struct FooterContext {
    year: i32,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<Vec<FooterLink>>,
}

#[derive(Serialize)]
struct FooterLink {
    text: String,
    href: String,
}

#[derive(Serialize)]
struct ArticleContext {
    title: String,
    intro: String,
    created: String,
    updated: String,
    read_time: String,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    html: Option<String>,
}

#[derive(Serialize)]
struct LayoutContext<'a> {
    shared: &'a crate::trellis::layout::SharedLayout,
    content: &'a crate::trellis::layout::PageLayout,
    list: &'a crate::trellis::layout::PageLayout,
}

fn build_home_context<'a>(engine: &'a TrellisEngine, page: RenderedPage) -> HomeContext<'a> {
    let article = to_article(&page);
    let nav = build_nav_from_content(&engine.config);
    let styles = compiled_styles(&engine.config);
    let fonts_href = google_font_href(&engine.config.configuration.theme);
    let footer = footer_context(&engine.config);
    let scripts = inline_scripts();

    HomeContext {
        site: SiteContext {
            name: engine.config.configuration.page_title.clone(),
            tagline: None,
        },
        nav,
        article,
        explorer: explorer_context(&engine.config),
        layout: LayoutContext {
            shared: &engine.shared_layout,
            content: &engine.content_layout,
            list: &engine.list_layout,
        },
        configuration: &engine.config,
        styles,
        fonts_href,
        scripts,
        footer,
    }
}

fn explorer_context(config: &SiteConfig) -> ExplorerContext {
    let cfg = &config.layout.explorer;
    let data_fns_json = serde_json::json!({
        "order": cfg.order,
        "sortFn": cfg.sort_fn,
        "filterFn": cfg.filter_fn,
        "mapFn": cfg.map_fn,
    })
    .to_string();

    ExplorerContext {
        id: "explorer-content".into(),
        title: cfg.title.clone(),
        folder_default_state: cfg.folder_default_state.clone(),
        folder_click_behavior: cfg.folder_click_behavior.clone(),
        use_saved_state: cfg.use_saved_state,
        data_fns_json,
    }
}

fn footer_context(config: &SiteConfig) -> FooterContext {
    let links = links_from_config(&config.layout.footer.links);
    FooterContext {
        year: Utc::now().year(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        links,
    }
}

fn links_from_config(links: &BTreeMap<String, String>) -> Option<Vec<FooterLink>> {
    if links.is_empty() {
        return None;
    }

    Some(
        links
            .iter()
            .map(|(text, href)| FooterLink {
                text: text.to_string(),
                href: href.to_string(),
            })
            .collect(),
    )
}

fn build_nav_from_content(config: &SiteConfig) -> Vec<NavItem> {
    let content_root = resolve_path(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &config.paths.content_root,
    );
    let latest = latest_mtime_recursive(&content_root, &config.configuration.ignore_patterns);

    static NAV_CACHE: OnceLock<RwLock<NavCache>> = OnceLock::new();
    let cache = NAV_CACHE.get_or_init(|| {
        RwLock::new(NavCache {
            mtime: SystemTime::UNIX_EPOCH,
            nav: Vec::new(),
        })
    });

    if let Ok(guard) = cache.read() {
        if guard.mtime >= latest {
            return guard.nav.clone();
        }
    }

    let nav = compute_nav(&content_root, &config.configuration.ignore_patterns);

    if let Ok(mut guard) = cache.write() {
        // Only replace if fresher; avoids races with concurrent builders
        if latest >= guard.mtime {
            guard.mtime = latest;
            guard.nav = nav.clone();
        }
    }

    nav
}

struct NavCache {
    mtime: SystemTime,
    nav: Vec<NavItem>,
}

fn latest_mtime_recursive(root: &Path, ignore_patterns: &[String]) -> SystemTime {
    let mut newest = fs::metadata(root)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root, ignore_patterns))
        .filter_map(Result::ok)
    {
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified > newest {
                    newest = modified;
                }
            }
        }
    }

    newest
}

fn compute_nav(content_root: &Path, ignore_patterns: &[String]) -> Vec<NavItem> {
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

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

        let mut slug = slug_from_path(entry.path(), content_root);
        if slug.ends_with("/index") {
            slug.truncate(slug.len() - "/index".len());
        }
        if slug == "index" || slug.is_empty() {
            // root home handled separately
            continue;
        }

        let mut parts = slug.split('/').collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }

        let group = parts.remove(0).to_string();
        if parts.is_empty() {
            groups.entry(group).or_default();
        } else {
            groups.entry(group).or_default().push(slug);
        }
    }

    let mut nav = Vec::with_capacity(groups.len());

    let humanize = |slug: &str| humanize_segment(slug.rsplit('/').next().unwrap_or(slug));

    let read_frontmatter_title = |path: &Path| -> Option<String> {
        let content = fs::read_to_string(path).ok()?;
        let mut lines = content.lines();
        if lines.next()? != "---" {
            return None;
        }

        let mut fm = Vec::new();
        for line in lines {
            if line.trim() == "---" {
                break;
            }
            fm.push(line);
        }
        if fm.is_empty() {
            return None;
        }

        let yaml = fm.join("\n");
        serde_yaml::from_str::<serde_yaml::Value>(&yaml)
            .ok()?
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let title_for = |slug: &str, is_folder: bool| -> String {
        let path = if is_folder {
            content_root.join(slug).join("index.md")
        } else {
            content_root.join(slug).with_extension("md")
        };

        read_frontmatter_title(&path).unwrap_or_else(|| humanize(slug))
    };

    for (group, mut children) in groups {
        children.sort();

        let children = if children.is_empty() {
            None
        } else {
            Some(
                children
                    .iter()
                    .map(|slug| NavLeaf {
                        title: title_for(slug, false),
                        path: slug.clone(),
                    })
                    .collect(),
            )
        };

        let title = title_for(&group, children.is_some());

        nav.push(NavItem {
            title,
            path: group.clone(),
            children,
        });
    }

    nav
}

fn humanize_segment(segment: &str) -> String {
    segment.replace('-', " ")
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

fn resolve_path(base: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        base.join(path)
    }
}

fn to_article(page: &RenderedPage) -> ArticleContext {
    let page = page.to_owned();

    let created = page
        .frontmatter
        .created
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "".into());
    let updated = page
        .frontmatter
        .updated
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "".into());

    let words = page
        .frontmatter
        .word_count
        .unwrap_or_else(|| page.html.split_whitespace().count().max(1) as u64);

    ArticleContext {
        title: page.frontmatter.title.unwrap_or(String::new()),
        intro: page.frontmatter.description.unwrap_or(String::new()),
        created,
        updated,
        read_time: format!(
            "{} min read",
            ((words as f64) / 200.0).ceil().max(1.0) as u32
        ),
        body: page.html.to_owned(),
        html: Some(page.html.clone()),
    }
}
