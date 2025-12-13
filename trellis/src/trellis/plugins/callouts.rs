use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

static CALLOUT_RE: Lazy<Regex> = Lazy::new(|| {
    // Allow leading whitespace before the blockquote marker so indented callouts match.
    Regex::new(r"(?i)^\s*>\s*\[\!(?P<kind>[\w-]+)(?:\|(?P<meta>[^\]]+))?\](?P<collapse>[+-]?)(?:\s+(?P<title>.*))?$")
        .expect("callout regex")
});

static CALLOUT_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("note", "note"),
        ("abstract", "abstract"),
        ("summary", "abstract"),
        ("tldr", "abstract"),
        ("info", "info"),
        ("todo", "todo"),
        ("tip", "tip"),
        ("hint", "tip"),
        ("important", "tip"),
        ("success", "success"),
        ("check", "success"),
        ("done", "success"),
        ("question", "question"),
        ("help", "question"),
        ("faq", "question"),
        ("warning", "warning"),
        ("attention", "warning"),
        ("caution", "warning"),
        ("failure", "failure"),
        ("missing", "failure"),
        ("fail", "failure"),
        ("danger", "danger"),
        ("error", "danger"),
        ("bug", "bug"),
        ("example", "example"),
        ("quote", "quote"),
        ("cite", "quote"),
    ])
});

pub fn rewrite_callouts(input: &str) -> String {
    let mut output = String::with_capacity(input.len() + 128);
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.peek() {
        if let Some(cap) = CALLOUT_RE.captures(line) {
            // consume first line
            let _ = lines.next();

            let kind_raw = cap
                .name("kind")
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_lowercase();
            let kind = canonicalize(&kind_raw);
            let meta = cap.name("meta").map(|m| m.as_str()).unwrap_or("").trim();
            let collapse = cap.name("collapse").map(|m| m.as_str()).unwrap_or("");
            let title = cap
                .name("title")
                .map(|m| m.as_str().trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| capitalize(&kind_raw));

            // Gather content lines while they keep starting with '>' (optional leading spaces allowed)
            let mut content_md = String::new();
            while let Some(next_line) = lines.peek() {
                if !next_line.trim_start().starts_with('>') {
                    break;
                }
                let raw = lines.next().unwrap_or_default();
                let stripped = raw
                    .trim_start()
                    .trim_start_matches('>')
                    .trim_start_matches(' ')
                    .to_string();
                content_md.push_str(&stripped);
                content_md.push('\n');
            }

            let inner_html = render_md(&content_md);
            let title_html = render_md(&title);

            let collapsible = collapse == "+" || collapse == "-";
            let collapsed = collapse == "-";

            let mut class_list = vec!["callout".to_string(), kind.clone()];
            if collapsible {
                class_list.push("is-collapsible".into());
            }
            if collapsed {
                class_list.push("is-collapsed".into());
            }

            let data_fold = if collapsed {
                "true"
            } else if collapsible {
                "false"
            } else {
                "false"
            };

            // Ensure raw HTML block is separated so markdown doesn't wrap it in <p>.
            if !output.ends_with('\n') {
                output.push('\n');
            }

            output.push_str(&format!(
                r#"<div class="{classes}" data-callout="{kind}" data-callout-fold="{fold}" data-callout-metadata="{meta}">"#,
                classes = class_list.join(" "),
                kind = kind,
                fold = data_fold,
                meta = html_escape(meta),
            ));
            output.push_str(r#"<div class="callout-title">"#);
            output.push_str(r#"<div class="callout-icon"></div>"#);
            output.push_str(r#"<div class="callout-title-inner">"#);
            output.push_str(&title_html);
            output.push_str("</div>");
            if collapsible {
                output.push_str(r#"<div class="fold-callout-icon"></div>"#);
            }
            output.push_str("</div>"); // callout-title
            output.push_str(r#"<div class="callout-content"><div class="callout-content-inner">"#);
            output.push_str(&inner_html);
            output.push_str("</div></div></div>");
            output.push_str("\n\n");
            continue;
        }

        // default: passthrough line
        output.push_str(lines.next().unwrap_or_default());
        output.push('\n');
    }

    output
}

fn canonicalize(kind: &str) -> String {
    let key = kind.to_lowercase();
    CALLOUT_MAP
        .get(key.as_str())
        .copied()
        .unwrap_or(key.as_str())
        .to_string()
}

fn capitalize(s: &str) -> String {
    if s.is_empty() {
        return "Note".to_string();
    }
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        let mut buf = String::with_capacity(s.len());
        buf.push_str(&first.to_uppercase().collect::<String>());
        buf.push_str(chars.as_str());
        return buf;
    }
    "Note".to_string()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_md(src: &str) -> String {
    markdown::to_html_with_options(
        src,
        &markdown::Options {
            parse: markdown::ParseOptions::gfm(),
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                ..markdown::CompileOptions::gfm()
            },
        },
    )
    .unwrap_or_default()
}
