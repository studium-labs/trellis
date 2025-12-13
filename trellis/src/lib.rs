pub mod handlers;
pub mod trellis;
use std::io;

use actix_cors::Cors;
use actix_web::{App, HttpServer, http::header, middleware::Logger, web};
use futures_util::future::BoxFuture;
use handlebars::Handlebars;
use log::{error, info};

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn db_pool() -> BoxFuture<'static, SqlitePool> {
    let attempts = 0;
    Box::pin(async move {
        // debug!("Connecting to PostgreSQL using URI: {}", &cfg.database_url);
        match SqlitePoolOptions::new().connect("studium.db").await {
            Ok(pool) => {
                info!("SQLite connection found!");
                pool
            }
            Err(err) => {
                let attempts = attempts + 1;
                error!(
                    "Error connecting to db pool, {:?} attempts failed. error: {}:",
                    attempts, err
                );
                return db_pool().await;
            }
        }
    })
}

pub async fn run() -> io::Result<()> {
    info!(
        "studium.dev is listening on: http://{}:{}",
        "0.0.0.0", 40075
    );
    // Build the shared Handlebars registry once for all workers.
    let handlebars = web::Data::new(build_handlebars());

    HttpServer::new(move || {
        // Set payload limit based on configuration (affects multipart and others)
        let max_mb = 100;
        let max_bytes = (max_mb as usize).saturating_mul(1024 * 1024);
        let pool = db_pool();
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(pool))
            .app_data(handlebars.clone())
            .app_data(web::PayloadConfig::new(max_bytes))
            // .app_data(web::Data::new())
            .wrap(
                Cors::default()
                    .allowed_origin("0.0.0.0:40075")
                    .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE"])
                    .allowed_headers(vec![header::CONTENT_TYPE, header::ACCEPT])
                    .supports_credentials(),
            )
            .configure(handlers::config)
    })
    .bind(("0.0.0.0", 40075))?
    .run()
    .await
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
