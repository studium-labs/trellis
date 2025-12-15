use anyhow::{Result, anyhow};

use crate::trellis::types::Page;

use super::callouts;
use super::emojicode;
use super::mermaid;
use super::traits::Transformer;

pub struct MarkdownRenderer;

impl Transformer for MarkdownRenderer {
    fn transform(&self, mut page: Page) -> Result<Page> {
        //  callout syntax renders callouts instead of plain blockquotes.
        let with_callouts = callouts::rewrite_callouts(&page.content);
        //  emoji shortcodes become their Unicode glyphs before HTML rendering.
        let with_emojis = emojicode::rewrite_emojis(&with_callouts);
        //  mermaid fences become HTML wrappers expected by our inline mermaid script.
        let with_mermaid = mermaid::rewrite_mermaid(&with_emojis);

        let rendered = markdown::to_html_with_options(
            &with_mermaid,
            &markdown::Options {
                parse: markdown::ParseOptions::gfm(),
                compile: markdown::CompileOptions {
                    allow_dangerous_html: true,
                    ..markdown::CompileOptions::gfm()
                },
            },
        )
        .map_err(|e| anyhow!(e.to_string()))?;
        page.html = Some(rendered);
        Ok(page)
    }
}
