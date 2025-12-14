use std::path::Path;

use confik::{Configuration, EnvSource};
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};

use self::yaml::YamlFileSource;
use crate::trellis::layout::LayoutConfig;

fn default_host() -> String {
    "0.0.0.0".into()
}

fn default_port() -> u16 {
    40075
}

fn default_max_payload_mb() -> usize {
    100
}

fn default_content_root() -> String {
    "../content".into()
}

fn default_cache_root() -> String {
    "../.build".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct ThemeFonts {
    pub header: String,
    pub body: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct ThemePalette {
    pub light: String,
    pub lightgray: String,
    pub gray: String,
    pub darkgray: String,
    pub dark: String,
    pub secondary: String,
    pub tertiary: String,
    pub highlight: String,
    pub text_highlight: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct ThemeMode {
    pub light_mode: ThemePalette,
    pub dark_mode: ThemePalette,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct ThemeConfig {
    pub font_origin: String,
    pub cdn_caching: bool,
    pub typography: ThemeFonts,
    pub colors: ThemeMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
#[serde(rename_all = "lowercase")]
#[confik(forward(serde(rename_all = "lowercase")))]
pub enum DefaultDateType {
    Created,
    Modified,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct GlobalConfiguration {
    pub page_title: String,
    pub tagline: Option<String>,
    #[serde(default)]
    pub page_title_suffix: String,
    pub enable_spa: bool,
    pub enable_popovers: bool,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
    #[serde(default = "default_date_type_modified")]
    pub default_date_type: DefaultDateType,
    pub theme: ThemeConfig,
}

fn default_date_type_modified() -> DefaultDateType {
    DefaultDateType::Modified
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub cors_origins: Vec<String>,
    #[serde(default = "default_max_payload_mb")]
    pub max_payload_mb: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            cors_origins: vec!["0.0.0.0:40075".into()],
            max_payload_mb: default_max_payload_mb(),
        }
    }
}

impl ServerConfig {
    pub fn max_payload_bytes(&self) -> usize {
        self.max_payload_mb.saturating_mul(1024 * 1024)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct PathsConfig {
    #[serde(default = "default_content_root")]
    pub content_root: String,
    #[serde(default = "default_cache_root")]
    pub cache_root: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            content_root: default_content_root(),
            cache_root: default_cache_root(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
#[serde(rename_all = "lowercase")]
#[confik(forward(serde(rename_all = "lowercase")))]
pub enum JsContentType {
    External,
    Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub enum JsLoadTime {
    #[serde(rename = "beforeDOMReady")]
    BeforeDomReady,
    #[serde(rename = "afterDOMReady")]
    AfterDomReady,
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct JsResource {
    #[serde(rename = "loadTime")]
    pub load_time: JsLoadTime,
    #[serde(rename = "contentType")]
    pub content_type: JsContentType,
    #[serde(rename = "moduleType", skip_serializing_if = "Option::is_none")]
    pub module_type: Option<String>,
    #[serde(rename = "spaPreserve", default)]
    pub spa_preserve: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

impl JsResource {
    pub fn external(load_time: JsLoadTime, src: String) -> Self {
        Self {
            load_time,
            content_type: JsContentType::External,
            module_type: None,
            spa_preserve: false,
            src: Some(src),
            script: None,
        }
    }

    pub fn inline(load_time: JsLoadTime, script: String) -> Self {
        Self {
            load_time,
            content_type: JsContentType::Inline,
            module_type: None,
            spa_preserve: false,
            src: None,
            script: Some(script),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct CssResource {
    pub content: String,
    #[serde(default)]
    pub inline: bool,
    #[serde(rename = "spaPreserve", default)]
    pub spa_preserve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Configuration)]
pub struct ComponentResources {
    #[serde(default)]
    pub css: Vec<CssResource>,
    #[serde(default)]
    pub js: Vec<JsResource>,
    #[serde(default, rename = "additionalHead")]
    pub additional_head: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Configuration)]
pub struct PluginConfig {
    /// Static resources contributed by plugins (e.g., LaTeX or other emitters).
    #[serde(default)]
    pub resources: ComponentResources,
}

impl PluginConfig {
    /// Combine plugin-provided static assets with the core Quartz bundles for a page.
    pub fn page_resources(&self, base_dir: &str) -> ComponentResources {
        page_resources(base_dir, &self.resources)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Configuration)]
pub struct SiteConfig {
    pub configuration: GlobalConfiguration,
    pub layout: LayoutConfig,
    pub plugins: PluginConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub paths: PathsConfig,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            configuration: GlobalConfiguration {
                page_title: "Moss".into(),
                tagline: None,
                page_title_suffix: String::new(),
                enable_spa: true,
                enable_popovers: true,
                locale: "en-US".into(),
                base_url: None,
                ignore_patterns: vec!["private".into(), "templates".into(), ".obsidian".into()],
                default_date_type: DefaultDateType::Modified,
                theme: ThemeConfig {
                    font_origin: "googleFonts".into(),
                    cdn_caching: true,
                    typography: ThemeFonts {
                        header: "Schibsted Grotesk".into(),
                        body: "Source Sans Pro".into(),
                        code: "IBM Plex Mono".into(),
                    },
                    colors: ThemeMode {
                        light_mode: ThemePalette {
                            light: "#faf8f8".into(),
                            lightgray: "#e5e5e5".into(),
                            gray: "#b8b8b8".into(),
                            darkgray: "#4e4e4e".into(),
                            dark: "#2b2b2b".into(),
                            secondary: "#284b63".into(),
                            tertiary: "#84a59d".into(),
                            highlight: "rgba(143, 159, 169, 0.15)".into(),
                            text_highlight: "#fff23688".into(),
                        },
                        dark_mode: ThemePalette {
                            light: "#161618".into(),
                            lightgray: "#393639".into(),
                            gray: "#646464".into(),
                            darkgray: "#d4d4d4".into(),
                            dark: "#ebebec".into(),
                            secondary: "#7b97aa".into(),
                            tertiary: "#84a59d".into(),
                            highlight: "rgba(143, 159, 169, 0.15)".into(),
                            text_highlight: "#b3aa0288".into(),
                        },
                    },
                },
            },
            layout: LayoutConfig::default(),
            plugins: PluginConfig::default(),
            server: ServerConfig::default(),
            paths: PathsConfig::default(),
        }
    }
}

impl SiteConfig {
    /// Load configuration from `config.yml` (if present) and environment variables.
    /// Falls back to the compiled-in defaults when parsing fails.
    pub fn load() -> Self {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.yml");
        let mut builder = SiteConfig::builder();

        if config_path.exists() {
            builder.override_with(YamlFileSource::new(config_path));
        }

        builder.override_with(EnvSource::new());

        match builder.try_build() {
            Ok(cfg) => cfg,
            Err(err) => {
                log::warn!("Failed to load config.yml or env overrides: {err}. Using defaults.");
                SiteConfig::default()
            }
        }
    }
}

pub fn google_font_href(theme: &ThemeConfig) -> String {
    let typography = &theme.typography;
    let code = &typography.code;
    let header = &typography.header;
    let body = &typography.body;

    format!(
        "https://fonts.googleapis.com/css2?family={}
  &family={}:wght@400;700&family={}:ital,wght@0,400;0,600;1,400;1,600&display=swap",
        code, header, body
    )
}

/// Stable hash of the active theme configuration, used for cache busting.
pub fn theme_hash(theme: &ThemeConfig) -> String {
    let json = serde_json::to_string(theme).unwrap_or_default();
    format!("{:x}", Sha256::digest(json.as_bytes()))
}

fn join_segments(base: &str, tail: &str) -> String {
    if base.is_empty() || base == "." {
        return tail.to_string();
    }

    let base = base.trim_end_matches('/');
    let tail = tail.trim_start_matches('/');

    if tail.is_empty() {
        base.to_string()
    } else {
        format!("{}/{}", base, tail)
    }
}

/// Build the per-page resource list, mirroring Quartz's `pageResources` helper.
/// `base_dir` should be the relative path from the current page to the site root (e.g., ".", "..", "../../").
pub fn page_resources(base_dir: &str, static_resources: &ComponentResources) -> ComponentResources {
    let content_index_path = join_segments(base_dir, "static/content-index.json");
    let content_index_script = format!(
        "const fetchData = fetch(\"{}\").then(data => data.json())",
        content_index_path
    );

    let mut css = vec![CssResource {
        content: join_segments(base_dir, "index.css"),
        inline: false,
        spa_preserve: false,
    }];
    css.extend(static_resources.css.clone());

    let mut js = vec![
        JsResource::external(
            JsLoadTime::BeforeDomReady,
            join_segments(base_dir, "prescript.js"),
        ),
        JsResource {
            load_time: JsLoadTime::BeforeDomReady,
            content_type: JsContentType::Inline,
            module_type: None,
            spa_preserve: true,
            src: None,
            script: Some(content_index_script),
        },
    ];

    js.extend(static_resources.js.clone());

    let mut postscript = JsResource::external(
        JsLoadTime::AfterDomReady,
        join_segments(base_dir, "postscript.js"),
    );
    postscript.module_type = Some("module".into());
    js.push(postscript);

    ComponentResources {
        css,
        js,
        additional_head: static_resources.additional_head.clone(),
    }
}

mod yaml {
    use std::error::Error;
    use std::path::PathBuf;

    use confik::Source;
    use serde::de::DeserializeOwned;
    use serde_yaml;

    #[derive(Debug)]
    pub struct YamlFileSource {
        path: PathBuf,
    }

    impl YamlFileSource {
        pub fn new(path: impl Into<PathBuf>) -> Self {
            Self { path: path.into() }
        }
    }

    impl<T> Source<T> for YamlFileSource
    where
        T: DeserializeOwned + confik::ConfigurationBuilder,
    {
        fn allows_secrets(&self) -> bool {
            false
        }

        fn provide(&self) -> Result<T, Box<dyn Error + Sync + Send>> {
            let contents = std::fs::read_to_string(&self.path)?;
            let parsed = serde_yaml::from_str(&contents)?;
            Ok(parsed)
        }
    }
}
