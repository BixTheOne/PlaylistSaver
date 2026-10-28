mod commands;
mod db;
mod library;
mod queue;
mod settings;
mod sources;
mod state;
mod tools;
mod util;
mod ytdlp;

use std::sync::{Arc, Mutex, RwLock};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

use settings::Settings;
use state::{AppState, Paths, QueueState, StopReason, ToolsInstall};
use tools::Tools;

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn quit(app: &AppHandle) {
    // Running downloads go back to the queue and resume on next launch.
    app.state::<Arc<AppState>>().stop_all(StopReason::Requeue);
    app.exit(0);
}

fn create_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir PlaylistSaver", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &exit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("PlaylistSaver")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let paths = Paths {
                settings_file: config_dir.join("settings.json"),
                default_temp: data_dir.join("downloads"),
            };
            let settings = Settings::load(&paths.settings_file);
            let conn = db::open(&data_dir.join("library.db"))?;
            let state = Arc::new(AppState {
                tools: Tools::new(data_dir.join("bin")),
                db: Mutex::new(conn),
                settings: RwLock::new(settings),
                queue: QueueState::default(),
                tools_install: Mutex::new(ToolsInstall::default()),
                paths,
            });
            app.manage(state.clone());
            create_tray(app)?;

            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            let handle = app.handle().clone();
            queue::start(handle.clone(), state.clone());
            sources::start_scheduler(handle.clone(), state.clone());
            tauri::async_runtime::spawn(async move {
                if !state.tools.ready() {
                    commands::install_tools_inner(&handle, &state).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Arc<AppState>>();
                if state.settings().close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    state.stop_all(StopReason::Requeue);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::tools_status,
            commands::install_tools,
            commands::tools_versions,
            commands::update_ytdlp,
            commands::fetch_url,
            commands::add_source,
            commands::list_playlists,
            commands::get_playlist,
            commands::update_playlist,
            commands::delete_playlist,
            commands::sync_playlist,
            commands::download_playlist,
            commands::list_queue,
            commands::queue_videos,
            commands::pause_video,
            commands::remove_from_queue,
            commands::set_queue_paused,
            commands::open_playlist_folder,
            commands::test_jellyfin,
            commands::refresh_jellyfin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
