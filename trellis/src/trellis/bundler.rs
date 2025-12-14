use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Error, Result};
use log::error;
use serde::Serialize;
use swc_atoms::Atom;
use swc_bundler::{Bundle, Bundler, Config, Hook, Load, ModuleData, ModuleType, Resolve};
use swc_common::errors::{EmitterWriter, Handler};
use swc_common::{FileName, GLOBALS, Globals, Mark, SourceMap, sync::Lrc};
use swc_ecma_ast::{EsVersion, KeyValueProp, Module, Program};
use swc_ecma_codegen::{Emitter, text_writer::JsWriter};
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_transforms_base::helpers::Helpers;
use swc_ecma_transforms_base::resolver;
use swc_ecma_transforms_typescript::strip_type;
use swc_ecma_visit::VisitMutWith;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptKind {
    Explorer,
    OverlayExplorer,
    EncryptedNote,
    Mermaid,
    Callouts,
    Graph,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct InlineScripts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_explorer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mermaid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callouts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScriptNeeds {
    pub explorer: bool,
    pub overlay_explorer: bool,
    pub encrypted_note: bool,
    pub mermaid: bool,
    pub callouts: bool,
    pub graph: bool,
}

pub fn inline_scripts(needs: ScriptNeeds) -> InlineScripts {
    static CACHE: OnceLock<RwLock<ScriptsCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        RwLock::new(ScriptsCache {
            bundles: HashMap::new(),
            mtime: UNIX_EPOCH,
        })
    });

    let newest_mtime = newest_templates_mtime().unwrap_or(UNIX_EPOCH);
    {
        if let Ok(cache_guard) = cache.read() {
            if cache_guard.mtime >= newest_mtime {
                return InlineScripts {
                    explorer: needs
                        .explorer
                        .then(|| cache_guard.bundles.get(&ScriptKind::Explorer).cloned())
                        .flatten(),
                    overlay_explorer: needs
                        .overlay_explorer
                        .then(|| {
                            cache_guard
                                .bundles
                                .get(&ScriptKind::OverlayExplorer)
                                .cloned()
                        })
                        .flatten(),
                    encrypted_note: needs
                        .encrypted_note
                        .then(|| cache_guard.bundles.get(&ScriptKind::EncryptedNote).cloned())
                        .flatten(),
                    mermaid: needs
                        .mermaid
                        .then(|| cache_guard.bundles.get(&ScriptKind::Mermaid).cloned())
                        .flatten(),
                    callouts: needs
                        .callouts
                        .then(|| cache_guard.bundles.get(&ScriptKind::Callouts).cloned())
                        .flatten(),
                    graph: needs
                        .graph
                        .then(|| cache_guard.bundles.get(&ScriptKind::Graph).cloned())
                        .flatten(),
                };
            }
        }
    }

    match build_all_bundles() {
        Ok((bundles, mtime)) => {
            if let Ok(mut cache_guard) = cache.write() {
                cache_guard.bundles = bundles.clone();
                cache_guard.mtime = mtime;
            }
            InlineScripts {
                explorer: needs
                    .explorer
                    .then(|| bundles.get(&ScriptKind::Explorer).cloned())
                    .flatten(),
                overlay_explorer: needs
                    .overlay_explorer
                    .then(|| bundles.get(&ScriptKind::OverlayExplorer).cloned())
                    .flatten(),
                encrypted_note: needs
                    .encrypted_note
                    .then(|| bundles.get(&ScriptKind::EncryptedNote).cloned())
                    .flatten(),
                mermaid: needs
                    .mermaid
                    .then(|| bundles.get(&ScriptKind::Mermaid).cloned())
                    .flatten(),
                callouts: needs
                    .callouts
                    .then(|| bundles.get(&ScriptKind::Callouts).cloned())
                    .flatten(),
                graph: needs
                    .graph
                    .then(|| bundles.get(&ScriptKind::Graph).cloned())
                    .flatten(),
            }
        }
        Err(err) => {
            error!("failed to build inline scripts: {err:?}");
            InlineScripts::default()
        }
    }
}

struct ScriptsCache {
    bundles: HashMap<ScriptKind, String>,
    mtime: SystemTime,
}

fn newest_templates_mtime() -> Result<SystemTime> {
    let templates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    let util = templates.join("util");
    let components = templates.join("components/scripts");
    let util_mtime = newest_mtime_in(&util)?;
    let comp_mtime = newest_mtime_in(&components)?;
    Ok(util_mtime.max(comp_mtime))
}

fn newest_mtime_in(dir: &Path) -> Result<SystemTime> {
    let mut newest = UNIX_EPOCH;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            if let Ok(mtime) = metadata.modified() {
                if mtime > newest {
                    newest = mtime;
                }
            }
        }
    }
    Ok(newest)
}

fn build_all_bundles() -> Result<(HashMap<ScriptKind, String>, SystemTime)> {
    let templates_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    let component_root = templates_root.join("components/scripts");

    let entries = vec![
        (
            ScriptKind::Explorer,
            component_root.join("explorer.inline.ts"),
        ),
        (
            ScriptKind::OverlayExplorer,
            component_root.join("overlay-explorer.inline.ts"),
        ),
        (
            ScriptKind::EncryptedNote,
            component_root.join("encrypted-note.inline.ts"),
        ),
        (
            ScriptKind::Mermaid,
            component_root.join("mermaid.inline.ts"),
        ),
        (
            ScriptKind::Callouts,
            component_root.join("callouts.inline.ts"),
        ),
        (ScriptKind::Graph, component_root.join("graph.inline.ts")),
    ];

    let mut bundles = HashMap::new();
    let mut latest = newest_templates_mtime().unwrap_or(UNIX_EPOCH);

    for (kind, path) in entries {
        match bundle_entry(&path) {
            Ok(code) => {
                bundles.insert(kind, code);
                if let Ok(meta) = fs::metadata(&path) {
                    if let Ok(mtime) = meta.modified() {
                        if mtime > latest {
                            latest = mtime;
                        }
                    }
                }
            }
            Err(err) => {
                error!("failed to bundle {:?}: {}", kind, err);
            }
        }
    }

    Ok((bundles, latest))
}

fn bundle_entry(entry: &Path) -> Result<String> {
    let cm: Lrc<SourceMap> = Default::default();
    let globals = Globals::new();
    let loader = FsLoader { cm: cm.clone() };
    let resolver = ScriptResolver::new();
    let hook = Box::new(NoopHook);

    let mut bundler = Bundler::new(
        &globals,
        cm.clone(),
        loader,
        resolver,
        Config {
            require: false,
            disable_hygiene: false,
            disable_fixer: false,
            disable_inliner: true,
            disable_dce: true,
            external_modules: vec![Atom::from(
                "https://cdnjs.cloudflare.com/ajax/libs/mermaid/11.4.0/mermaid.esm.min.mjs",
            )],
            module: ModuleType::Es,
        },
        hook,
    );

    let mut entries = HashMap::new();
    entries.insert(
        entry
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("entry")
            .to_string(),
        FileName::Real(entry.to_path_buf()),
    );

    let bundles: Vec<Bundle> = bundler.bundle(entries)?;

    // Expect first bundle to be our entry (Named)
    let bundled = bundles
        .into_iter()
        .find(|b| matches!(b.kind, swc_bundler::BundleKind::Named { .. }))
        .ok_or_else(|| Error::msg("bundle not produced"))?;

    emit_minified(&bundled.module, cm, &globals)
}

struct NoopHook;
impl Hook for NoopHook {
    fn get_import_meta_props(
        &self,
        _span: swc_common::Span,
        _module_record: &swc_bundler::ModuleRecord,
    ) -> Result<Vec<KeyValueProp>, Error> {
        Ok(vec![])
    }
}

struct ScriptResolver;

impl ScriptResolver {
    fn new() -> Self {
        Self
    }

    fn resolve_spec(&self, spec: &str) -> Result<FileName> {
        let templates_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
        let util_root = templates_root.join("util");
        let scripts_root = templates_root.join("components/scripts");

        let (root, mut rel) = if let Some(rest) = spec.strip_prefix("/js/util/") {
            (util_root, rest.to_string())
        } else if let Some(rest) = spec.strip_prefix("/js/components/scripts/") {
            (scripts_root, rest.to_string())
        } else {
            return Err(Error::msg(format!("unhandled module specifier: {spec}")));
        };

        if !rel.ends_with(".ts") && !rel.ends_with(".js") {
            rel.push_str(".ts");
        } else if rel.ends_with(".js") {
            rel = rel.trim_end_matches(".js").to_string() + ".ts";
        }

        Ok(FileName::Real(root.join(rel)))
    }
}

impl Resolve for ScriptResolver {
    fn resolve(
        &self,
        base: &FileName,
        module_specifier: &str,
    ) -> Result<swc_ecma_loader::resolve::Resolution, Error> {
        let resolved = if module_specifier.starts_with("http://")
            || module_specifier.starts_with("https://")
        {
            FileName::Custom(module_specifier.to_string())
        } else if module_specifier.starts_with("/js/") {
            self.resolve_spec(module_specifier)?
        } else {
            match base {
                FileName::Real(path) => {
                    let parent = path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("."));
                    let mut joined = parent.join(module_specifier);
                    if joined.extension().is_none() {
                        // Try TypeScript first; these sources live alongside.
                        joined.set_extension("ts");
                    } else if joined.extension().map(|e| e == "js").unwrap_or(false) {
                        joined.set_extension("ts");
                    }
                    FileName::Real(joined)
                }
                _ => return Err(Error::msg("unsupported base filename")),
            }
        };

        Ok(swc_ecma_loader::resolve::Resolution {
            filename: resolved,
            slug: None,
        })
    }
}

struct FsLoader {
    cm: Lrc<SourceMap>,
}

impl Load for FsLoader {
    fn load(&self, file: &FileName) -> Result<ModuleData, Error> {
        let path = match file {
            FileName::Real(p) => p.clone(),
            _ => return Err(Error::msg("unsupported filename kind for loader")),
        };

        let emitter = EmitterWriter::new(
            Box::new(std::io::stderr()),
            Some(self.cm.clone()),
            false,
            false,
        );
        let handler = Handler::with_emitter(true, false, Box::new(emitter));

        let fm = self
            .cm
            .load_file(&path)
            .with_context(|| format!("loading script {}", path.display()))?;

        let syntax = Syntax::Typescript(TsSyntax {
            tsx: false,
            decorators: true,
            dts: false,
            ..Default::default()
        });

        let lexer = Lexer::new(syntax, EsVersion::Es2022, StringInput::from(&*fm), None);
        let mut parser = Parser::new_from(lexer);
        let mut module = parser.parse_module().map_err(|e| {
            let mut diag = e.into_diagnostic(&handler);
            diag.emit();
            Error::msg("failed to parse module")
        })?;

        // Strip TypeScript types so downstream bundler passes don't see TS nodes.
        module.visit_mut_with(&mut strip_type());

        Ok(ModuleData {
            fm,
            module,
            helpers: Helpers::new(false),
        })
    }
}

fn emit_minified(module: &Module, cm: Lrc<SourceMap>, globals: &Globals) -> Result<String> {
    GLOBALS.set(globals, || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        // Run resolver to set syntax contexts.
        let mut resolved = module.clone();
        // typescript = true so TS syntax contexts are handled correctly
        resolved.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, true));

        let program = Program::Module(resolved);

        let mut buf = Vec::new();
        {
            let mut cfg = swc_ecma_codegen::Config::default();
            cfg.minify = true;
            cfg.target = EsVersion::Es2016;

            let mut emitter = Emitter {
                cfg,
                comments: None,
                cm: cm.clone(),
                wr: JsWriter::new(cm.clone(), "\n", &mut buf, None),
            };

            emitter.emit_program(&program)?;
        }

        let out = String::from_utf8(buf)?;
        Ok(out)
    })
}
