pub mod handlers;
pub mod trellis;
use std::{env, io};

use actix_cors::Cors;
use actix_web::{App, HttpServer, http::header, middleware::Logger, web};
use handlebars::Handlebars;
use log::info;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use tokio::fs::File;
use walkdir::WalkDir;

use crate::trellis::config::SiteConfig;

pub async fn run() -> io::Result<()> {
    let config = SiteConfig::load();
    let server_cfg = config.server.clone();

    info!(
        "Trellis is listening on: http://{}:{}",
        server_cfg.host, server_cfg.port
    );
    // Build the shared Handlebars registry once for all workers.
    let handlebars = build_handlebars();

    let pool = get_db_pool()
        .await
        .expect("unable to connect to sqlite db!");
    let max_bytes = server_cfg.max_payload_bytes();
    let cors_origins = server_cfg.cors_origins.clone();

    HttpServer::new(move || {
        // Set payload limit based on configuration (affects multipart and others)
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(handlebars.clone()))
            .app_data(web::PayloadConfig::new(max_bytes))
            // .app_data(web::Data::new())
            .wrap(build_cors(&cors_origins))
            .configure(handlers::config)
    })
    .bind((server_cfg.host.as_str(), server_cfg.port))?
    .run()
    .await
}

fn build_cors(origins: &[String]) -> Cors {
    let base = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
        .allowed_headers(vec![header::CONTENT_TYPE, header::ACCEPT]);

    if origins.iter().any(|o| o == "*") {
        return base.allow_any_origin();
    }

    let cors = origins
        .iter()
        .fold(base, |c, origin| c.allowed_origin(origin));

    // Maintain previous behavior of allowing credentials when specific origins are set.
    cors.supports_credentials()
}

fn build_handlebars() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();

    // Register every .hbs file under ./api/templates so they are all available to handlers.
    let templates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");

    for entry in WalkDir::new(&templates_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file() && e.path().extension() == Some(OsStr::new("hbs")))
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(&templates_dir)
            .expect("template path prefix");
        let rel_no_ext = rel.with_extension("");
        let name = rel_no_ext.to_string_lossy().replace('\\', "/");

        if rel.parent().map(|p| p == Path::new("")).unwrap_or(true) {
            // top-level templates (e.g., index, page, feed)
            let stem = rel_no_ext
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("template");
            handlebars
                .register_template_file(stem, &path)
                .unwrap_or_else(|e| panic!("failed to register template {}: {}", stem, e));
        } else {
            // nested templates treated as partials (e.g., components/...)
            let partial_src = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read partial {}: {}", name, e));
            handlebars
                .register_partial(name.as_str(), partial_src)
                .unwrap_or_else(|e| panic!("failed to register partial {}: {}", name, e));
        }
    }

    handlebars
}

pub async fn get_db_pool() -> anyhow::Result<SqlitePool> {
    // Allow override via env (works for deployments that supply DATABASE_URL)
    let url = env::var("DATABASE_URL")
        .or_else(|_| env::var("TRELLIS_DATABASE_URL"))
        .unwrap_or_else(|_| {
            let mut path = env::current_dir().expect("cwd");
            path.push("trellis.db");
            // SQLx expects three slashes for an absolute path (sqlite:///...)
            // otherwise it treats it as relative and the connection can fail.
            path.display().to_string()
        });
    let uri = format!("sqlite://{}", &url);

    let db_path = std::path::PathBuf::from(&url);
    // If we're pointing at a file-based SQLite DB, ensure the file exists
    // so SQLx doesn't return code 14 ("unable to open database file").
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    if tokio::fs::metadata(&db_path).await.is_err() {
        File::create(&db_path).await?;
    }

    info!("Opening SQLite at {}", &uri);
    let pool = SqlitePoolOptions::new().connect(&uri).await?;
    Ok(pool)
}
