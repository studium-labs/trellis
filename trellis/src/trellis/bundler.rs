use std::path::Path;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use log::error;
use regex::{Captures, Regex};
use serde::Serialize;
use std::collections::BTreeMap;

// TODO: Use `swc_bundler` crate, rewrite JS scripts to be typescript, and deprecate logic here for swc logic

#[derive(Debug, Serialize, Clone)]
pub struct InlineScripts {
    pub import_map: String,
    pub explorer: String,
    pub overlay_explorer: String,
    pub encrypted_note: String,
    pub mermaid: String,
}

impl Default for InlineScripts {
    fn default() -> Self {
        InlineScripts {
            import_map: r#"{"imports":{}}"#.into(),
            explorer: String::new(),
            overlay_explorer: String::new(),
            encrypted_note: String::new(),
            mermaid: String::new(),
        }
    }
}

pub fn inline_scripts() -> InlineScripts {
    static SCRIPTS: OnceLock<RwLock<ScriptsCache>> = OnceLock::new();
    let mtime = UNIX_EPOCH;
    let cache = SCRIPTS.get_or_init(|| {
        RwLock::new(ScriptsCache {
            scripts: InlineScripts::default(),
            mtime: mtime,
        })
    });

    if let Ok(guard) = cache.read() {
        if guard.mtime >= mtime {
            return guard.scripts.clone();
        }
    }

    let compiled = build_inline_scripts().unwrap_or_else(|err| {
        error!("Failed to inline scripts: {err}");
        InlineScripts::default()
    });

    if let Ok(mut guard) = cache.write() {
        guard.scripts = compiled.clone();
        guard.mtime = mtime;
    }

    compiled
}

struct ScriptsCache {
    scripts: InlineScripts,
    mtime: SystemTime,
}

fn build_inline_scripts() -> io::Result<InlineScripts> {
    let templates_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let component_root = templates_root.join("components/scripts");
    let util_root = templates_root.join("util");

    let util_modules = [
        ("/js/util/path", util_root.join("path.js")),
        ("/js/util/fileTrie", util_root.join("fileTrie.js")),
        (
            "/js/util/github-slugger",
            util_root.join("github-slugger.js"),
        ),
        ("/js/util/clone", util_root.join("clone.js")),
        (
            "/js/components/scripts/util.js",
            component_root.join("util.js"),
        ),
        (
            "/js/components/scripts/util",
            component_root.join("util.js"),
        ),
    ];

    let mut imports = BTreeMap::new();
    for (spec, path) in util_modules {
        let code = load_js_module(spec, &path)?;
        imports.insert(spec.to_string(), to_data_url(&code));
    }

    let explorer = load_js_module(
        "/js/components/scripts/explorer.inline.js",
        &component_root.join("explorer.inline.js"),
    )?;
    let overlay = load_js_module(
        "/js/components/scripts/overlay-explorer.inline.js",
        &component_root.join("overlay-explorer.inline.js"),
    )?;
    let encrypted = load_js_module(
        "/js/components/scripts/encrypted-note.inline.js",
        &component_root.join("encrypted-note.inline.js"),
    )?;
    let mermaid = load_js_module(
        "/js/components/scripts/mermaid.inline.js",
        &component_root.join("mermaid.inline.js"),
    )?;

    let import_map = serde_json::json!({ "imports": imports }).to_string();
    let inline_scripts = InlineScripts {
        import_map,
        explorer,
        overlay_explorer: overlay,
        encrypted_note: encrypted,
        mermaid,
    };
    Ok(inline_scripts)
}

fn load_js_module(spec: &str, path: &Path) -> io::Result<String> {
    let source = fs::read_to_string(path)?;
    let normalized = rewrite_relative_imports(&source, spec);
    Ok(minify_js(&normalized))
}

fn rewrite_relative_imports(source: &str, base_spec: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r#"(?P<prefix>\bimport\s+(?:[^'"]*?\s+from\s+)?|\bexport\s+[^'"]*?\s+from\s+|\bimport\s*\(\s*)(?P<quote>["'])(?P<spec>[^"']+)(?P=quote)"#,
        )
        .expect("import regex")
    });

    let base_dir = Path::new(base_spec)
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .to_path_buf();

    re.replace_all(source, |caps: &Captures| {
        let spec = caps.name("spec").map(|m| m.as_str()).unwrap_or("");
        if !spec.starts_with('.') {
            return caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
        }

        let normalized = normalize_spec(&base_dir, spec);
        format!(
            "{}{}{}{}",
            &caps["prefix"], &caps["quote"], normalized, &caps["quote"]
        )
    })
    .into_owned()
}

fn normalize_spec(base_dir: &Path, spec: &str) -> String {
    let joined = base_dir.join(spec);
    let mut normalized = joined.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{}", normalized);
    }
    normalized
}

fn minify_js(source: &str) -> String {
    source
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn to_data_url(js: &str) -> String {
    let encoded = BASE64.encode(js);
    format!("data:application/javascript;base64,{encoded}")
}
