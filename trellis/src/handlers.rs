use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

use actix_files::Files;
use actix_web::{HttpResponse, HttpResponseBuilder, Responder, get, web};
use handlebars::Handlebars;
use log::error;
use serde::Serialize;
use serde_json;
use serde_json::json;
use serde_yaml;
use std::fs;

use crate::trellis::bundler::{InlineScripts, ScriptNeeds, inline_scripts};
use crate::trellis::config::google_font_href;
use crate::trellis::content_index::{extract_links, generate_content_index};
use crate::trellis::layout::LayoutComponent;
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
    if let Err(err) = generate_content_index(
        engine.content_root(),
        engine.cache_root(),
        &engine.config.configuration.ignore_patterns,
    ) {
        error!("failed to generate content index: {err}");
    }

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
    conf.service(
        Files::new("/static", engine.cache_root().join("static"))
            .prefer_utf8(true)
            .use_last_modified(true),
    );
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
    #[serde(skip_serializing_if = "is_false")]
    open: bool,
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
    graph: GraphContext,
    backlinks: BacklinksContext,
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

#[derive(Serialize, Clone)]
struct GraphContext {
    title: String,
    local_cfg_json: String,
    global_cfg_json: String,
}

#[derive(Serialize, Clone)]
struct BacklinkEntry {
    title: String,
    slug: String,
    href: String,
}

#[derive(Serialize, Clone)]
struct BacklinksContext {
    title: String,
    items: Vec<BacklinkEntry>,
    empty_text: String,
    hide_when_empty: bool,
    has_backlinks: bool,
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
    slug: String,
    title: String,
    intro: String,
    created: String,
    updated: String,
    read_time: String,
    body: String,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    html: Option<String>,
}

#[derive(Serialize)]
struct LayoutContext<'a> {
    shared: &'a crate::trellis::layout::SharedLayout,
    content: &'a crate::trellis::layout::PageLayout,
    list: &'a crate::trellis::layout::PageLayout,
}

fn script_needs(page: &RenderedPage, layout: &LayoutContext) -> ScriptNeeds {
    let html = &page.html;
    let has_mermaid = html.contains("class=\"mermaid\"");
    let has_callouts = html.contains("class=\"callout ");
    let encrypted = page.frontmatter.encrypted.unwrap_or(false);
    let has_explorer = layout_contains_explorer(layout);
    let has_graph = layout_contains_graph(layout);

    ScriptNeeds {
        explorer: has_explorer,
        overlay_explorer: has_explorer,
        encrypted_note: encrypted,
        mermaid: has_mermaid,
        callouts: has_callouts,
        graph: has_graph,
    }
}

fn layout_contains_explorer(layout: &LayoutContext) -> bool {
    component_list_has_explorer(&layout.shared.header)
        || component_list_has_explorer(&layout.content.left)
        || component_list_has_explorer(&layout.content.before_body)
        || component_list_has_explorer(&layout.content.right)
        || component_list_has_explorer(&layout.list.left)
        || component_list_has_explorer(&layout.list.before_body)
        || component_list_has_explorer(&layout.list.right)
        || matches!(layout.shared.head, LayoutComponent::Explorer(_))
        || matches!(layout.shared.footer, LayoutComponent::Explorer(_))
        || component_list_has_explorer(&layout.shared.after_body)
}

fn component_list_has_explorer(list: &[LayoutComponent]) -> bool {
    list.iter().any(component_has_explorer)
}

fn component_has_explorer(component: &LayoutComponent) -> bool {
    match component {
        LayoutComponent::Explorer(_) => true,
        LayoutComponent::Flex(cfg) => cfg
            .components
            .iter()
            .any(|item| component_has_explorer(&item.component)),
        LayoutComponent::MobileOnly(inner) | LayoutComponent::DesktopOnly(inner) => {
            component_has_explorer(inner)
        }
        _ => false,
    }
}

fn layout_contains_graph(layout: &LayoutContext) -> bool {
    component_list_has_graph(&layout.shared.header)
        || component_list_has_graph(&layout.content.left)
        || component_list_has_graph(&layout.content.before_body)
        || component_list_has_graph(&layout.content.right)
        || component_list_has_graph(&layout.list.left)
        || component_list_has_graph(&layout.list.before_body)
        || component_list_has_graph(&layout.list.right)
        || matches!(layout.shared.head, LayoutComponent::Graph)
        || matches!(layout.shared.footer, LayoutComponent::Graph)
        || component_list_has_graph(&layout.shared.after_body)
}

fn component_list_has_graph(list: &[LayoutComponent]) -> bool {
    list.iter().any(component_has_graph)
}

fn component_has_graph(component: &LayoutComponent) -> bool {
    match component {
        LayoutComponent::Graph => true,
        LayoutComponent::Flex(cfg) => cfg
            .components
            .iter()
            .any(|item| component_has_graph(&item.component)),
        LayoutComponent::MobileOnly(inner) | LayoutComponent::DesktopOnly(inner) => {
            component_has_graph(inner)
        }
        _ => false,
    }
}

fn build_home_context<'a>(engine: &'a TrellisEngine, page: RenderedPage) -> HomeContext<'a> {
    let article = to_article(&page);
    let nav = build_nav_from_content(&engine.config, &article.slug);
    let styles = compiled_styles(&engine.config);
    let fonts_href = google_font_href(&engine.config.configuration.theme);
    let footer = footer_context(&engine.config);
    let graph = graph_context();
    let backlinks = backlinks_context(engine, &article.slug);

    let layout_ctx = LayoutContext {
        shared: &engine.shared_layout,
        content: &engine.content_layout,
        list: &engine.list_layout,
    };
    let scripts = inline_scripts(script_needs(&page, &layout_ctx));

    HomeContext {
        site: SiteContext {
            name: engine.config.configuration.page_title.clone(),
            tagline: None,
        },
        nav,
        article,
        explorer: explorer_context(&engine.config),
        graph,
        backlinks,
        layout: layout_ctx,
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

fn graph_context() -> GraphContext {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct D3Config<'a> {
        drag: bool,
        zoom: bool,
        depth: i32,
        scale: f32,
        repel_force: f32,
        center_force: f32,
        link_distance: i32,
        font_size: f32,
        opacity_scale: f32,
        remove_tags: &'a [&'a str],
        show_tags: bool,
        focus_on_hover: bool,
        enable_radial: bool,
    }

    let local = D3Config {
        drag: true,
        zoom: true,
        depth: 1,
        scale: 1.1,
        repel_force: 0.5,
        center_force: 0.3,
        link_distance: 30,
        font_size: 0.6,
        opacity_scale: 1.0,
        remove_tags: &[],
        show_tags: true,
        focus_on_hover: false,
        enable_radial: false,
    };

    let global = D3Config {
        drag: true,
        zoom: true,
        depth: -1,
        scale: 0.9,
        repel_force: 0.5,
        center_force: 0.2,
        link_distance: 30,
        font_size: 0.6,
        opacity_scale: 1.0,
        remove_tags: &[],
        show_tags: true,
        focus_on_hover: true,
        enable_radial: true,
    };

    GraphContext {
        title: "Graph".into(),
        local_cfg_json: serde_json::to_string(&local).unwrap_or_else(|_| "{}".into()),
        global_cfg_json: serde_json::to_string(&global).unwrap_or_else(|_| "{}".into()),
    }
}

fn backlinks_context(engine: &TrellisEngine, current_slug: &str) -> BacklinksContext {
    let content_root = engine.content_root();
    let ignore_patterns = &engine.config.configuration.ignore_patterns;
    let targets = backlink_targets(current_slug);

    let mut items = Vec::new();

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

        let source_slug = slug_from_path(entry.path(), content_root);
        if source_slug == current_slug {
            continue;
        }

        let Ok(content) = fs::read_to_string(entry.path()) else {
            continue;
        };

        let links = extract_links(&content);
        if !links.iter().any(|link| targets.contains(link)) {
            continue;
        }

        let mut backlink_slug = source_slug.clone();
        if backlink_slug.ends_with("/index") {
            backlink_slug.truncate(backlink_slug.len() - "/index".len());
        }

        let href = if backlink_slug.is_empty() || backlink_slug == "index" {
            "/".to_string()
        } else {
            format!("/{}", backlink_slug)
        };

        let title = frontmatter_title(entry.path()).unwrap_or_else(|| {
            humanize_segment(backlink_slug.rsplit('/').next().unwrap_or(&backlink_slug))
        });

        items.push(BacklinkEntry {
            title,
            slug: backlink_slug,
            href,
        });
    }

    items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    let has_backlinks = !items.is_empty();
    let cfg = &engine.config.layout.backlinks;

    BacklinksContext {
        title: cfg.title.clone(),
        items,
        empty_text: cfg.empty_text.clone(),
        hide_when_empty: cfg.hide_when_empty,
        has_backlinks,
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

fn backlink_targets(slug: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut normalized = slug.trim_matches('/').to_string();
    if normalized.is_empty() {
        normalized = "index".into();
    }

    let mut push_unique = |value: String| {
        if !targets.contains(&value) {
            targets.push(value);
        }
    };

    push_unique(normalized.clone());

    if let Some(stripped) = normalized.strip_suffix("/index") {
        push_unique(stripped.to_string());
    }

    if normalized == "index" {
        push_unique(".".into());
    }

    targets
}

fn frontmatter_title(path: &Path) -> Option<String> {
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
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn build_nav_from_content(config: &SiteConfig, current_slug: &str) -> Vec<NavItem> {
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

    let cached = if let Ok(guard) = cache.read() {
        if guard.mtime >= latest {
            Some(guard.nav.clone())
        } else {
            None
        }
    } else {
        None
    };

    let mut nav = cached.unwrap_or_else(|| {
        let computed = compute_nav(&content_root, &config.configuration.ignore_patterns);

        if let Ok(mut guard) = cache.write() {
            // Only replace if fresher; avoids races with concurrent builders
            if latest >= guard.mtime {
                guard.mtime = latest;
                guard.nav = computed.clone();
            }
        }

        computed
    });

    mark_nav_open(&mut nav, current_slug);

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

    let title_for = |slug: &str, is_folder: bool| -> String {
        let path = if is_folder {
            content_root.join(slug).join("index.md")
        } else {
            content_root.join(slug).with_extension("md")
        };

        frontmatter_title(&path).unwrap_or_else(|| humanize(slug))
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
            open: false,
            children,
        });
    }

    nav
}

fn mark_nav_open(nav: &mut [NavItem], current_slug: &str) {
    let normalized = current_slug.trim_end_matches('/');
    for item in nav {
        let group_prefix = format!("{}/", item.path);
        let mut is_open = normalized == item.path || normalized.starts_with(&group_prefix);

        if !is_open {
            if let Some(children) = &item.children {
                is_open = children.iter().any(|child| child.path == normalized);
            }
        }

        item.open = is_open;
    }
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
        slug: page.slug.clone(),
        title: page.frontmatter.title.unwrap_or(String::new()),
        intro: page.frontmatter.description.unwrap_or(String::new()),
        created,
        updated,
        read_time: format!(
            "{} min read",
            ((words as f64) / 200.0).ceil().max(1.0) as u32
        ),
        body: page.html.to_owned(),
        tags: page.frontmatter.tags.unwrap_or_default(),
        html: Some(page.html.clone()),
    }
}
