use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

pub fn cache_path(cache_root: &Path, slug: &str) -> PathBuf {
    let mut path = cache_root.to_path_buf();
    let slug_path = Path::new(slug);
    if let Some(parent) = slug_path.parent() {
        path = path.join(parent);
    }
    let filename = slug_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| slug.to_string());
    path.join(format!("{}.html", filename))
}

pub fn ensure_cache_root(cache_root: &Path) -> io::Result<()> {
    fs::create_dir_all(cache_root)
}

pub fn cache_is_fresh(src: &Path, cached: &Path, deps: &[SystemTime]) -> io::Result<bool> {
    let src_meta = fs::metadata(src)?;
    let cache_meta = fs::metadata(cached)?;

    let src_time = src_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let cache_time = cache_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    let newest_dep = deps.iter().copied().max().unwrap_or(SystemTime::UNIX_EPOCH);

    Ok(cache_time >= src_time && cache_time >= newest_dep)
}

pub fn write_cache(path: &Path, html: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, html)
}

/// Modified time of the current executable (used to bust caches on new builds).
pub fn binary_mtime() -> SystemTime {
    std::env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Find the most recent modification time for files with the given extension under `dir`.
pub fn newest_mtime_with_extension(dir: &Path, ext: &str) -> io::Result<SystemTime> {
    let mut newest = SystemTime::UNIX_EPOCH;
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        let is_match = entry.path().extension().map(|e| e == ext).unwrap_or(false);
        if !is_match {
            continue;
        }

        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified > newest {
                    newest = modified;
                }
            }
        }
    }

    Ok(newest)
}

/// Write or refresh a marker file containing the provided hash and return its mtime.
/// This lets callers include semantic hashes as cache dependencies without parsing HTML.
pub fn update_hash_marker(cache_root: &Path, name: &str, hash: &str) -> io::Result<SystemTime> {
    let marker = cache_root.join(format!(".{name}_hash"));
    let mut needs_write = true;

    if let Ok(existing) = fs::read_to_string(&marker) {
        if existing.trim() == hash {
            needs_write = false;
        }
    }

    if needs_write {
        if let Some(parent) = marker.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&marker, hash)?;
    }

    fs::metadata(&marker)
        .and_then(|m| m.modified())
        .or(Ok(SystemTime::UNIX_EPOCH))
}
