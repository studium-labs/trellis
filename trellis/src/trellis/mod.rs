pub mod bundler;
pub mod cache;
pub mod config;
pub mod content_index;
pub mod layout;
pub mod plugins;
pub mod renderer;
pub mod styles;
pub mod types;

use std::sync::OnceLock;

pub use config::SiteConfig;
pub use renderer::TrellisEngine;

pub fn trellis_engine() -> &'static TrellisEngine {
    static ENGINE: OnceLock<TrellisEngine> = OnceLock::new();
    ENGINE.get_or_init(|| TrellisEngine::new(SiteConfig::load()).expect("init quartz engine"))
}
