use std::ffi::{c_char, CStr, CString};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Emitter, Manager};

use super::{
    build_menu_bar_popup_snapshot, refresh_daily_value_menu_bar, show_main_window, AppState,
    MAIN_WINDOW_LABEL, MENU_BAR_POPUP_OPEN_SETTINGS_EVENT,
};

static APP_HANDLE: OnceLock<Mutex<Option<AppHandle>>> = OnceLock::new();

type SnapshotProvider = extern "C" fn(force_refresh: bool) -> *mut c_char;
type SnapshotFree = extern "C" fn(value: *mut c_char);
type ActionHandler = extern "C" fn(action: *const c_char);

extern "C" {
    fn codex_pacer_macos_menu_bar_configure(
        snapshot_provider: SnapshotProvider,
        snapshot_free: SnapshotFree,
        action_handler: ActionHandler,
    );
    fn codex_pacer_macos_menu_bar_update(
        visible: bool,
        popup_enabled: bool,
        show_logo: bool,
        api_value_title: *const c_char,
        live_metric_label: *const c_char,
        live_metric_value: *const c_char,
        tooltip: *const c_char,
    );
    fn codex_pacer_macos_menu_bar_close_popover();
    fn codex_pacer_macos_menu_bar_refresh_popover();
}

pub(crate) fn configure(app: &AppHandle) -> Result<(), String> {
    let slot = APP_HANDLE.get_or_init(|| Mutex::new(None));
    let mut guard = slot
        .lock()
        .map_err(|_| "macOS menu bar bridge is unavailable".to_string())?;
    *guard = Some(app.clone());
    drop(guard);

    unsafe {
        codex_pacer_macos_menu_bar_configure(snapshot_provider, snapshot_free, action_handler);
    }
    Ok(())
}

pub(crate) fn update(
    visible: bool,
    popup_enabled: bool,
    show_logo: bool,
    api_value_title: Option<&str>,
    live_metric_label: Option<&str>,
    live_metric_value: Option<&str>,
    tooltip: &str,
) {
    let api_value_title = cstring_lossy(api_value_title.unwrap_or(""));
    let live_metric_label = cstring_lossy(live_metric_label.unwrap_or(""));
    let live_metric_value = cstring_lossy(live_metric_value.unwrap_or(""));
    let tooltip = cstring_lossy(tooltip);
    unsafe {
        codex_pacer_macos_menu_bar_update(
            visible,
            popup_enabled,
            show_logo,
            api_value_title.as_ptr(),
            live_metric_label.as_ptr(),
            live_metric_value.as_ptr(),
            tooltip.as_ptr(),
        );
    }
}

pub(crate) fn close_popover() {
    unsafe {
        codex_pacer_macos_menu_bar_close_popover();
    }
}

pub(crate) fn refresh_popover() {
    unsafe {
        codex_pacer_macos_menu_bar_refresh_popover();
    }
}

fn app_handle() -> Option<AppHandle> {
    APP_HANDLE
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|guard| guard.clone()))
}

extern "C" fn snapshot_provider(force_refresh: bool) -> *mut c_char {
    let Some(app) = app_handle() else {
        return json_error("App handle is unavailable.");
    };
    let state = app.state::<AppState>();
    match build_menu_bar_popup_snapshot(state.inner(), force_refresh) {
        Ok(snapshot) => match serde_json::to_string(&snapshot) {
            Ok(json) => into_c_string(json),
            Err(error) => json_error(&format!("Failed to encode popup snapshot: {error}")),
        },
        Err(error) => json_error(&error),
    }
}

extern "C" fn snapshot_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(value);
    }
}

extern "C" fn action_handler(action: *const c_char) {
    let Some(app) = app_handle() else {
        return;
    };
    let action = unsafe { cstr_to_string(action) };
    match action.as_deref() {
        Some("open_dashboard") => {
            close_popover();
            show_main_window(&app);
        }
        Some("open_settings") => {
            close_popover();
            show_main_window(&app);
            if let Err(error) =
                app.emit_to(MAIN_WINDOW_LABEL, MENU_BAR_POPUP_OPEN_SETTINGS_EVENT, ())
            {
                log::warn!("Failed to focus settings from native popup: {error}");
            }
        }
        Some("refresh") => {
            let state = app.state::<AppState>();
            if let Err(error) = build_menu_bar_popup_snapshot(state.inner(), true) {
                log::warn!("Failed to refresh native menu bar snapshot: {error}");
            }
            refresh_daily_value_menu_bar(state.inner());
            refresh_popover();
        }
        Some("hide") => close_popover(),
        Some("quit") => {
            close_popover();
            app.exit(0);
        }
        Some(other) => log::warn!("Unsupported native menu bar action: {other}"),
        None => {}
    }
}

unsafe fn cstr_to_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    CStr::from_ptr(value).to_str().ok().map(str::to_owned)
}

fn json_error(message: &str) -> *mut c_char {
    let payload = serde_json::json!({ "error": message });
    into_c_string(payload.to_string())
}

fn into_c_string(value: String) -> *mut c_char {
    let bytes = value
        .into_bytes()
        .into_iter()
        .filter(|byte| *byte != 0)
        .collect::<Vec<_>>();
    CString::new(bytes)
        .unwrap_or_else(|_| CString::new("").expect("empty CString is valid"))
        .into_raw()
}

fn cstring_lossy(value: &str) -> CString {
    CString::new(value).unwrap_or_else(|error| {
        CString::new(
            error
                .into_vec()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| CString::new("").expect("empty CString is valid"))
    })
}
