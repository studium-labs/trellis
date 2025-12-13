use std::collections::HashMap;
use std::sync::Mutex;

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{
        Aead,
        generic_array::{GenericArray, typenum::U12},
    },
};
use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use getrandom::fill;
use once_cell::sync::Lazy;
use pbkdf2::pbkdf2_hmac_array;
use sha2::{Digest, Sha256};

use crate::trellis::types::Page;

use super::traits::Transformer;

const PBKDF2_ITERATIONS: u32 = 120_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Clone)]
struct CachedCipher {
    ciphertext_b64: String,
    salt_b64: String,
    nonce_b64: String,
}

static ENCRYPT_CACHE: Lazy<Mutex<HashMap<String, CachedCipher>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Encrypt note bodies when a `password` frontmatter key is present.
/// The password itself is stripped from the rendered page; only cipher data
/// is emitted to be decrypted client-side via WebCrypto.
pub struct EncryptContent;

impl Transformer for EncryptContent {
    fn transform(&self, mut page: Page) -> Result<Page> {
        let Some(password_val) = &page.frontmatter.password else {
            return Ok(page);
        };

        let password = password_val.as_str();

        let Some(plaintext_html) = &page.html else {
            // We expect MarkdownRenderer to run before this transformer.
            return Ok(page);
        };

        // Backend cache so repeated renders don't re-encrypt unchanged notes.
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(plaintext_html.as_bytes());
        hasher.update(PBKDF2_ITERATIONS.to_le_bytes());
        hasher.update(b"AES-256-GCM:v1");
        let cache_key = format!("{:x}", hasher.finalize());

        let cached = ENCRYPT_CACHE
            .lock()
            .ok()
            .and_then(|map| map.get(&cache_key).cloned());

        let (ciphertext_b64, salt_b64, nonce_b64) = if let Some(hit) = cached {
            (hit.ciphertext_b64, hit.salt_b64, hit.nonce_b64)
        } else {
            let mut salt = [0u8; SALT_LEN];
            fill(&mut salt).map_err(|e| anyhow!("random salt failed: {e}"))?;

            let key =
                pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), &salt, PBKDF2_ITERATIONS);

            let mut nonce = [0u8; NONCE_LEN];
            fill(&mut nonce).map_err(|e| anyhow!("random nonce failed: {e}"))?;

            let cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| anyhow!("failed to create cipher: {e}"))?;

            let nonce_ga = GenericArray::<u8, U12>::from_slice(&nonce);

            let ciphertext = cipher
                .encrypt(nonce_ga, plaintext_html.as_bytes())
                .map_err(|e| anyhow!("encrypting protected note: {e}"))?;

            let ciphertext_b64 = B64.encode(ciphertext);
            let salt_b64 = B64.encode(salt);
            let nonce_b64 = B64.encode(nonce_ga);

            if let Ok(mut map) = ENCRYPT_CACHE.lock() {
                map.insert(
                    cache_key,
                    CachedCipher {
                        ciphertext_b64: ciphertext_b64.clone(),
                        salt_b64: salt_b64.clone(),
                        nonce_b64: nonce_b64.clone(),
                    },
                );
            }

            (ciphertext_b64, salt_b64, nonce_b64)
        };

        // Prevent leaking the password into the rendered page.
        page.frontmatter.password = Some(String::new());

        // Preserve an approximate word count for read-time calculations.
        let word_count = plaintext_html.split_whitespace().count() as u64;
        page.frontmatter.word_count = Some(word_count);
        page.frontmatter.encrypted = Some(true);

        page.html = Some(format!(
            r#"<div class="encrypted-note" data-ciphertext="{ciphertext}" data-salt="{salt}" data-nonce="{nonce}" data-iterations="{iterations}" data-algo="AES-256-GCM" data-kdf="PBKDF2-SHA256" data-version="1">
  <div class="encrypted-note__chrome">
    <div class="encrypted-note__status">Protected note · Enter the password to decrypt locally.</div>
    <form class="encrypted-note__form" novalidate>
      <div class="encrypted-note__field">
        <input class="encrypted-note__input" type="password" name="password" autocomplete="current-password" placeholder=" " required />
        <label class="encrypted-note__label">Password</label>
      </div>
      <div class="encrypted-note__actions">
        <button type="submit">Decrypt</button>
      </div>
    </form>
  </div>
  <div class="encrypted-note__decode" aria-live="polite"></div>
  <div class="encrypted-note__body" hidden></div>
</div>"#,
            ciphertext = ciphertext_b64,
            salt = salt_b64,
            nonce = nonce_b64,
            iterations = PBKDF2_ITERATIONS,
        ));

        Ok(page)
    }
}
