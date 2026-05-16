mod database;
mod importer;
#[cfg(target_os = "macos")]
mod macos_menu_bar;
mod models;
mod pricing;
mod queries;
mod rate_limits;
mod sources;

use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use chrono::{DateTime, Duration as ChronoDuration, Local};
use database::{
    canonical_subscription_currency, create_subscription_record,
    delete_codex_source as delete_codex_source_record, delete_subscription_record,
    get_display_language, get_subscription_profile, get_sync_settings, init_db,
    insert_live_rate_limit_snapshot, list_codex_sources, list_subscription_records,
    load_latest_rate_limit_metadata, open_connection, save_subscription_profile,
    save_sync_settings, set_codex_source_display_selected, set_codex_source_selected,
    set_codex_source_update_selected, set_display_language, update_subscription_record,
    upsert_ssh_codex_source,
};
use importer::{perform_scan, perform_scan_for_source, recalculate_all_session_values};
use models::{
    is_shared_rate_limit_identity, CodexAccountStatus, CodexSource, CodexSourceBatchDownloadResult,
    CodexSourceCandidate, CodexSourceDownloadResult, CodexSourceInput, ConversationDetail,
    ConversationFilters, ConversationListItem, DashboardSnapshot, LiveRateLimitSnapshot,
    MenuBarPopupQuotaSnapshot, MenuBarPopupSnapshot, MenuBarPopupSuggestedSpeed, OverviewResponse,
    PricingCatalogEntry, RateLimitWindowSnapshot, ScanResult, SubscriptionProfile,
    SubscriptionRecord, SubscriptionRecordInput, SyncSettings,
};
use pricing::{load_catalog, refresh_pricing_catalog_from_openai, seed_pricing_catalog};
use queries::{
    get_conversation_detail, get_overview, get_quota_trend, list_conversations, load_dashboard_data,
};
use rate_limits::{query_codex_account_status, query_live_rate_limits};
use rusqlite::params;
use sources::{
    discover_ssh_codex_sources, download_codex_source, download_codex_sources_parallel,
    source_cache_codex_home,
};
use tauri::{AppHandle, Manager, State};

const MAIN_WINDOW_LABEL: &str = "main";
const MENU_BAR_POPUP_OPEN_SETTINGS_EVENT: &str = "codex-counter://open-settings";

#[derive(Clone)]
struct CachedRateLimitSnapshot {
    fetched_at: Instant,
    snapshot: LiveRateLimitSnapshot,
}

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    app_data_dir: PathBuf,
    scan_in_progress: Arc<AtomicBool>,
    menu_bar_available: bool,
    live_rate_limits: Arc<Mutex<Option<CachedRateLimitSnapshot>>>,
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn scanCodexUsage(
    state: State<'_, AppState>,
    codex_home: Option<String>,
) -> Result<ScanResult, String> {
    run_scan_if_idle(state.inner().clone(), codex_home)
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn scanCodexSources(
    state: State<'_, AppState>,
    source_ids: Option<Vec<String>>,
) -> Result<Vec<ScanResult>, String> {
    run_source_scan_if_idle(state.inner().clone(), source_ids)
}

#[allow(non_snake_case)]
#[tauri::command]
fn getScanInProgress(state: State<'_, AppState>) -> bool {
    state.inner().scan_in_progress.load(Ordering::SeqCst)
}

#[allow(non_snake_case)]
#[tauri::command]
fn refreshPricing(state: State<'_, AppState>) -> Result<Vec<PricingCatalogEntry>, String> {
    run_pricing_refresh_if_idle(state.inner().clone())
}

fn refresh_pricing_catalog(state: &AppState) -> Result<Vec<PricingCatalogEntry>, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    refresh_pricing_catalog_from_openai(&conn)?;
    recalculate_all_session_values(&conn).map_err(|error| error.to_string())?;
    let catalog = load_catalog(&conn).map_err(|error| error.to_string())?;
    Ok(catalog)
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn getOverview(
    state: State<'_, AppState>,
    bucket: Option<String>,
    anchor: Option<String>,
    custom_start: Option<String>,
    custom_end: Option<String>,
    live_window_offset: Option<i64>,
    source_ids: Option<Vec<String>>,
) -> Result<OverviewResponse, String> {
    let live_rate_limits =
        maybe_live_rate_limits_for_bucket(state.inner(), bucket.as_deref(), live_window_offset)?;
    get_overview(
        &state.db_path,
        bucket,
        anchor,
        custom_start,
        custom_end,
        live_rate_limits,
        live_window_offset,
        source_ids,
    )
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn listConversations(
    state: State<'_, AppState>,
    filters: Option<ConversationFilters>,
) -> Result<Vec<ConversationListItem>, String> {
    let live_rate_limits = maybe_live_rate_limits_for_bucket(
        state.inner(),
        filters.as_ref().and_then(|value| value.bucket.as_deref()),
        filters.as_ref().and_then(|value| value.live_window_offset),
    )?;
    list_conversations(&state.db_path, filters, live_rate_limits)
}

#[allow(non_snake_case)]
#[tauri::command]
fn getLiveRateLimits(state: State<'_, AppState>) -> Result<LiveRateLimitSnapshot, String> {
    get_live_rate_limits_cached(state.inner())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
async fn loadDashboard(
    state: State<'_, AppState>,
    bucket: Option<String>,
    anchor: Option<String>,
    custom_start: Option<String>,
    custom_end: Option<String>,
    search: Option<String>,
    live_window_offset: Option<i64>,
    source_ids: Option<Vec<String>>,
) -> Result<DashboardSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let normalized_bucket = bucket.clone().unwrap_or_else(|| "month".to_string());
        let live_rate_limits = maybe_live_rate_limits_for_bucket(
            &state,
            Some(&normalized_bucket),
            live_window_offset,
        )?;
        let snapshot = load_dashboard_data(
            &state.db_path,
            Some(normalized_bucket.clone()),
            anchor.clone(),
            custom_start.clone(),
            custom_end.clone(),
            search,
            live_rate_limits.clone(),
            live_window_offset,
            source_ids,
        )?;
        let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
        let sync_settings = get_sync_settings(&conn).map_err(|error| error.to_string())?;
        let subscription_profile =
            get_subscription_profile(&conn).map_err(|error| error.to_string())?;
        let codex_sources = list_codex_sources(&conn).map_err(|error| error.to_string())?;

        Ok(DashboardSnapshot {
            overview: snapshot.overview,
            conversations: snapshot.conversations,
            codex_sources,
            sync_settings,
            subscription_profile,
            subscription_records: snapshot.subscription_records,
            account_status: safe_codex_account_status(),
            live_rate_limits,
        })
    })
    .await
    .map_err(|error| format!("Failed to load dashboard: {error}"))?
}

#[allow(non_snake_case)]
#[tauri::command]
fn discoverSshCodexSources() -> Vec<CodexSourceCandidate> {
    discover_ssh_codex_sources()
}

#[allow(non_snake_case)]
#[tauri::command]
fn listCodexSources(state: State<'_, AppState>) -> Result<Vec<CodexSource>, String> {
    let conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    list_codex_sources(&conn).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn upsertCodexSource(
    state: State<'_, AppState>,
    payload: CodexSourceInput,
) -> Result<CodexSource, String> {
    let source_id = format!(
        "ssh_{}",
        payload
            .ssh_alias
            .chars()
            .map(|character| if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            })
            .collect::<String>()
            .split('_')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    );
    let cache_home = source_cache_codex_home(&state.inner().app_data_dir, &source_id)
        .to_string_lossy()
        .to_string();
    let conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    upsert_ssh_codex_source(&conn, &source_id, &payload, &cache_home)
        .map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn setCodexSourceSelected(
    state: State<'_, AppState>,
    source_id: String,
    selected: bool,
) -> Result<CodexSource, String> {
    let conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    set_codex_source_selected(&conn, &source_id, selected).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn setCodexSourceDisplaySelected(
    state: State<'_, AppState>,
    source_id: String,
    selected: bool,
) -> Result<CodexSource, String> {
    let conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    set_codex_source_display_selected(&conn, &source_id, selected)
        .map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn setCodexSourceUpdateSelected(
    state: State<'_, AppState>,
    source_id: String,
    selected: bool,
) -> Result<CodexSource, String> {
    let conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    set_codex_source_update_selected(&conn, &source_id, selected).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn deleteCodexSource(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<Vec<CodexSource>, String> {
    if source_id == "local" {
        return Err("localhost cannot be deleted.".to_string());
    }

    let source_root = source_cache_codex_home(&state.inner().app_data_dir, &source_id)
        .parent()
        .map(|path| path.to_path_buf());
    let mut conn = open_connection(&state.inner().db_path).map_err(|error| error.to_string())?;
    let deleted =
        delete_codex_source_record(&mut conn, &source_id).map_err(|error| error.to_string())?;
    if !deleted {
        return Err("Source not found.".to_string());
    }
    if let Some(path) = source_root {
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("Deleted source but failed to remove cache: {error}"))?;
        }
    }
    list_codex_sources(&conn).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
async fn downloadCodexSource(
    app: AppHandle,
    state: State<'_, AppState>,
    source_id: String,
) -> Result<CodexSourceDownloadResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_single_source_download_if_idle(&app, state, &source_id)
    })
    .await
    .map_err(|error| format!("Failed to download Codex source: {error}"))?
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
async fn downloadCodexSources(
    app: AppHandle,
    state: State<'_, AppState>,
    source_ids: Vec<String>,
) -> Result<CodexSourceBatchDownloadResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_source_download_if_idle(&app, state, source_ids)
    })
    .await
    .map_err(|error| format!("Failed to sync Codex sources: {error}"))?
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn getConversationDetail(
    state: State<'_, AppState>,
    root_session_id: String,
) -> Result<ConversationDetail, String> {
    get_conversation_detail(&state.db_path, &root_session_id)
}

#[allow(non_snake_case)]
#[tauri::command]
fn getSyncSettings(state: State<'_, AppState>) -> Result<SyncSettings, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    get_sync_settings(&conn).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn updateSyncSettings(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: SyncSettings,
) -> Result<SyncSettings, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    let current = get_sync_settings(&conn).map_err(|error| error.to_string())?;
    let updated = SyncSettings {
        codex_home: payload.codex_home,
        auto_scan_enabled: payload.auto_scan_enabled,
        auto_scan_interval_minutes: payload.auto_scan_interval_minutes.max(1),
        remote_auto_update_enabled: payload.remote_auto_update_enabled,
        remote_auto_update_interval_minutes: payload.remote_auto_update_interval_minutes.max(1),
        live_quota_refresh_interval_seconds: payload
            .live_quota_refresh_interval_seconds
            .clamp(60, 3600),
        hide_dock_icon_when_menu_bar_visible: payload.hide_dock_icon_when_menu_bar_visible,
        show_menu_bar_logo: payload.show_menu_bar_logo,
        show_menu_bar_daily_api_value: payload.show_menu_bar_daily_api_value,
        show_menu_bar_live_quota_percent: payload.show_menu_bar_live_quota_percent,
        menu_bar_live_quota_metric: normalize_menu_bar_live_quota_metric(
            &payload.menu_bar_live_quota_metric,
        ),
        menu_bar_live_quota_bucket: normalize_menu_bar_live_quota_bucket(
            &payload.menu_bar_live_quota_bucket,
        ),
        menu_bar_bucket: normalize_menu_bar_bucket(&payload.menu_bar_bucket),
        menu_bar_speed_show_emoji: payload.menu_bar_speed_show_emoji,
        menu_bar_speed_fast_threshold_percent: payload
            .menu_bar_speed_fast_threshold_percent
            .clamp(0, 1000),
        menu_bar_speed_slow_threshold_percent: payload
            .menu_bar_speed_slow_threshold_percent
            .clamp(0, 1000),
        menu_bar_speed_healthy_emoji: normalize_menu_bar_speed_emoji(
            &payload.menu_bar_speed_healthy_emoji,
            "🟢",
        ),
        menu_bar_speed_fast_emoji: normalize_menu_bar_speed_emoji(
            &payload.menu_bar_speed_fast_emoji,
            "🔥",
        ),
        menu_bar_speed_slow_emoji: normalize_menu_bar_speed_emoji(
            &payload.menu_bar_speed_slow_emoji,
            "🐢",
        ),
        menu_bar_popup_enabled: payload.menu_bar_popup_enabled,
        menu_bar_popup_modules: normalize_menu_bar_popup_modules(&payload.menu_bar_popup_modules),
        menu_bar_popup_show_reset_timeline: payload.menu_bar_popup_show_reset_timeline,
        menu_bar_popup_show_actions: payload.menu_bar_popup_show_actions,
        last_scan_started_at: current.last_scan_started_at,
        last_scan_completed_at: current.last_scan_completed_at,
        updated_at: current.updated_at,
    };
    let saved = save_sync_settings(&conn, &updated).map_err(|error| error.to_string())?;
    refresh_daily_value_menu_bar(state.inner());
    sync_dock_icon_visibility(&app, state.inner());
    Ok(saved)
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn updateDisplayLanguage(state: State<'_, AppState>, language: String) -> Result<String, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    set_display_language(&conn, &language).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command]
fn getSubscriptionProfile(state: State<'_, AppState>) -> Result<SubscriptionProfile, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    get_subscription_profile(&conn).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn updateSubscriptionProfile(
    state: State<'_, AppState>,
    payload: SubscriptionProfile,
) -> Result<SubscriptionProfile, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    let updated = SubscriptionProfile {
        plan_type: payload.plan_type,
        currency: canonical_subscription_currency().to_string(),
        monthly_price: payload.monthly_price.max(0.0),
        billing_anchor_day: payload.billing_anchor_day.clamp(1, 28),
        updated_at: payload.updated_at,
    };
    save_subscription_profile(&conn, &updated).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command]
fn listSubscriptionRecords(state: State<'_, AppState>) -> Result<Vec<SubscriptionRecord>, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    list_subscription_records(&conn).map_err(|error| error.to_string())
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn createSubscriptionRecord(
    state: State<'_, AppState>,
    payload: SubscriptionRecordInput,
) -> Result<SubscriptionRecord, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    create_subscription_record(&conn, &payload)
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn updateSubscriptionRecord(
    state: State<'_, AppState>,
    id: i64,
    payload: SubscriptionRecordInput,
) -> Result<SubscriptionRecord, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    update_subscription_record(&conn, id, &payload)
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
fn deleteSubscriptionRecord(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    delete_subscription_record(&conn, id)
}

#[allow(non_snake_case)]
#[tauri::command]
fn getCodexAccountStatus() -> CodexAccountStatus {
    safe_codex_account_status()
}

fn safe_codex_account_status() -> CodexAccountStatus {
    query_codex_account_status()
        .unwrap_or_else(|error| CodexAccountStatus::unavailable(error, Local::now().to_rfc3339()))
}

fn run_scan_if_idle(state: AppState, codex_home: Option<String>) -> Result<ScanResult, String> {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan is already running.".to_string());
    }

    let result = perform_scan(&state.db_path, codex_home);
    state.scan_in_progress.store(false, Ordering::SeqCst);
    refresh_daily_value_menu_bar(&state);
    result
}

fn run_source_scan_if_idle(
    state: AppState,
    source_ids: Option<Vec<String>>,
) -> Result<Vec<ScanResult>, String> {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan is already running.".to_string());
    }

    let result = (|| {
        let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
        let sources = list_codex_sources(&conn).map_err(|error| error.to_string())?;
        let requested = source_ids.unwrap_or_else(|| {
            sources
                .iter()
                .filter(|source| source.display_selected)
                .map(|source| source.id.clone())
                .collect()
        });
        let requested = requested
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut results = Vec::new();
        for source in sources {
            if !requested.contains(&source.id) {
                continue;
            }
            let codex_home = if source.id == "local" {
                None
            } else {
                source.local_codex_home.clone()
            };
            if source.id != "local" && codex_home.is_none() {
                continue;
            }
            results.push(perform_scan_for_source(
                &state.db_path,
                &source.id,
                codex_home,
            )?);
        }
        Ok(results)
    })();

    state.scan_in_progress.store(false, Ordering::SeqCst);
    if result.is_ok() {
        refresh_daily_value_menu_bar(&state);
    }
    result
}

fn run_source_download_if_idle(
    app: &AppHandle,
    state: AppState,
    source_ids: Vec<String>,
) -> Result<CodexSourceBatchDownloadResult, String> {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan is already running.".to_string());
    }

    let result =
        download_codex_sources_parallel(app, &state.db_path, &state.app_data_dir, source_ids);
    state.scan_in_progress.store(false, Ordering::SeqCst);
    refresh_daily_value_menu_bar(&state);
    Ok(result)
}

fn run_single_source_download_if_idle(
    app: &AppHandle,
    state: AppState,
    source_id: &str,
) -> Result<CodexSourceDownloadResult, String> {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan is already running.".to_string());
    }

    let result = download_codex_source(app, &state.db_path, &state.app_data_dir, source_id);
    state.scan_in_progress.store(false, Ordering::SeqCst);
    if result.is_ok() {
        refresh_daily_value_menu_bar(&state);
    }
    result
}

fn run_pricing_refresh_if_idle(state: AppState) -> Result<Vec<PricingCatalogEntry>, String> {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan is already running.".to_string());
    }

    let result = refresh_pricing_catalog(&state);
    state.scan_in_progress.store(false, Ordering::SeqCst);
    if result.is_ok() {
        refresh_daily_value_menu_bar(&state);
    }
    result
}

fn refresh_daily_value_menu_bar(state: &AppState) {
    if let Err(error) = update_daily_value_menu_bar(state) {
        log::warn!("Failed to update menu bar display: {error}");
    }
}

fn update_daily_value_menu_bar(state: &AppState) -> Result<(), String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    let settings = get_sync_settings(&conn).map_err(|error| error.to_string())?;

    #[cfg(target_os = "macos")]
    {
        if !state.menu_bar_available {
            return Ok(());
        }
        let visible = menu_bar_has_visible_content(&settings);
        let (api_value_title, live_metric_label, live_metric_title) =
            current_menu_bar_title_parts(state, &settings)?;
        let tooltip = menu_bar_tooltip(&settings, api_value_title.as_deref(), state)?;
        macos_menu_bar::update(
            visible,
            settings.menu_bar_popup_enabled,
            settings.show_menu_bar_logo,
            api_value_title.as_deref(),
            live_metric_label.as_deref(),
            live_metric_title.as_deref(),
            &tooltip,
        );
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = settings;
        Ok(())
    }
}

fn menu_bar_has_visible_content(settings: &SyncSettings) -> bool {
    settings.show_menu_bar_logo
        || settings.show_menu_bar_daily_api_value
        || settings.show_menu_bar_live_quota_percent
}

fn should_hide_dock_icon(settings: &SyncSettings) -> bool {
    settings.hide_dock_icon_when_menu_bar_visible && menu_bar_has_visible_content(settings)
}

#[cfg(target_os = "macos")]
fn apply_dock_icon_visibility(app: &AppHandle, settings: &SyncSettings, menu_bar_available: bool) {
    let main_window_visible = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true);
    let activation_policy =
        if menu_bar_available && should_hide_dock_icon(settings) && !main_window_visible {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };

    if let Err(error) = app.set_activation_policy(activation_policy) {
        log::warn!("Failed to update macOS Dock visibility: {error}");
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_dock_icon_visibility(_: &AppHandle, _: &SyncSettings, _: bool) {}

fn sync_dock_icon_visibility(app: &AppHandle, state: &AppState) {
    let Ok(conn) = open_connection(&state.db_path) else {
        return;
    };
    let Ok(settings) = get_sync_settings(&conn) else {
        return;
    };
    apply_dock_icon_visibility(app, &settings, state.menu_bar_available);
}

#[cfg(target_os = "macos")]
fn show_dock_icon_for_main_window(app: &AppHandle) {
    if let Err(error) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
        log::warn!("Failed to show macOS Dock icon: {error}");
    }
}

#[cfg(not(target_os = "macos"))]
fn show_dock_icon_for_main_window(_: &AppHandle) {}

fn current_menu_bar_title_parts(
    state: &AppState,
    settings: &SyncSettings,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let bucket = normalize_menu_bar_bucket(&settings.menu_bar_bucket);
    let anchor = Local::now().format("%Y-%m-%d").to_string();
    let live_rate_limits = if settings.show_menu_bar_live_quota_percent
        || (settings.show_menu_bar_daily_api_value && bucket_uses_live_rate_limits(&bucket))
    {
        maybe_live_rate_limits_for_bucket(state, Some(&bucket), None)?
    } else {
        None
    };
    let api_value_title = if settings.show_menu_bar_daily_api_value {
        let overview = get_overview(
            &state.db_path,
            Some(bucket.clone()),
            if bucket_uses_anchor(&bucket) {
                Some(anchor)
            } else {
                None
            },
            None,
            None,
            live_rate_limits.clone(),
            None,
            None,
        )?;
        Some(format!("${:.1}", overview.stats.api_value_usd))
    } else {
        None
    };
    let (live_metric_label, live_metric_title) = if settings.show_menu_bar_live_quota_percent {
        menu_bar_live_quota_snapshot(
            state,
            settings,
            &settings.menu_bar_live_quota_bucket,
            &settings.menu_bar_live_quota_metric,
            live_rate_limits,
            Local::now(),
        )?
        .map(|metric| (Some(metric.0), Some(metric.1)))
        .unwrap_or((None, None))
    } else {
        (None, None)
    };
    Ok((api_value_title, live_metric_label, live_metric_title))
}

#[cfg(test)]
fn menu_bar_title(
    api_value_title: Option<&str>,
    live_metric_title: Option<&str>,
) -> Option<String> {
    let mut segments = Vec::new();
    if let Some(value) = api_value_title.filter(|value| !value.trim().is_empty()) {
        segments.push(value.to_string());
    }
    if let Some(value) = live_metric_title.filter(|value| !value.trim().is_empty()) {
        segments.push(value.to_string());
    }
    if segments.is_empty() {
        None
    } else {
        Some(segments.join(" "))
    }
}

fn normalize_menu_bar_bucket(bucket: &str) -> String {
    match bucket {
        "day" | "week" | "five_hour" | "seven_day" | "month" | "year" | "total" => {
            bucket.to_string()
        }
        _ => "day".to_string(),
    }
}

fn normalize_menu_bar_live_quota_bucket(bucket: &str) -> String {
    match bucket {
        "five_hour" | "seven_day" => bucket.to_string(),
        _ => "five_hour".to_string(),
    }
}

fn normalize_menu_bar_live_quota_metric(metric: &str) -> String {
    match metric {
        "remaining_percent" | "used_percent" | "suggested_usage_speed" => metric.to_string(),
        _ => "remaining_percent".to_string(),
    }
}

fn normalize_menu_bar_speed_emoji(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(4).collect()
    }
}

fn normalize_menu_bar_popup_modules(modules: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for module in modules {
        let candidate = match module.as_str() {
            "api_value"
            | "token_count"
            | "scan_freshness"
            | "live_quota_freshness"
            | "payoff_ratio"
            | "conversation_count" => module.clone(),
            _ => continue,
        };

        if !normalized.iter().any(|existing| existing == &candidate) {
            normalized.push(candidate);
        }
    }
    normalized
}

fn menu_bar_tooltip(
    settings: &SyncSettings,
    api_value_title: Option<&str>,
    state: &AppState,
) -> Result<String, String> {
    let bucket = normalize_menu_bar_bucket(&settings.menu_bar_bucket);
    let mut fragments = Vec::new();
    if let Some(title) = api_value_title.filter(|value| !value.trim().is_empty()) {
        fragments.push(format!(
            "{}累计 API 价值：{title}",
            menu_bar_bucket_label(&bucket)
        ));
    }
    if settings.show_menu_bar_live_quota_percent {
        let snapshot = get_live_rate_limits_cached(state)?;
        if let Some(fragment) = menu_bar_live_quota_tooltip(
            &snapshot,
            settings,
            &settings.menu_bar_live_quota_bucket,
            &settings.menu_bar_live_quota_metric,
            Local::now(),
        ) {
            fragments.push(fragment);
        }
    }
    if fragments.is_empty() {
        Ok("Codex Pacer".to_string())
    } else {
        Ok(fragments.join(" · "))
    }
}

fn menu_bar_bucket_label(bucket: &str) -> &'static str {
    match bucket {
        "week" => "本周",
        "five_hour" => "近 5 小时",
        "seven_day" => "近 7 天",
        "month" => "本月",
        "year" => "本年",
        "total" => "总计",
        _ => "今日",
    }
}

fn bucket_uses_live_rate_limits(bucket: &str) -> bool {
    matches!(bucket, "five_hour" | "seven_day")
}

fn bucket_uses_anchor(bucket: &str) -> bool {
    !matches!(bucket, "total" | "five_hour" | "seven_day")
}

fn menu_bar_live_quota_snapshot(
    state: &AppState,
    settings: &SyncSettings,
    bucket: &str,
    metric: &str,
    existing_snapshot: Option<LiveRateLimitSnapshot>,
    now: DateTime<Local>,
) -> Result<Option<(String, String)>, String> {
    let snapshot = match existing_snapshot {
        Some(snapshot) => snapshot,
        None => get_live_rate_limits_cached(state)?,
    };
    Ok(menu_bar_live_quota_title(
        &snapshot, settings, bucket, metric, now,
    ))
}

fn selected_menu_bar_live_quota_window<'a>(
    snapshot: &'a LiveRateLimitSnapshot,
    bucket: &str,
) -> Option<(&'static str, &'a RateLimitWindowSnapshot)> {
    match normalize_menu_bar_live_quota_bucket(bucket).as_str() {
        "seven_day" => snapshot.secondary.as_ref().map(|window| ("7天", window)),
        _ => snapshot.primary.as_ref().map(|window| ("5小时", window)),
    }
}

fn menu_bar_live_quota_title(
    snapshot: &LiveRateLimitSnapshot,
    settings: &SyncSettings,
    bucket: &str,
    metric: &str,
    now: DateTime<Local>,
) -> Option<(String, String)> {
    let (_, window) = selected_menu_bar_live_quota_window(snapshot, bucket)?;
    let normalized_bucket = normalize_menu_bar_live_quota_bucket(bucket);
    match normalize_menu_bar_live_quota_metric(metric).as_str() {
        "used_percent" => Some((
            menu_bar_live_quota_status_label(&normalized_bucket, "used_percent"),
            format!("{}%", window.used_percent.clamp(0, 100)),
        )),
        "suggested_usage_speed" => {
            let velocity = suggested_usage_velocity(window, now, settings)?;
            Some((
                menu_bar_live_quota_status_label(&normalized_bucket, "suggested_usage_speed"),
                velocity.rendered_value(),
            ))
        }
        _ => Some((
            menu_bar_live_quota_status_label(&normalized_bucket, "remaining_percent"),
            format!("{}%", window.remaining_percent.clamp(0, 100)),
        )),
    }
}

fn menu_bar_live_quota_status_label(bucket: &str, metric: &str) -> String {
    let prefix = match normalize_menu_bar_live_quota_bucket(bucket).as_str() {
        "seven_day" => "7d",
        _ => "5h",
    };
    let suffix = match normalize_menu_bar_live_quota_metric(metric).as_str() {
        "used_percent" => "Cost",
        "suggested_usage_speed" => "Pace",
        _ => "Rest",
    };
    format!("{prefix} {suffix}")
}

fn menu_bar_live_quota_tooltip(
    snapshot: &LiveRateLimitSnapshot,
    settings: &SyncSettings,
    bucket: &str,
    metric: &str,
    now: DateTime<Local>,
) -> Option<String> {
    let (label, window) = selected_menu_bar_live_quota_window(snapshot, bucket)?;
    match normalize_menu_bar_live_quota_metric(metric).as_str() {
        "used_percent" => Some(format!(
            "{label}已用 {}%",
            window.used_percent.clamp(0, 100)
        )),
        "suggested_usage_speed" => {
            let velocity = suggested_usage_velocity(window, now, settings)?;
            Some(format!(
                "{label}建议使用速度 {} {} · 剩余额度 {}% / 剩余时间 {:.1}%",
                velocity.emoji,
                velocity.display_value,
                window.remaining_percent.clamp(0, 100),
                velocity.remaining_time_percent,
            ))
        }
        _ => Some(format!(
            "{label}剩余 {}%",
            window.remaining_percent.clamp(0, 100)
        )),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SuggestedUsageVelocityDisplay {
    emoji: String,
    display_value: String,
    remaining_time_percent: f64,
}

impl SuggestedUsageVelocityDisplay {
    fn rendered_value(&self) -> String {
        if self.emoji.is_empty() {
            self.display_value.clone()
        } else {
            format!("{} {}", self.emoji, self.display_value)
        }
    }
}

fn suggested_usage_velocity(
    window: &RateLimitWindowSnapshot,
    now: DateTime<Local>,
    settings: &SyncSettings,
) -> Option<SuggestedUsageVelocityDisplay> {
    let (window_start, reset_at) = quota_window_bounds(window)?;
    let total_seconds = reset_at.signed_duration_since(window_start).num_seconds();
    if total_seconds <= 0 {
        return None;
    }

    let remaining_seconds = reset_at.signed_duration_since(now).num_seconds() as f64;
    let remaining_time_percent =
        ((remaining_seconds / total_seconds as f64) * 100.0).clamp(0.0, 100.0);
    let remaining_percent = window.remaining_percent.clamp(0, 100) as f64;
    let ratio = if remaining_time_percent <= 0.0 {
        if remaining_percent <= 0.0 {
            1.0
        } else {
            10.0
        }
    } else {
        remaining_percent / remaining_time_percent
    };
    let capped_ratio = ratio.clamp(0.0, 10.0);
    let percent = capped_ratio * 100.0;
    let display_value = if ratio > 10.0 {
        "1000%+".to_string()
    } else {
        format!("{percent:.0}%")
    };

    let fast_threshold = settings
        .menu_bar_speed_fast_threshold_percent
        .clamp(0, 1000) as f64;
    let slow_threshold = settings
        .menu_bar_speed_slow_threshold_percent
        .clamp(0, 1000)
        .max(
            settings
                .menu_bar_speed_fast_threshold_percent
                .clamp(0, 1000),
        ) as f64;

    Some(SuggestedUsageVelocityDisplay {
        emoji: usage_velocity_emoji(percent, fast_threshold, slow_threshold, settings),
        display_value,
        remaining_time_percent,
    })
}

fn usage_velocity_emoji(
    percent: f64,
    fast_threshold: f64,
    slow_threshold: f64,
    settings: &SyncSettings,
) -> String {
    if !settings.menu_bar_speed_show_emoji {
        String::new()
    } else if percent < fast_threshold {
        settings.menu_bar_speed_fast_emoji.clone()
    } else if percent <= slow_threshold {
        settings.menu_bar_speed_healthy_emoji.clone()
    } else {
        settings.menu_bar_speed_slow_emoji.clone()
    }
}

fn quota_window_bounds(
    window: &RateLimitWindowSnapshot,
) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let reset_at = window.resets_at.as_deref().and_then(parse_rfc3339_local)?;
    let window_start = match window.window_start.as_deref().and_then(parse_rfc3339_local) {
        Some(timestamp) => timestamp,
        None => reset_at - ChronoDuration::minutes(window.window_duration_mins?),
    };
    Some((window_start, reset_at))
}

fn parse_rfc3339_local(value: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Local))
}

fn maybe_live_rate_limits_for_bucket(
    state: &AppState,
    bucket: Option<&str>,
    live_window_offset: Option<i64>,
) -> Result<Option<LiveRateLimitSnapshot>, String> {
    let Some(bucket) = bucket else {
        return Ok(None);
    };
    if !bucket_uses_live_rate_limits(bucket) {
        return Ok(None);
    }
    if live_window_offset.unwrap_or(0) > 0 {
        return Ok(None);
    }
    Ok(Some(get_live_rate_limits_cached(state)?))
}

fn get_live_rate_limits_cached(state: &AppState) -> Result<LiveRateLimitSnapshot, String> {
    get_live_rate_limits(state, false)
}

fn get_live_rate_limits(
    state: &AppState,
    force_refresh: bool,
) -> Result<LiveRateLimitSnapshot, String> {
    let ttl = live_rate_limit_cache_ttl(state);

    if !force_refresh {
        let cache = state
            .live_rate_limits
            .lock()
            .map_err(|_| "Live rate-limit cache is unavailable.".to_string())?;
        if let Some(snapshot) = cache.as_ref() {
            if snapshot.fetched_at.elapsed() <= ttl {
                return Ok(snapshot.snapshot.clone());
            }
        }
    }

    match query_live_rate_limits() {
        Ok(snapshot)
            if is_shared_rate_limit_identity(
                snapshot.limit_id.as_deref(),
                snapshot.limit_name.as_deref(),
            ) =>
        {
            let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
            insert_live_rate_limit_snapshot(&conn, &snapshot).map_err(|error| error.to_string())?;
            store_live_rate_limits_cache(state, &snapshot)?;
            Ok(snapshot)
        }
        Ok(snapshot) => {
            let error = format!(
                "Codex app-server returned a model-specific rate limit ({:?}); ignoring it.",
                snapshot.limit_name.clone().or(snapshot.limit_id.clone())
            );
            log::warn!("{error}");
            if let Ok(conn) = open_connection(&state.db_path) {
                if let Err(insert_error) = insert_live_rate_limit_snapshot(&conn, &snapshot) {
                    log::warn!("Failed to persist model-specific rate limits: {insert_error}");
                }
            }
            if let Some(snapshot) = get_live_rate_limits_history_fallback(state) {
                if let Err(cache_error) = store_live_rate_limits_cache(state, &snapshot) {
                    log::warn!("Failed to cache fallback live rate limits: {cache_error}");
                }
                return Ok(snapshot);
            }
            Err(error)
        }
        Err(error) => {
            log::warn!("Failed to refresh live rate limits from Codex app-server: {error}");
            if let Some(snapshot) = get_live_rate_limits_history_fallback(state) {
                if let Err(cache_error) = store_live_rate_limits_cache(state, &snapshot) {
                    log::warn!("Failed to cache fallback live rate limits: {cache_error}");
                }
                return Ok(snapshot);
            }
            Err(error)
        }
    }
}

#[derive(Clone)]
struct PersistedRateLimitWindow {
    snapshot: RateLimitWindowSnapshot,
    fetched_at: String,
    limit_id: Option<String>,
    limit_name: Option<String>,
    plan_type: Option<String>,
}

fn load_latest_persisted_rate_limit_window(
    conn: &rusqlite::Connection,
    bucket: &str,
    source_kind: Option<&str>,
) -> Result<Option<PersistedRateLimitWindow>, String> {
    let mut stmt = conn
    .prepare(
      "
      SELECT sample_timestamp, limit_id, limit_name, plan_type, window_start, resets_at, used_percent, remaining_percent
      FROM rate_limit_samples
      WHERE bucket = ?1 AND (?2 IS NULL OR source_kind = ?2)
        AND (limit_id = '' OR limit_id = 'codex')
        AND (limit_name = '' OR limit_name NOT LIKE 'GPT-%')
      ORDER BY sample_timestamp DESC
      LIMIT 1
      ",
    )
    .map_err(|error| error.to_string())?;

    let mut rows = stmt
        .query(params![bucket, source_kind])
        .map_err(|error| error.to_string())?;
    let Some(row) = rows.next().map_err(|error| error.to_string())? else {
        return Ok(None);
    };

    let sample_timestamp = row.get::<_, String>(0).map_err(|error| error.to_string())?;
    let limit_id = row
        .get::<_, String>(1)
        .ok()
        .and_then(|value| (!value.is_empty()).then_some(value));
    let limit_name = row
        .get::<_, String>(2)
        .ok()
        .and_then(|value| (!value.is_empty()).then_some(value));
    let plan_type = row
        .get::<_, String>(3)
        .ok()
        .and_then(|value| (!value.is_empty()).then_some(value));
    let window_start = row.get::<_, String>(4).map_err(|error| error.to_string())?;
    let resets_at = row.get::<_, String>(5).map_err(|error| error.to_string())?;
    let used_percent = row.get::<_, i64>(6).map_err(|error| error.to_string())?;
    let remaining_percent = row.get::<_, i64>(7).map_err(|error| error.to_string())?;

    let window_duration_mins = match (
        parse_rfc3339_local(&window_start),
        parse_rfc3339_local(&resets_at),
    ) {
        (Some(start), Some(end)) => Some(end.signed_duration_since(start).num_minutes().max(0)),
        _ => None,
    };

    Ok(Some(PersistedRateLimitWindow {
        snapshot: RateLimitWindowSnapshot {
            used_percent,
            remaining_percent,
            window_duration_mins,
            resets_at: Some(resets_at),
            window_start: Some(window_start),
        },
        fetched_at: sample_timestamp,
        limit_id,
        limit_name,
        plan_type,
    }))
}

fn load_persisted_live_rate_limits(state: &AppState) -> Option<LiveRateLimitSnapshot> {
    load_persisted_live_rate_limits_for_source(state, None)
}

fn load_history_live_rate_limits(state: &AppState) -> Option<LiveRateLimitSnapshot> {
    load_persisted_live_rate_limits_for_source(state, Some("session"))
}

fn load_persisted_live_rate_limits_for_source(
    state: &AppState,
    source_kind: Option<&str>,
) -> Option<LiveRateLimitSnapshot> {
    let conn = open_connection(&state.db_path).ok()?;
    let primary = load_latest_persisted_rate_limit_window(&conn, "five_hour", source_kind)
        .ok()
        .flatten();
    let secondary = load_latest_persisted_rate_limit_window(&conn, "seven_day", source_kind)
        .ok()
        .flatten();
    let metadata = load_latest_rate_limit_metadata(&conn, source_kind)
        .ok()
        .flatten();
    if primary.is_none() && secondary.is_none() && metadata.is_none() {
        return None;
    }

    let fetched_at = latest_rate_limit_timestamp([
        primary.as_ref().map(|window| window.fetched_at.as_str()),
        secondary.as_ref().map(|window| window.fetched_at.as_str()),
        metadata
            .as_ref()
            .map(|metadata| metadata.sample_timestamp.as_str()),
    ])
    .unwrap_or_else(|| Local::now().to_rfc3339());

    Some(LiveRateLimitSnapshot {
        limit_id: metadata
            .as_ref()
            .and_then(|metadata| metadata.limit_id.clone())
            .or_else(|| primary.as_ref().and_then(|window| window.limit_id.clone()))
            .or_else(|| {
                secondary
                    .as_ref()
                    .and_then(|window| window.limit_id.clone())
            }),
        limit_name: metadata
            .as_ref()
            .and_then(|metadata| metadata.limit_name.clone())
            .or_else(|| {
                primary
                    .as_ref()
                    .and_then(|window| window.limit_name.clone())
            })
            .or_else(|| {
                secondary
                    .as_ref()
                    .and_then(|window| window.limit_name.clone())
            }),
        plan_type: metadata
            .as_ref()
            .and_then(|metadata| metadata.plan_type.clone())
            .or_else(|| primary.as_ref().and_then(|window| window.plan_type.clone()))
            .or_else(|| {
                secondary
                    .as_ref()
                    .and_then(|window| window.plan_type.clone())
            }),
        credits: metadata
            .as_ref()
            .and_then(|metadata| metadata.credits.clone()),
        rate_limit_reached_type: metadata
            .as_ref()
            .and_then(|metadata| metadata.rate_limit_reached_type.clone()),
        primary: primary.map(|window| window.snapshot),
        secondary: secondary.map(|window| window.snapshot),
        fetched_at,
    })
}

fn latest_rate_limit_timestamp<'a>(
    values: impl IntoIterator<Item = Option<&'a str>>,
) -> Option<String> {
    let mut latest: Option<&str> = None;
    for value in values.into_iter().flatten() {
        match latest {
            None => latest = Some(value),
            Some(current) if timestamp_after(value, current) => latest = Some(value),
            _ => {}
        }
    }
    latest.map(ToString::to_string)
}

fn timestamp_after(candidate: &str, current: &str) -> bool {
    match (parse_rfc3339_local(candidate), parse_rfc3339_local(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => candidate > current,
    }
}

fn get_live_rate_limits_local(state: &AppState) -> Option<LiveRateLimitSnapshot> {
    get_cached_live_rate_limits(state).or_else(|| load_persisted_live_rate_limits(state))
}

fn get_live_rate_limits_history_fallback(state: &AppState) -> Option<LiveRateLimitSnapshot> {
    refresh_rate_limit_history_if_idle(state);
    load_history_live_rate_limits(state)
        .or_else(|| load_persisted_live_rate_limits(state))
        .or_else(|| get_cached_live_rate_limits(state))
}

fn refresh_rate_limit_history_if_idle(state: &AppState) {
    if state
        .scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let result = perform_scan(&state.db_path, None);
    state.scan_in_progress.store(false, Ordering::SeqCst);
    if let Err(error) = result {
        log::warn!("Failed to refresh Codex history rate-limit samples: {error}");
    }
}

fn get_cached_live_rate_limits(state: &AppState) -> Option<LiveRateLimitSnapshot> {
    state
        .live_rate_limits
        .lock()
        .ok()
        .and_then(|cache| cache.as_ref().map(|snapshot| snapshot.snapshot.clone()))
}

fn store_live_rate_limits_cache(
    state: &AppState,
    snapshot: &LiveRateLimitSnapshot,
) -> Result<(), String> {
    let mut cache = state
        .live_rate_limits
        .lock()
        .map_err(|_| "Live rate-limit cache is unavailable.".to_string())?;
    *cache = Some(CachedRateLimitSnapshot {
        fetched_at: Instant::now(),
        snapshot: snapshot.clone(),
    });
    Ok(())
}

fn best_effort_live_rate_limits(
    state: &AppState,
    force_refresh: bool,
) -> Option<LiveRateLimitSnapshot> {
    match get_live_rate_limits(state, force_refresh) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            log::warn!("Failed to refresh live rate limits for popup: {error}");
            get_live_rate_limits_local(state)
        }
    }
}

fn build_menu_bar_popup_snapshot(
    state: &AppState,
    force_refresh: bool,
) -> Result<MenuBarPopupSnapshot, String> {
    let conn = open_connection(&state.db_path).map_err(|error| error.to_string())?;
    let settings = get_sync_settings(&conn).map_err(|error| error.to_string())?;
    let live_rate_limits = if force_refresh {
        best_effort_live_rate_limits(state, true)
    } else {
        get_live_rate_limits_local(state)
    };
    let selected_bucket = normalize_menu_bar_bucket(&settings.menu_bar_bucket);
    let anchor =
        bucket_uses_anchor(&selected_bucket).then(|| Local::now().format("%Y-%m-%d").to_string());
    let overview = get_overview(
        &state.db_path,
        Some(selected_bucket.clone()),
        anchor,
        None,
        None,
        if bucket_uses_live_rate_limits(&selected_bucket) {
            live_rate_limits.clone()
        } else {
            None
        },
        None,
        None,
    )
    .ok();
    let quota_trend_7d = if selected_bucket == "seven_day" {
        overview
            .as_ref()
            .map(|value| value.quota_trend.clone())
            .unwrap_or_default()
    } else {
        get_quota_trend(
            &state.db_path,
            "seven_day".to_string(),
            live_rate_limits.clone(),
        )
        .unwrap_or_default()
    };

    Ok(MenuBarPopupSnapshot {
        display_language: get_display_language(&conn).unwrap_or_else(|_| "zh-CN".to_string()),
        fetched_at: Local::now().to_rfc3339(),
        refresh_interval_seconds: settings.live_quota_refresh_interval_seconds,
        selected_bucket,
        quota_5h: live_rate_limits
            .as_ref()
            .and_then(|snapshot| snapshot.primary.as_ref().map(menu_bar_popup_quota_snapshot)),
        quota_7d: live_rate_limits.as_ref().and_then(|snapshot| {
            snapshot
                .secondary
                .as_ref()
                .map(menu_bar_popup_quota_snapshot)
        }),
        quota_trend_7d,
        suggested_speed_7d: live_rate_limits
            .as_ref()
            .and_then(|snapshot| snapshot.secondary.as_ref())
            .and_then(|window| menu_bar_popup_suggested_speed(window, &settings, Local::now())),
        speed_fast_threshold_percent: settings.menu_bar_speed_fast_threshold_percent,
        speed_slow_threshold_percent: settings.menu_bar_speed_slow_threshold_percent,
        api_value_selected_bucket: overview
            .as_ref()
            .map(|value| value.stats.api_value_usd)
            .unwrap_or(0.0),
        total_tokens_selected_bucket: overview
            .as_ref()
            .map(|value| value.stats.total_tokens)
            .unwrap_or(0),
        conversation_count_selected_bucket: overview
            .as_ref()
            .map(|value| value.stats.conversation_count)
            .unwrap_or(0),
        payoff_ratio: overview
            .as_ref()
            .map(|value| value.stats.payoff_ratio)
            .unwrap_or(0.0),
        last_scan_completed_at: settings.last_scan_completed_at,
        live_quota_fetched_at: live_rate_limits
            .as_ref()
            .map(|snapshot| snapshot.fetched_at.clone()),
        visible_modules: normalize_menu_bar_popup_modules(&settings.menu_bar_popup_modules),
        show_reset_timeline: settings.menu_bar_popup_show_reset_timeline,
        show_actions: settings.menu_bar_popup_show_actions,
    })
}

fn menu_bar_popup_quota_snapshot(window: &RateLimitWindowSnapshot) -> MenuBarPopupQuotaSnapshot {
    MenuBarPopupQuotaSnapshot {
        used_percent: window.used_percent,
        remaining_percent: window.remaining_percent,
        window_duration_mins: window.window_duration_mins,
        resets_at: window.resets_at.clone(),
        window_start: window.window_start.clone(),
    }
}

fn menu_bar_popup_suggested_speed(
    window: &RateLimitWindowSnapshot,
    settings: &SyncSettings,
    now: DateTime<Local>,
) -> Option<MenuBarPopupSuggestedSpeed> {
    let velocity = suggested_usage_velocity(window, now, settings)?;
    let fast_threshold = settings
        .menu_bar_speed_fast_threshold_percent
        .clamp(0, 1000) as f64;
    let slow_threshold = settings
        .menu_bar_speed_slow_threshold_percent
        .clamp(0, 1000)
        .max(
            settings
                .menu_bar_speed_fast_threshold_percent
                .clamp(0, 1000),
        ) as f64;
    let percent = velocity_ratio_percent(window, now);

    Some(MenuBarPopupSuggestedSpeed {
        percent: percent.round() as i64,
        display_value: velocity.display_value,
        emoji: velocity.emoji,
        status: usage_velocity_status(percent, fast_threshold, slow_threshold).to_string(),
        remaining_time_percent: velocity.remaining_time_percent,
        remaining_percent: window.remaining_percent.clamp(0, 100),
    })
}

fn velocity_ratio_percent(window: &RateLimitWindowSnapshot, now: DateTime<Local>) -> f64 {
    let Some((window_start, reset_at)) = quota_window_bounds(window) else {
        return 0.0;
    };
    let total_seconds = reset_at.signed_duration_since(window_start).num_seconds();
    if total_seconds <= 0 {
        return 0.0;
    }

    let remaining_seconds = reset_at.signed_duration_since(now).num_seconds() as f64;
    let remaining_time_percent =
        ((remaining_seconds / total_seconds as f64) * 100.0).clamp(0.0, 100.0);
    if remaining_time_percent <= 0.0 {
        if window.remaining_percent <= 0 {
            100.0
        } else {
            1000.0
        }
    } else {
        ((window.remaining_percent.clamp(0, 100) as f64 / remaining_time_percent) * 100.0)
            .clamp(0.0, 1000.0)
    }
}

fn usage_velocity_status(percent: f64, fast_threshold: f64, slow_threshold: f64) -> &'static str {
    if percent < fast_threshold {
        "fast"
    } else if percent <= slow_threshold {
        "healthy"
    } else {
        "slow"
    }
}

fn live_rate_limit_cache_ttl(state: &AppState) -> Duration {
    open_connection(&state.db_path)
        .ok()
        .and_then(|conn| get_sync_settings(&conn).ok())
        .map(|settings| {
            Duration::from_secs(settings.live_quota_refresh_interval_seconds.clamp(60, 3600) as u64)
        })
        .unwrap_or(Duration::from_secs(300))
}

fn show_main_window(app: &AppHandle) {
    show_dock_icon_for_main_window(app);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn spawn_initial_scan(state: AppState) {
    tauri::async_runtime::spawn(async move {
        let _ = run_scan_if_idle(state, None);
    });
}

fn remote_sources_due_for_auto_update(
    sources: &[CodexSource],
    interval_minutes: i64,
    now: DateTime<chrono::Utc>,
) -> Vec<String> {
    let interval = ChronoDuration::minutes(interval_minutes.max(1));
    sources
        .iter()
        .filter(|source| source.kind == "ssh")
        .filter(|source| source.update_selected)
        .filter(|source| source.status != "downloading")
        .filter(|source| {
            let reference = source
                .last_downloaded_at
                .as_deref()
                .or_else(|| (source.status == "failed").then_some(source.updated_at.as_str()));
            let Some(reference) = reference else {
                return true;
            };
            DateTime::parse_from_rfc3339(reference)
                .map(|last| now.signed_duration_since(last.with_timezone(&chrono::Utc)) >= interval)
                .unwrap_or(true)
        })
        .map(|source| source.id.clone())
        .collect()
}

fn spawn_scheduler(app: AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            if state.scan_in_progress.load(Ordering::SeqCst) {
                continue;
            }

            let Ok(conn) = open_connection(&state.db_path) else {
                continue;
            };
            let Ok(settings) = get_sync_settings(&conn) else {
                continue;
            };
            if menu_bar_has_visible_content(&settings) {
                refresh_daily_value_menu_bar(&state);
            }

            if settings.auto_scan_enabled {
                let should_scan = match settings.last_scan_completed_at.as_deref() {
                    Some(last_completed_at) => {
                        chrono::DateTime::parse_from_rfc3339(last_completed_at)
                            .ok()
                            .map(|last| {
                                let elapsed = chrono::Utc::now()
                                    .signed_duration_since(last.with_timezone(&chrono::Utc));
                                elapsed.num_minutes() >= settings.auto_scan_interval_minutes.max(1)
                            })
                            .unwrap_or(true)
                    }
                    None => true,
                };

                if should_scan {
                    let _ = run_scan_if_idle(state.clone(), settings.codex_home.clone());
                    continue;
                }
            }

            if settings.remote_auto_update_enabled {
                let source_ids = list_codex_sources(&conn)
                    .map(|sources| {
                        remote_sources_due_for_auto_update(
                            &sources,
                            settings.remote_auto_update_interval_minutes,
                            chrono::Utc::now(),
                        )
                    })
                    .unwrap_or_default();
                drop(conn);
                if !source_ids.is_empty() {
                    let _ = run_source_download_if_idle(&app, state.clone(), source_ids);
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }));
    }

    builder
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Info
                } else {
                    log::LevelFilter::Warn
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };

            let state = window.state::<AppState>();
            let should_hide_to_menu_bar = if state.menu_bar_available {
                open_connection(&state.db_path)
                    .ok()
                    .and_then(|conn| get_sync_settings(&conn).ok())
                    .map(|settings| menu_bar_has_visible_content(&settings))
                    .unwrap_or(false)
            } else {
                false
            };

            if should_hide_to_menu_bar {
                api.prevent_close();
                let _ = window.hide();
                sync_dock_icon_visibility(window.app_handle(), state.inner());
            }
        })
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("Failed to resolve app data dir: {error}"))?;
            fs::create_dir_all(&app_data_dir).map_err(|error| {
                format!(
                    "Failed to create app data dir {}: {error}",
                    app_data_dir.display()
                )
            })?;
            let db_path = app_data_dir.join("codex-counter.sqlite");

            let conn = open_connection(&db_path).map_err(|error| error.to_string())?;
            init_db(&conn).map_err(|error| error.to_string())?;
            seed_pricing_catalog(&conn).map_err(|error| error.to_string())?;
            recalculate_all_session_values(&conn).map_err(|error| error.to_string())?;

            let app_handle = app.app_handle();
            #[cfg(target_os = "macos")]
            let menu_bar_available = match macos_menu_bar::configure(&app_handle) {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("Failed to set up native macOS menu bar popup: {error}");
                    false
                }
            };
            #[cfg(not(target_os = "macos"))]
            let menu_bar_available = false;
            let state = AppState {
                db_path,
                app_data_dir,
                scan_in_progress: Arc::new(AtomicBool::new(false)),
                menu_bar_available,
                live_rate_limits: Arc::new(Mutex::new(None)),
            };
            app.manage(state.clone());
            if let Ok(settings) = get_sync_settings(&conn) {
                apply_dock_icon_visibility(&app_handle, &settings, state.menu_bar_available);
            }
            refresh_daily_value_menu_bar(&state);
            spawn_initial_scan(state.clone());
            spawn_scheduler(app_handle.clone(), state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scanCodexUsage,
            scanCodexSources,
            getScanInProgress,
            refreshPricing,
            getOverview,
            listConversations,
            getLiveRateLimits,
            loadDashboard,
            discoverSshCodexSources,
            listCodexSources,
            upsertCodexSource,
            setCodexSourceSelected,
            setCodexSourceDisplaySelected,
            setCodexSourceUpdateSelected,
            deleteCodexSource,
            downloadCodexSource,
            downloadCodexSources,
            getConversationDetail,
            getSyncSettings,
            updateSyncSettings,
            updateDisplayLanguage,
            getSubscriptionProfile,
            updateSubscriptionProfile,
            listSubscriptionRecords,
            createSubscriptionRecord,
            updateSubscriptionRecord,
            deleteSubscriptionRecord,
            getCodexAccountStatus,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::insert_rate_limit_metadata_samples;
    use crate::models::{RateLimitCreditsSnapshot, RateLimitMetadataSampleRecord};
    use tempfile::tempdir;

    fn speed_test_settings() -> SyncSettings {
        SyncSettings {
            show_menu_bar_logo: true,
            menu_bar_speed_show_emoji: true,
            menu_bar_speed_fast_threshold_percent: 85,
            menu_bar_speed_slow_threshold_percent: 115,
            menu_bar_speed_healthy_emoji: "🟢".to_string(),
            menu_bar_speed_fast_emoji: "🔥".to_string(),
            menu_bar_speed_slow_emoji: "🐢".to_string(),
            ..SyncSettings::default()
        }
    }

    fn local_time(value: &str) -> DateTime<Local> {
        DateTime::parse_from_rfc3339(value)
            .expect("parse test timestamp")
            .with_timezone(&Local)
    }

    fn test_codex_source(
        id: &str,
        kind: &str,
        update_selected: bool,
        status: &str,
        last_downloaded_at: Option<&str>,
        updated_at: &str,
    ) -> CodexSource {
        CodexSource {
            id: id.to_string(),
            kind: kind.to_string(),
            label: id.to_string(),
            ssh_alias: None,
            host_name: None,
            user: None,
            port: None,
            remote_codex_home: None,
            local_codex_home: None,
            selected: update_selected,
            display_selected: true,
            update_selected,
            status: status.to_string(),
            last_discovered_at: None,
            last_downloaded_at: last_downloaded_at.map(str::to_string),
            last_scanned_at: None,
            last_error: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn remote_auto_update_only_tracks_due_selected_ssh_sources() {
        let now = DateTime::parse_from_rfc3339("2026-05-17T12:00:00+08:00")
            .expect("parse now")
            .with_timezone(&chrono::Utc);
        let sources = vec![
            test_codex_source(
                "local",
                "local",
                true,
                "ready",
                None,
                "2026-05-17T10:00:00+08:00",
            ),
            test_codex_source(
                "ssh_due",
                "ssh",
                true,
                "ready",
                Some("2026-05-17T11:20:00+08:00"),
                "2026-05-17T11:20:00+08:00",
            ),
            test_codex_source(
                "ssh_fresh",
                "ssh",
                true,
                "ready",
                Some("2026-05-17T11:45:00+08:00"),
                "2026-05-17T11:45:00+08:00",
            ),
            test_codex_source(
                "ssh_untracked",
                "ssh",
                false,
                "ready",
                Some("2026-05-17T10:00:00+08:00"),
                "2026-05-17T10:00:00+08:00",
            ),
        ];

        assert_eq!(
            remote_sources_due_for_auto_update(&sources, 30, now),
            vec!["ssh_due".to_string()]
        );
    }

    #[test]
    fn suggested_usage_speed_is_balanced_at_one() {
        let window = RateLimitWindowSnapshot {
            used_percent: 0,
            remaining_percent: 100,
            window_duration_mins: Some(300),
            resets_at: Some("2026-03-27T05:00:00+08:00".to_string()),
            window_start: Some("2026-03-27T00:00:00+08:00".to_string()),
        };

        let settings = speed_test_settings();
        let velocity =
            suggested_usage_velocity(&window, local_time("2026-03-27T00:00:00+08:00"), &settings)
                .expect("calculate velocity");

        assert_eq!(velocity.emoji, "🟢");
        assert_eq!(velocity.display_value, "100%");
    }

    #[test]
    fn suggested_usage_speed_caps_at_ten_plus() {
        let window = RateLimitWindowSnapshot {
            used_percent: 10,
            remaining_percent: 90,
            window_duration_mins: Some(10080),
            resets_at: Some("2026-03-27T00:00:00+08:00".to_string()),
            window_start: Some("2026-03-20T00:00:00+08:00".to_string()),
        };

        let settings = speed_test_settings();
        let velocity =
            suggested_usage_velocity(&window, local_time("2026-03-26T22:19:12+08:00"), &settings)
                .expect("calculate velocity");

        assert_eq!(velocity.emoji, "🐢");
        assert_eq!(velocity.display_value, "1000%+");
    }

    #[test]
    fn suggested_usage_speed_marks_fast_usage() {
        let window = RateLimitWindowSnapshot {
            used_percent: 80,
            remaining_percent: 20,
            window_duration_mins: Some(300),
            resets_at: Some("2026-03-27T05:00:00+08:00".to_string()),
            window_start: Some("2026-03-27T00:00:00+08:00".to_string()),
        };

        let settings = speed_test_settings();
        let velocity =
            suggested_usage_velocity(&window, local_time("2026-03-27T03:00:00+08:00"), &settings)
                .expect("calculate velocity");

        assert_eq!(velocity.emoji, "🔥");
        assert_eq!(velocity.display_value, "50%");
    }

    #[test]
    fn suggested_usage_speed_respects_custom_thresholds_and_hidden_emoji() {
        let window = RateLimitWindowSnapshot {
            used_percent: 30,
            remaining_percent: 70,
            window_duration_mins: Some(300),
            resets_at: Some("2026-03-27T05:00:00+08:00".to_string()),
            window_start: Some("2026-03-27T00:00:00+08:00".to_string()),
        };

        let settings = SyncSettings {
            menu_bar_speed_show_emoji: false,
            menu_bar_speed_fast_threshold_percent: 60,
            menu_bar_speed_slow_threshold_percent: 90,
            menu_bar_speed_healthy_emoji: "OK".to_string(),
            menu_bar_speed_fast_emoji: "FAST".to_string(),
            menu_bar_speed_slow_emoji: "SLOW".to_string(),
            ..SyncSettings::default()
        };

        let velocity =
            suggested_usage_velocity(&window, local_time("2026-03-27T03:00:00+08:00"), &settings)
                .expect("calculate velocity");

        assert_eq!(velocity.emoji, "");
        assert_eq!(velocity.rendered_value(), "175%");
    }

    #[test]
    fn menu_bar_title_joins_visible_segments_without_extra_spacing() {
        assert_eq!(menu_bar_title(None, None), None);
        assert_eq!(
            menu_bar_title(Some("$12.4"), None),
            Some("$12.4".to_string())
        );
        assert_eq!(menu_bar_title(None, Some("67%")), Some("67%".to_string()));
        assert_eq!(
            menu_bar_title(Some("$12.4"), Some("67%")),
            Some("$12.4 67%".to_string())
        );
    }

    #[test]
    fn menu_bar_can_show_live_metric_without_logo_or_api_value() {
        let settings = SyncSettings {
            show_menu_bar_logo: false,
            show_menu_bar_daily_api_value: false,
            show_menu_bar_live_quota_percent: true,
            menu_bar_live_quota_metric: "remaining_percent".to_string(),
            menu_bar_live_quota_bucket: "five_hour".to_string(),
            ..SyncSettings::default()
        };

        assert!(menu_bar_has_visible_content(&settings));
        assert_eq!(menu_bar_title(None, Some("42%")), Some("42%".to_string()));
    }

    #[test]
    fn menu_bar_live_metric_label_matches_bucket_and_metric() {
        assert_eq!(
            menu_bar_live_quota_status_label("seven_day", "remaining_percent"),
            "7d Rest"
        );
        assert_eq!(
            menu_bar_live_quota_status_label("five_hour", "remaining_percent"),
            "5h Rest"
        );
        assert_eq!(
            menu_bar_live_quota_status_label("seven_day", "used_percent"),
            "7d Cost"
        );
        assert_eq!(
            menu_bar_live_quota_status_label("seven_day", "suggested_usage_speed"),
            "7d Pace"
        );
    }

    #[test]
    fn menu_bar_live_metric_can_show_used_percent_cost() {
        let snapshot = LiveRateLimitSnapshot {
            limit_id: Some("codex".to_string()),
            limit_name: None,
            plan_type: Some("pro".to_string()),
            credits: None,
            rate_limit_reached_type: None,
            primary: None,
            secondary: Some(RateLimitWindowSnapshot {
                used_percent: 15,
                remaining_percent: 85,
                window_duration_mins: Some(10080),
                resets_at: Some("2026-05-22T03:29:00+08:00".to_string()),
                window_start: Some("2026-05-15T03:29:00+08:00".to_string()),
            }),
            fetched_at: "2026-05-15T20:00:00+08:00".to_string(),
        };

        assert_eq!(
            menu_bar_live_quota_title(
                &snapshot,
                &speed_test_settings(),
                "seven_day",
                "used_percent",
                local_time("2026-05-15T20:00:00+08:00"),
            ),
            Some(("7d Cost".to_string(), "15%".to_string()))
        );
    }

    #[test]
    fn menu_bar_can_hide_completely_when_all_display_content_is_disabled() {
        let settings = SyncSettings {
            show_menu_bar_logo: false,
            show_menu_bar_daily_api_value: false,
            show_menu_bar_live_quota_percent: false,
            ..SyncSettings::default()
        };

        assert!(!menu_bar_has_visible_content(&settings));
    }

    #[test]
    fn dock_icon_hides_only_when_enabled_and_menu_bar_has_content() {
        let enabled_with_menu_bar = SyncSettings {
            hide_dock_icon_when_menu_bar_visible: true,
            show_menu_bar_logo: true,
            show_menu_bar_daily_api_value: false,
            show_menu_bar_live_quota_percent: false,
            ..SyncSettings::default()
        };
        let enabled_without_menu_bar = SyncSettings {
            hide_dock_icon_when_menu_bar_visible: true,
            show_menu_bar_logo: false,
            show_menu_bar_daily_api_value: false,
            show_menu_bar_live_quota_percent: false,
            ..SyncSettings::default()
        };
        let disabled_with_menu_bar = SyncSettings {
            hide_dock_icon_when_menu_bar_visible: false,
            show_menu_bar_logo: true,
            show_menu_bar_daily_api_value: false,
            show_menu_bar_live_quota_percent: false,
            ..SyncSettings::default()
        };

        assert!(should_hide_dock_icon(&enabled_with_menu_bar));
        assert!(!should_hide_dock_icon(&enabled_without_menu_bar));
        assert!(!should_hide_dock_icon(&disabled_with_menu_bar));
    }

    #[test]
    fn fallback_snapshot_includes_latest_rate_limit_metadata() {
        let directory = tempdir().expect("tempdir");
        let db_path = directory.path().join("usage.sqlite");
        let conn = open_connection(&db_path).expect("open db");
        init_db(&conn).expect("init db");

        insert_live_rate_limit_snapshot(
            &conn,
            &LiveRateLimitSnapshot {
                limit_id: Some("codex".to_string()),
                limit_name: None,
                plan_type: Some("old-plan".to_string()),
                credits: None,
                rate_limit_reached_type: None,
                primary: Some(RateLimitWindowSnapshot {
                    used_percent: 20,
                    remaining_percent: 80,
                    window_duration_mins: Some(300),
                    resets_at: Some("2026-05-15T15:00:00+08:00".to_string()),
                    window_start: Some("2026-05-15T10:00:00+08:00".to_string()),
                }),
                secondary: None,
                fetched_at: "2026-05-15T12:00:00+08:00".to_string(),
            },
        )
        .expect("insert live window");
        insert_rate_limit_metadata_samples(
            &conn,
            &[RateLimitMetadataSampleRecord {
                source_kind: "live".to_string(),
                source_session_id: None,
                sample_timestamp: "2026-05-15T12:05:00+08:00".to_string(),
                limit_id: Some("codex".to_string()),
                limit_name: None,
                plan_type: Some("pro".to_string()),
                credits: Some(RateLimitCreditsSnapshot {
                    has_credits: Some(true),
                    unlimited: Some(false),
                    balance: Some("bonus-balance".to_string()),
                }),
                rate_limit_reached_type: Some("primary".to_string()),
                raw_rate_limits_json: None,
            }],
        )
        .expect("insert metadata");
        conn.execute(
            "
        INSERT INTO rate_limit_samples (
          source_kind, source_session_id, bucket, sample_timestamp, limit_id, limit_name, plan_type,
          window_start, resets_at, used_percent, remaining_percent, created_at
        )
        VALUES
          ('live', '', 'five_hour', '2026-05-15T12:10:00+08:00', 'codex_bengalfox', 'GPT-5.3-Codex-Spark', 'pro',
           '2026-05-15T10:00:00+08:00', '2026-05-15T15:00:00+08:00', 3, 97, '2026-05-15T12:10:00+08:00'),
          ('live', '', 'seven_day', '2026-05-15T12:10:00+08:00', 'codex_bengalfox', 'GPT-5.3-Codex-Spark', 'pro',
           '2026-05-15T10:00:00+08:00', '2026-05-22T10:00:00+08:00', 2, 98, '2026-05-15T12:10:00+08:00')
        ",
            [],
        )
        .expect("insert model-specific windows");
        conn.execute(
            "
        INSERT INTO rate_limit_metadata_samples (
          source_kind, source_session_id, sample_timestamp, limit_id, limit_name, plan_type,
          credits_has_credits, credits_unlimited, credits_balance, rate_limit_reached_type,
          raw_rate_limits_json, created_at
        )
        VALUES (
          'live', '', '2026-05-15T12:11:00+08:00', 'codex_bengalfox', 'GPT-5.3-Codex-Spark', 'pro',
          1, 0, 'spark-balance', '', NULL, '2026-05-15T12:11:00+08:00'
        )
        ",
            [],
        )
        .expect("insert model-specific metadata");

        let state = AppState {
            db_path,
            app_data_dir: directory.path().join("app-data"),
            scan_in_progress: Arc::new(AtomicBool::new(false)),
            menu_bar_available: false,
            live_rate_limits: Arc::new(Mutex::new(None)),
        };

        let snapshot =
            load_persisted_live_rate_limits_for_source(&state, None).expect("fallback snapshot");

        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.limit_id.as_deref(), Some("codex"));
        assert_eq!(snapshot.plan_type.as_deref(), Some("pro"));
        assert_eq!(
            snapshot
                .primary
                .as_ref()
                .map(|window| window.remaining_percent),
            Some(80)
        );
        assert_eq!(
            snapshot
                .credits
                .as_ref()
                .and_then(|credits| credits.balance.as_deref()),
            Some("bonus-balance")
        );
        assert_eq!(snapshot.rate_limit_reached_type.as_deref(), Some("primary"));
        assert_eq!(snapshot.fetched_at, "2026-05-15T12:05:00+08:00");
    }
}
