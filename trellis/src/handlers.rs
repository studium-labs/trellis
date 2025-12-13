use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

use actix_web::http::header::CONTENT_TYPE;
use actix_web::{HttpResponse, HttpResponseBuilder, Responder, get, web};
use handlebars::Handlebars;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use serde_yaml;
use std::fs;

use crate::trellis::cache;
use crate::trellis::config::{google_font_href, theme_css_variables};
use crate::trellis::types::{RenderedPage, slug_from_path};
use crate::trellis::{RootzEngine, SiteConfig};
use chrono::{Datelike, Utc};
use grass;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[get("/health")]
pub async fn healthcheck_handler() -> impl Responder {
    HttpResponse::Ok().json(json!({ "message": "pong" }))
}

#[get("/static/explorer.bundle.js")]
pub async fn explorer_script_handler() -> impl Responder {
    const JS: &str = include_str!("../templates/components/scripts/explorer.inline.js");
    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, "application/javascript"))
        .body(JS)
}

#[get("/js/{tail:.*}")]
pub async fn js_handler(path: web::Path<String>) -> impl Responder {
    let tail = path.into_inner();
    let Some(path) = resolve_js_path(&tail) else {
        return HttpResponse::NotFound().finish();
    };
    println!("READING PATH: {:?}", &path);

    match std::fs::read_to_string(&path) {
        Ok(body) => HttpResponse::Ok()
            .insert_header((CONTENT_TYPE, "application/javascript"))
            .body(body),
        Err(err) => {
            log::error!("Failed to read JS file {:?}: {}", path, err);
            HttpResponse::InternalServerError().body("failed to load script")
        }
    }
}

fn resolve_js_path(tail: &str) -> Option<PathBuf> {
    let tail = if tail.ends_with(".js") {
        tail.to_string()
    } else {
        format!("{tail}.js")
    };

    let templates_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");

    let direct = templates_root.join(&tail);
    if direct.exists() {
        return Some(direct);
    }

    let util = templates_root.join("util").join(&tail);
    if util.exists() {
        return Some(util);
    }

    None
}

fn quartz_engine() -> &'static RootzEngine {
    static ENGINE: OnceLock<RootzEngine> = OnceLock::new();
    ENGINE.get_or_init(|| RootzEngine::new(SiteConfig::load()).expect("init quartz engine"))
}

#[get("/")]
pub async fn home_handler(hb: web::Data<Handlebars<'static>>) -> impl Responder {
    let engine = quartz_engine();
    let page = match engine.render_page("index") {
        Ok(page) => page,
        Err(err) => {
            log::error!("failed to render page: {}", err);
            placeholder_page()
        }
    };

    let ctx = build_home_context(engine, page);
    render(hb, "index", json!(ctx), HttpResponse::Ok())
}

async fn render_slug(slug: String, hb: web::Data<Handlebars<'static>>) -> impl Responder {
    let engine = quartz_engine();
    let page = match engine.render_page(&slug) {
        Ok(page) => page,
        Err(err) => {
            log::error!("failed to render page {}: {}", slug, err);
            placeholder_page()
        }
    };

    let ctx = build_home_context(engine, page);
    render(hb, "page", json!(ctx), HttpResponse::Ok())
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

#[get("/error")]
pub async fn error_handler(hb: web::Data<Handlebars<'static>>) -> impl Responder {
    let engine = quartz_engine();
    let footer = footer_context(&engine.config);
    render(
        hb,
        "error",
        json!({ "status_code": 500, "error": "Something went wrong", "footer": footer }),
        HttpResponse::InternalServerError(),
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

#[derive(Serialize)]
struct NavItem {
    title: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<NavLeaf>>,
}

#[derive(Serialize)]
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
    body: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    html: Option<String>,
}

#[derive(Serialize)]
struct LayoutContext<'a> {
    shared: &'a crate::trellis::layout::SharedLayout,
    content: &'a crate::trellis::layout::PageLayout,
    list: &'a crate::trellis::layout::PageLayout,
}

fn build_home_context<'a>(engine: &'a RootzEngine, page: RenderedPage) -> HomeContext<'a> {
    let article = to_article(&page);
    let nav = build_nav_from_content();
    let styles = compiled_styles(&engine.config);
    let fonts_href = google_font_href(&engine.config.configuration.theme);
    let footer = footer_context(&engine.config);

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

fn build_nav_from_content() -> Vec<NavItem> {
    let content_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content");
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for entry in WalkDir::new(&content_root)
        .into_iter()
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

        let mut slug = slug_from_path(entry.path(), &content_root);
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

    for (group, mut children) in groups {
        children.sort();

        let children = if children.is_empty() {
            None
        } else {
            Some(
                children
                    .iter()
                    .map(|slug| NavLeaf {
                        title: title_for_file(slug, &content_root),
                        path: slug.clone(),
                    })
                    .collect(),
            )
        };

        let title = if children.is_some() {
            title_for_folder(&group, &content_root)
        } else {
            title_for_file(&group, &content_root)
        };

        nav.push(NavItem {
            title,
            path: group.clone(),
            children,
        });
    }

    nav
}

fn title_for_folder(folder: &str, content_root: &Path) -> String {
    let index_path = content_root.join(folder).join("index.md");
    if let Some(title) = frontmatter_title(&index_path) {
        return title;
    }

    humanize_segment(folder.rsplit('/').next().unwrap_or(folder))
}

fn title_for_file(slug: &str, content_root: &Path) -> String {
    let path = content_root.join(slug).with_extension("md");
    if let Some(title) = frontmatter_title(&path) {
        return title;
    }

    humanize_segment(slug.rsplit('/').next().unwrap_or(slug))
}

fn frontmatter_title(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut lines = content.lines();

    if lines.next()? != "---" {
        return None;
    }

    let mut fm_lines = vec![];
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        fm_lines.push(line);
    }

    if fm_lines.is_empty() {
        return None;
    }

    let yaml = fm_lines.join("\n");
    serde_yaml::from_str::<serde_yaml::Value>(&yaml)
        .ok()?
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn humanize_segment(segment: &str) -> String {
    segment.replace('-', " ")
}

fn to_article(page: &RenderedPage) -> ArticleContext {
    let intro = page
        .meta
        .description
        .clone()
        .or_else(|| {
            page.frontmatter
                .get("description")
                .and_then(|v| v.as_str().map(String::from))
        })
        .unwrap_or_else(|| "This page is rendered on-demand from markdown.".into());

    let title = page
        .meta
        .title
        .clone()
        .or_else(|| {
            page.frontmatter
                .get("title")
                .and_then(|v| v.as_str().map(String::from))
        })
        .unwrap_or_else(|| "Untitled".into());

    let created = page
        .meta
        .created
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "".into());

    let updated = page
        .meta
        .updated
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "".into());

    let words = page
        .frontmatter
        .get("word_count")
        .and_then(|v| v.as_u64())
        .map(|n| n.max(1) as usize)
        .unwrap_or_else(|| page.html.split_whitespace().count().max(1));

    let read_time = ((words as f64) / 200.0).ceil().max(1.0) as u32;

    ArticleContext {
        title,
        intro,
        created,
        updated,
        read_time: format!("{} min read", read_time),
        body: html_to_paragraphs(&page.html),
        html: Some(page.html.clone()),
    }
}

fn compiled_styles(cfg: &SiteConfig) -> String {
    static STYLES: OnceLock<RwLock<StylesCache>> = OnceLock::new();

    let scss_mtime = latest_scss_mtime();
    let cache = STYLES.get_or_init(|| {
        RwLock::new(StylesCache {
            css: compile_scss(cfg),
            mtime: scss_mtime,
        })
    });

    if let Ok(guard) = cache.read() {
        if guard.mtime >= scss_mtime {
            return guard.css.clone();
        }
    }

    if let Ok(mut guard) = cache.write() {
        if guard.mtime < scss_mtime {
            guard.css = compile_scss(cfg);
            guard.mtime = scss_mtime;
        }
        return guard.css.clone();
    }

    // Fallback in case the lock is poisoned.
    compile_scss(cfg)
}

struct StylesCache {
    css: String,
    mtime: SystemTime,
}

fn compile_scss(cfg: &SiteConfig) -> String {
    let theme_vars = theme_css_variables(&cfg.configuration.theme);
    let scss_path = scss_entry_path();
    let include_path = scss_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| scss_root());

    match grass::from_path(
        &scss_path,
        &grass::Options::default()
            .load_path(include_path)
            .style(grass::OutputStyle::Compressed),
    ) {
        Ok(css) => format!("{theme_vars}\n{css}"),
        Err(err) => {
            log::warn!("Failed to compile SCSS at {:?}: {err}", scss_path);
            theme_vars
        }
    }
}

fn scss_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/assets/styles/")
}

fn scss_entry_path() -> std::path::PathBuf {
    scss_root().join("custom.scss")
}

fn latest_scss_mtime() -> SystemTime {
    cache::newest_mtime_with_extension(&scss_root(), "scss").unwrap_or(SystemTime::UNIX_EPOCH)
}

fn html_to_paragraphs(html: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("paragraph regex"));
    let text = re.replace_all(html, "");

    text.split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn placeholder_page() -> RenderedPage {
    RenderedPage {
        slug: "index".into(),
        html: "<p>Quartz could not render this page.</p>".into(),
        frontmatter: Default::default(),
        meta: Default::default(),
        cached: None,
    }
}

pub fn config(conf: &mut web::ServiceConfig) {
    let api_scope = web::scope("/api").service(healthcheck_handler);

    let engine = quartz_engine();

    // Prebuild markdown to cache and collect slugs
    let mut slugs: Vec<String> = engine.prebuild_all().unwrap_or_default();
    if let Ok(mut cached) = engine.cached_slugs() {
        slugs.append(&mut cached);
    }
    slugs.sort();
    slugs.dedup();

    let mut site_scope = web::scope("")
        .service(home_handler)
        .service(feed_handler)
        .service(error_handler)
        .service(explorer_script_handler)
        .service(js_handler);

    for slug in slugs.into_iter().filter(|s| !s.is_empty() && s != "index") {
        let path = format!("/{}", slug);
        let slug_clone = slug.clone();
        site_scope = site_scope.route(
            path.as_str(),
            web::get().to(move |hb: web::Data<Handlebars<'static>>| {
                let slug = slug_clone.clone();
                async move { render_slug(slug, hb).await }
            }),
        );
    }

    conf.service(api_scope);
    conf.service(site_scope);
}
