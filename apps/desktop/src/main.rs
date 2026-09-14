#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod features;
mod session;
use tauri::Emitter;
use tauri_plugin_deep_link::DeepLinkExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            features::focus(app)
        }))
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("navigation")
                .on_navigation(|_, url| {
                    url.scheme() == "tauri"
                        || matches!(url.host_str(), Some("tauri.localhost"))
                        || (cfg!(debug_assertions)
                            && url.origin().ascii_serialization() == "http://localhost:5173")
                })
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("HTTP client"),
        )
        .manage(features::PendingUpdate::default())
        .register_asynchronous_uri_scheme_protocol("den-media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(session::media(app, request).await);
            });
        })
        .invoke_handler(tauri::generate_handler![
            session::session_get,
            session::session_set,
            session::session_clear,
            session::instances_get,
            session::instances_set,
            session::api_request,
            features::platform,
            features::ptt_register,
            features::notify,
            features::badge,
            features::tray_state,
            features::deep_links,
            features::update_check,
            features::update_restart
        ])
        .setup(|app| {
            features::tray(app.handle())?;
            #[cfg(any(target_os = "linux", windows))]
            app.deep_link().register_all()?;
            let handle = app.handle().clone();
            app.deep_link()
                .on_open_url(move |_| features::focus(&handle));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
            if let tauri::WindowEvent::Focused(false) = event {
                let _ = window.emit("window-background", ());
            }
        })
        .build(tauri::generate_context!())
        .expect("Cannot start Den")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                features::focus(app);
            }
            let _ = (app, event);
        });
}
