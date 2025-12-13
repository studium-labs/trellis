use std::{
    path::Path,
    sync::{OnceLock, RwLock},
    time::SystemTime,
};

use log::warn;

use crate::trellis::{SiteConfig, cache, config::ThemeConfig};

pub fn compiled_styles(cfg: &SiteConfig) -> String {
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
pub fn compile_scss(cfg: &SiteConfig) -> String {
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
            warn!("Failed to compile SCSS at {:?}: {err}", scss_path);
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
