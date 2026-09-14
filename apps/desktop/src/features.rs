use std::sync::Mutex;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

type Result<T> = std::result::Result<T, String>;
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}
pub fn focus(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
#[tauri::command]
pub fn ptt_register(app: AppHandle, key: Option<String>) -> Result<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    if let Some(key) = key {
        app.global_shortcut()
            .on_shortcut(key.as_str(), |app, _, event| {
                let _ = app.emit("ptt", event.state == ShortcutState::Pressed);
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn deep_links(app: AppHandle) -> Vec<String> {
    app.deep_link()
        .get_current()
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .map(|u| u.to_string())
        .collect()
}
struct TrayState {
    mute: CheckMenuItem<tauri::Wry>,
    deafen: CheckMenuItem<tauri::Wry>,
}
pub fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Den", true, None::<&str>)?;
    let mute = CheckMenuItem::with_id(app, "mute", "Mute microphone", false, false, None::<&str>)?;
    let deafen = CheckMenuItem::with_id(app, "deafen", "Deafen", false, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &mute, &deafen, &quit])?;
    TrayIconBuilder::with_id("den")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Den")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => focus(app),
            "quit" => app.exit(0),
            action => {
                let _ = app.emit("tray-action", action);
            }
        })
        .build(app)?;
    app.manage(TrayState { mute, deafen });
    Ok(())
}
#[tauri::command]
pub fn tray_state(app: AppHandle, in_call: bool, muted: bool, deafened: bool) -> Result<()> {
    let state = app.state::<TrayState>();
    state.mute.set_enabled(in_call).map_err(|e| e.to_string())?;
    state
        .deafen
        .set_enabled(in_call)
        .map_err(|e| e.to_string())?;
    state.mute.set_checked(muted).map_err(|e| e.to_string())?;
    state
        .deafen
        .set_checked(deafened)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn badge(app: AppHandle, count: i64) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(not(windows))]
        window
            .set_badge_count(if count > 0 { Some(count) } else { None })
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        window
            .set_overlay_icon(if count > 0 {
                Some(badge_image(count))
            } else {
                None
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(windows)]
fn badge_image(count: i64) -> tauri::image::Image<'static> {
    // A tiny bitmap font stays legible at taskbar overlay size without a font dependency.
    const DIGITS: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let label = count.min(99).to_string();
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            if (x as i32 - 16).pow(2) + (y as i32 - 16).pow(2) < 250 {
                let i = (y * 32 + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[232, 164, 74, 255]);
            }
        }
    }
    for (n, d) in label.bytes().enumerate() {
        for (y, row) in DIGITS[(d - b'0') as usize].iter().enumerate() {
            for x in 0..3 {
                if row & (1 << (2 - x)) != 0 {
                    for dy in 0..3 {
                        for dx in 0..3 {
                            let px = if label.len() == 1 { 12 } else { 5 } + n * 12 + x * 3 + dx;
                            let i = ((8 + y * 3 + dy) * 32 + px) * 4;
                            pixels[i..i + 4].copy_from_slice(&[27, 25, 22, 255]);
                        }
                    }
                }
            }
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}
#[tauri::command]
pub fn notify(
    app: AppHandle,
    title: String,
    body: String,
    origin: String,
    channel: String,
) -> Result<()> {
    session_origin(&origin)?;
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.summary(&title).body(&body).action("default", "Open Den");
        #[cfg(windows)]
        n.app_id("app.denchat.desktop");
        #[cfg(target_os = "macos")]
        let _ = notify_rust::set_application("app.denchat.desktop");
        #[cfg(target_os = "linux")]
        n.appname("Den").icon("app.denchat.desktop");
        if let Ok(handle) = n.show() {
            let _ = handle.wait_for_response(|response: &notify_rust::NotificationResponse| {
                if matches!(
                    response,
                    notify_rust::NotificationResponse::Default
                        | notify_rust::NotificationResponse::Action(_)
                ) {
                    focus(&app);
                    let _ = app.emit(
                        "notification-open",
                        serde_json::json!({"origin":origin,"channel":channel}),
                    );
                }
            });
        }
    });
    Ok(())
}
fn session_origin(value: &str) -> Result<()> {
    crate::session::origin(value).map(|_| ())
}
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>);
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<bool> {
    if app.state::<PendingUpdate>().0.lock().unwrap().is_some() {
        return Ok(true);
    }
    if let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    {
        let bytes = update
            .download(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        *app.state::<PendingUpdate>().0.lock().unwrap() = Some((update, bytes));
        return Ok(true);
    }
    Ok(false)
}
#[tauri::command]
pub fn update_restart(app: AppHandle) -> Result<()> {
    let pending = app.state::<PendingUpdate>().0.lock().unwrap().take();
    if let Some((update, bytes)) = pending {
        update.install(bytes).map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}
