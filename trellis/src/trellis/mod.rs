pub mod bundler;
pub mod cache;
pub mod config;
pub mod layout;
pub mod plugins;
pub mod renderer;
pub mod types;
pub mod styles;

use std::sync::OnceLock;

pub use config::SiteConfig;
pub use layout::{default_content_page_layout, default_list_page_layout, shared_layout};
pub use renderer::TrellisEngine;
pub use types::{PageContext, RenderedPage};

pub fn trellis_engine() -> &'static TrellisEngine {
    static ENGINE: OnceLock<TrellisEngine> = OnceLock::new();
    ENGINE.get_or_init(|| TrellisEngine::new(SiteConfig::load()).expect("init quartz engine"))
}
