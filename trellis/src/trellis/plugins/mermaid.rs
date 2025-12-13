use once_cell::sync::Lazy;
use regex::Regex;

/// Rewrite fenced ```mermaid code blocks into the HTML structure Quartz expects so that
/// the client-side mermaid script can render and expand them.
pub fn rewrite_mermaid(input: &str) -> String {
    static START: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)^```mermaid\s*$").expect("mermaid start"));
    static END: Lazy<Regex> = Lazy::new(|| Regex::new(r"^```+\s*$").expect("mermaid end"));

    let mut out = String::with_capacity(input.len() + 64);
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.peek() {
        if START.is_match(line) {
            // consume start fence
            let _ = lines.next();
            let mut body = String::new();
            while let Some(next) = lines.peek() {
                if END.is_match(next) {
                    let _ = lines.next(); // consume end fence
                    break;
                }
                body.push_str(lines.next().unwrap_or_default());
                body.push('\n');
            }

            let escaped = html_escape(&body);
            out.push_str(r#"<pre class="mermaid-block">"#);
            out.push_str(
                r#"<button class="expand-button" aria-label="Expand mermaid diagram" data-view-component="true">"#,
            );
            out.push_str(
                r#"<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">"#,
            );
            out.push_str(
                r#"<path fill-rule="evenodd" d="M3.72 3.72a.75.75 0 011.06 1.06L2.56 7h10.88l-2.22-2.22a.75.75 0 011.06-1.06l3.5 3.5a.75.75 0 010 1.06l-3.5 3.5a.75.75 0 11-1.06-1.06l2.22-2.22H2.56l2.22 2.22a.75.75 0 11-1.06 1.06l-3.5-3.5a.75.75 0 010-1.06l3.5-3.5z"></path></svg></button>"#,
            );
            out.push_str(&format!(
                r#"<code class="mermaid" data-clipboard="{}">{}</code>"#,
                escaped, escaped
            ));
            out.push_str(
                r#"<div id="mermaid-container" role="dialog"><div id="mermaid-space"><div class="mermaid-content"></div></div></div></pre>"#,
            );
            out.push('\n');
        } else {
            out.push_str(lines.next().unwrap_or_default());
            out.push('\n');
        }
    }

    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
