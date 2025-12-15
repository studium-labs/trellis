use std::collections::BTreeMap;

use confik::Configuration;
use serde::{Deserialize, Serialize};

use crate::trellis::config::SiteConfig;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "config")]
pub enum LayoutComponent {
    Head,
    Footer(FooterConfig),
    PageTitle,
    Breadcrumbs,
    ArticleTitle,
    ContentMeta,
    TagList,
    Search,
    Darkmode,
    ReaderMode,
    Explorer(ExplorerConfig),
    Graph,
    TableOfContents,
    Backlinks(BacklinksConfig),
    Spacer,
    Flex(FlexConfig),
    MobileOnly(Box<LayoutComponent>),
    DesktopOnly(Box<LayoutComponent>),
}

#[derive(Clone, Debug, Serialize)]
pub struct FlexItem {
    pub component: LayoutComponent,
    #[serde(default)]
    pub grow: bool,
    #[serde(default)]
    pub shrink: bool,
    #[serde(default)]
    pub basis: Option<String>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub justify: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FlexConfig {
    pub components: Vec<FlexItem>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub wrap: Option<String>,
    #[serde(default)]
    pub gap: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Configuration)]
pub struct LayoutConfig {
    #[serde(default)]
    pub footer: FooterConfig,
    #[serde(default)]
    pub explorer: ExplorerConfig,
    #[serde(default)]
    pub backlinks: BacklinksConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, Configuration, Default)]
pub struct ExplorerConfig {
    #[serde(default = "default_explorer_title")]
    pub title: String,
    #[serde(default = "default_folder_default_state")]
    pub folder_default_state: String,
    #[serde(default = "default_folder_click_behavior")]
    pub folder_click_behavior: String,
    #[serde(default = "default_use_saved_state")]
    pub use_saved_state: bool,
    #[serde(default = "default_sort_fn")]
    pub sort_fn: String,
    #[serde(default = "default_filter_fn")]
    pub filter_fn: String,
    #[serde(default = "default_map_fn")]
    pub map_fn: String,
    #[serde(default = "default_order")]
    pub order: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Configuration, Default)]
pub struct BacklinksConfig {
    #[serde(default = "default_backlinks_title")]
    pub title: String,
    #[serde(default = "default_no_backlinks_text")]
    pub empty_text: String,
    #[serde(default = "default_hide_when_empty")]
    pub hide_when_empty: bool,
}

fn default_explorer_title() -> String {
    "Explorer".into()
}

fn default_folder_default_state() -> String {
    "collapsed".into()
}

fn default_folder_click_behavior() -> String {
    "collapse".into()
}

fn default_use_saved_state() -> bool {
    true
}

fn default_sort_fn() -> String {
    "(a,b)=>{if((!a.isFolder&&!b.isFolder)||(a.isFolder&&b.isFolder)){return a.displayName.localeCompare(b.displayName,undefined,{numeric:true,sensitivity:'base'});}if(!a.isFolder&&b.isFolder){return 1;}else{return -1;}}".into()
}

fn default_filter_fn() -> String {
    "(node)=>node.slugSegment!=='tags'".into()
}

fn default_map_fn() -> String {
    "(node)=>node".into()
}

fn default_order() -> Vec<String> {
    vec!["filter".into(), "map".into(), "sort".into()]
}

fn default_backlinks_title() -> String {
    "Backlinks".into()
}

fn default_no_backlinks_text() -> String {
    "No backlinks found.".into()
}

fn default_hide_when_empty() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, Configuration)]
pub struct FooterConfig {
    #[serde(default = "default_footer_links")]
    pub links: BTreeMap<String, String>,
}

pub fn default_footer_links() -> BTreeMap<String, String> {
    let mut links = BTreeMap::new();
    links.insert(
        "GitHub".into(),
        "https://github.com/jackyzha0/quartz".into(),
    );
    links.insert(
        "Discord Community".into(),
        "https://discord.gg/cRFFHYye7t".into(),
    );
    links
}

impl Default for FooterConfig {
    fn default() -> Self {
        Self {
            links: default_footer_links(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PageLayout {
    pub before_body: Vec<LayoutComponent>,
    pub left: Vec<LayoutComponent>,
    pub right: Vec<LayoutComponent>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SharedLayout {
    pub head: LayoutComponent,
    pub header: Vec<LayoutComponent>,
    pub footer: LayoutComponent,
    #[serde(default)]
    pub after_body: Vec<LayoutComponent>,
}

fn flex_header_stack(include_reader: bool) -> LayoutComponent {
    let mut components = vec![
        FlexItem {
            component: LayoutComponent::Search,
            grow: true,
            shrink: false,
            basis: None,
            order: None,
            align: None,
            justify: None,
        },
        FlexItem {
            component: LayoutComponent::Darkmode,
            grow: false,
            shrink: false,
            basis: None,
            order: None,
            align: None,
            justify: None,
        },
    ];

    if include_reader {
        components.push(FlexItem {
            component: LayoutComponent::ReaderMode,
            grow: false,
            shrink: false,
            basis: None,
            order: None,
            align: None,
            justify: None,
        });
    }

    LayoutComponent::Flex(FlexConfig {
        components,
        direction: Some("row".into()),
        wrap: None,
        gap: Some("1rem".into()),
    })
}

pub fn shared_layout(cfg: &SiteConfig) -> SharedLayout {
    let footer_cfg = cfg.layout.footer.clone();
    SharedLayout {
        head: LayoutComponent::Head,
        header: vec![],
        after_body: vec![],
        footer: LayoutComponent::Footer(footer_cfg),
    }
}

pub fn default_content_page_layout() -> PageLayout {
    PageLayout {
        before_body: vec![
            LayoutComponent::Breadcrumbs,
            LayoutComponent::ArticleTitle,
            LayoutComponent::ContentMeta,
            LayoutComponent::TagList,
        ],
        left: vec![
            LayoutComponent::PageTitle,
            LayoutComponent::MobileOnly(Box::new(LayoutComponent::Spacer)),
            flex_header_stack(true),
            LayoutComponent::Explorer(LayoutConfig::default().explorer.clone()),
        ],
        right: vec![
            LayoutComponent::Graph,
            LayoutComponent::DesktopOnly(Box::new(LayoutComponent::TableOfContents)),
            LayoutComponent::Backlinks(LayoutConfig::default().backlinks.clone()),
        ],
    }
}

pub fn default_list_page_layout() -> PageLayout {
    PageLayout {
        before_body: vec![
            LayoutComponent::Breadcrumbs,
            LayoutComponent::ArticleTitle,
            LayoutComponent::ContentMeta,
            LayoutComponent::TagList,
        ],
        left: vec![
            LayoutComponent::PageTitle,
            LayoutComponent::MobileOnly(Box::new(LayoutComponent::Spacer)),
            flex_header_stack(false),
            LayoutComponent::Explorer(LayoutConfig::default().explorer.clone()),
        ],
        right: vec![],
    }
}
