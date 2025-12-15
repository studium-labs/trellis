use once_cell::sync::Lazy;
use regex::{Captures, Regex};

/// Replace Discord/Gemoji-style `:shortcode:` tokens with their Unicode emoji.
/// Falls back to the original text when no shortcode exists.
pub fn rewrite_emojis(input: &str) -> String {
    // Matches :shortcode: with lowercase letters, digits, underscores, plus, or minus.
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r":([a-z0-9_+\-]+):").expect("emoji shortcode regex"));

    RE.replace_all(input, |caps: &Captures| {
        let code = &caps[1];
        emojis::get_by_shortcode(code)
            .map(|e| e.as_str())
            .unwrap_or_else(|| caps.get(0).unwrap().as_str())
            .to_string()
    })
    .into_owned()
}
