use std::path::Path;

use confik::{Configuration, EnvSource};
use serde::{Deserialize, Serialize};

use self::yaml::YamlFileSource;
use crate::trellis::layout::LayoutConfig;

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

/// CSS variable declarations derived from the active theme, mirroring Quartz's joinStyles.
pub fn theme_css_variables(theme: &ThemeConfig) -> String {
    const DEFAULT_SANS: &str = "system-ui, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif, \"Apple Color Emoji\", \"Segoe UI Emoji\", \"Segoe UI Symbol\"";
    const DEFAULT_MONO: &str = "ui-monospace, SFMono-Regular, SF Mono, Menlo, monospace";

    format!(
        r#"
:root {{
  --light: {l_light};
  --lightgray: {l_lightgray};
  --gray: {l_gray};
  --darkgray: {l_darkgray};
  --dark: {l_dark};
  --secondary: {l_secondary};
  --tertiary: {l_tertiary};
  --highlight: {l_highlight};
  --textHighlight: {l_text_highlight};

  --titleFont: "{title}", {sans};
  --headerFont: "{header}", {sans};
  --bodyFont: "{body}", {sans};
  --codeFont: "{code}", {mono};
}}

:root[saved-theme=\"dark\"] {{
  --light: {d_light};
  --lightgray: {d_lightgray};
  --gray: {d_gray};
  --darkgray: {d_darkgray};
  --dark: {d_dark};
  --secondary: {d_secondary};
  --tertiary: {d_tertiary};
  --highlight: {d_highlight};
  --textHighlight: {d_text_highlight};
}}
"#,
        l_light = theme.colors.light_mode.light,
        l_lightgray = theme.colors.light_mode.lightgray,
        l_gray = theme.colors.light_mode.gray,
        l_darkgray = theme.colors.light_mode.darkgray,
        l_dark = theme.colors.light_mode.dark,
        l_secondary = theme.colors.light_mode.secondary,
        l_tertiary = theme.colors.light_mode.tertiary,
        l_highlight = theme.colors.light_mode.highlight,
        l_text_highlight = theme.colors.light_mode.text_highlight,
        d_light = theme.colors.dark_mode.light,
        d_lightgray = theme.colors.dark_mode.lightgray,
        d_gray = theme.colors.dark_mode.gray,
        d_darkgray = theme.colors.dark_mode.darkgray,
        d_dark = theme.colors.dark_mode.dark,
        d_secondary = theme.colors.dark_mode.secondary,
        d_tertiary = theme.colors.dark_mode.tertiary,
        d_highlight = theme.colors.dark_mode.highlight,
        d_text_highlight = theme.colors.dark_mode.text_highlight,
        title = theme.typography.header,
        header = theme.typography.header,
        body = theme.typography.body,
        code = theme.typography.code,
        sans = DEFAULT_SANS,
        mono = DEFAULT_MONO
    )
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
/// `base_dir` should be the relative path from the current page to the site root
/// (e.g., ".", "..", "../../").
pub fn page_resources(base_dir: &str, static_resources: &ComponentResources) -> ComponentResources {
    let content_index_path = join_segments(base_dir, "static/contentIndex.json");
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
