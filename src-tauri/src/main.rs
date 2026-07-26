mod commands;
mod core;
mod db;

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use crate::core::{assets, indexer, models::VideoPlaylistPreview, vault};
use tauri::{
    http::{header, Response, StatusCode},
    Manager,
};

#[cfg(not(dev))]
use tauri::{ipc::CapabilityBuilder, Url};

pub struct AppState {
    vault_path: Mutex<PathBuf>,
    db_path: PathBuf,
    playlist_cache: Mutex<HashMap<String, VideoPlaylistPreview>>,
}

impl AppState {
    fn new(vault_path: PathBuf, db_path: PathBuf) -> Self {
        Self {
            vault_path: Mutex::new(vault_path),
            db_path,
            playlist_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

fn main() {
    #[cfg(not(dev))]
    let localhost_port = portpicker::pick_unused_port().expect("failed to find a local app port");
    let builder = tauri::Builder::default();

    // YouTube rejects embedded playback from Tauri's production custom protocol
    // (Error 153: missing an HTTP referrer). In packaged builds, serve the app on
    // a random loopback-only HTTP origin so the player receives a valid referrer.
    #[cfg(not(dev))]
    let builder = builder.plugin(
        tauri_plugin_localhost::Builder::new(localhost_port)
            .host("127.0.0.1")
            .build(),
    );

    builder
        .register_uri_scheme_protocol("courselib-asset", |context, request| {
            let app = context.app_handle();
            let state = app.state::<AppState>();
            let vault_path = match state.vault_path.lock() {
                Ok(path) => path.clone(),
                Err(_) => return asset_error_response(StatusCode::INTERNAL_SERVER_ERROR),
            };
            match assets::serve(&vault_path, request.uri().path()) {
                Ok((bytes, media_type)) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, media_type)
                    .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                    .header("X-Content-Type-Options", "nosniff")
                    .body(bytes)
                    .unwrap_or_else(|_| asset_error_response(StatusCode::INTERNAL_SERVER_ERROR)),
                Err(_) => asset_error_response(StatusCode::NOT_FOUND),
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let vault_path = vault::load_or_default_vault_path(app.handle())?;
            vault::ensure_vault(&vault_path)?;
            let db_path = db::default_db_path(app.handle())?;
            db::initialize(&db_path)?;
            let mut conn = db::open(&db_path)?;
            indexer::reindex_vault(&mut conn, &vault_path)?;
            app.manage(AppState::new(vault_path, db_path));

            #[cfg(not(dev))]
            {
                let url: Url = format!("http://127.0.0.1:{localhost_port}").parse()?;
                app.add_capability(
                    CapabilityBuilder::new("loopback-app")
                        .remote(url.to_string())
                        .local(false)
                        .window("main")
                        .permission("core:default")
                        .permission("dialog:default")
                        .permission("app-commands"),
                )?;
                app.get_webview_window("main")
                    .ok_or_else(|| anyhow::anyhow!("main window was not created"))?
                    .navigate(url)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::set_vault_path,
            commands::import_course,
            commands::fetch_youtube_playlist,
            commands::import_video_course,
            commands::delete_course,
            commands::list_courses,
            commands::get_course,
            commands::get_section,
            commands::update_course_meta,
            commands::list_categories,
            commands::create_category,
            commands::rename_category,
            commands::delete_category,
            commands::list_paths,
            commands::create_path,
            commands::get_path,
            commands::add_course_to_path,
            commands::reorder_path_items,
            commands::get_path_progress,
            commands::mark_section_status,
            commands::get_course_progress,
            commands::check_source_drift,
            commands::reimport_course,
            commands::reindex_vault
        ])
        .run(tauri::generate_context!())
        .expect("error while running CourseLib");
}

fn asset_error_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .body(Vec::new())
        .expect("static asset error response should build")
}
