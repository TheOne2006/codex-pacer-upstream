use rusqlite::{params, Connection};

use crate::models::SyncSettings;

use super::{bool_to_i64, i64_to_bool, now_utc_string};

pub(super) fn default_menu_bar_popup_modules() -> Vec<String> {
    vec!["api_value".to_string(), "scan_freshness".to_string()]
}

fn is_supported_menu_bar_popup_module(module: &str) -> bool {
    matches!(
        module,
        "api_value"
            | "token_count"
            | "scan_freshness"
            | "live_quota_freshness"
            | "payoff_ratio"
            | "conversation_count"
    )
}

pub(super) fn default_menu_bar_popup_modules_json() -> String {
    serde_json::to_string(&default_menu_bar_popup_modules())
        .expect("serialize default popup modules")
}

fn deserialize_menu_bar_popup_modules(value: String) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(&value)
        .ok()
        .map(|modules| {
            modules
                .into_iter()
                .filter(|module| is_supported_menu_bar_popup_module(module))
                .fold(Vec::new(), |mut deduped, module| {
                    if !deduped.iter().any(|existing| existing == &module) {
                        deduped.push(module);
                    }
                    deduped
                })
        })
        .unwrap_or_else(default_menu_bar_popup_modules)
}

fn serialize_menu_bar_popup_modules(modules: &[String]) -> String {
    serde_json::to_string(modules).unwrap_or_else(|_| default_menu_bar_popup_modules_json())
}

fn normalize_display_language(value: &str) -> String {
    match value {
        "en" => "en".to_string(),
        "zh-CN" => "zh-CN".to_string(),
        _ => "zh-CN".to_string(),
    }
}

pub fn get_display_language(conn: &Connection) -> rusqlite::Result<String> {
    let value: String = conn.query_row(
        "
    SELECT display_language
    FROM sync_settings
    WHERE singleton_id = 1
    ",
        [],
        |row| row.get(0),
    )?;
    Ok(normalize_display_language(&value))
}

pub fn set_display_language(conn: &Connection, language: &str) -> rusqlite::Result<String> {
    let normalized = normalize_display_language(language);
    let updated_at = now_utc_string();
    conn.execute(
        "
    UPDATE sync_settings
    SET display_language = ?1,
        updated_at = ?2
    WHERE singleton_id = 1
    ",
        params![normalized, updated_at],
    )?;
    get_display_language(conn)
}

pub fn get_sync_settings(conn: &Connection) -> rusqlite::Result<SyncSettings> {
    conn.query_row(
        "
    SELECT codex_home, auto_scan_enabled, auto_scan_interval_minutes,
           remote_auto_update_enabled, remote_auto_update_interval_minutes,
           live_quota_refresh_interval_seconds, hide_dock_icon_when_menu_bar_visible,
           show_menu_bar_logo, show_menu_bar_daily_api_value,
           show_menu_bar_live_quota_percent, menu_bar_live_quota_metric,
           menu_bar_live_quota_bucket, menu_bar_bucket,
           menu_bar_speed_show_emoji, menu_bar_speed_fast_threshold_percent,
           menu_bar_speed_slow_threshold_percent, menu_bar_speed_healthy_emoji,
           menu_bar_speed_fast_emoji, menu_bar_speed_slow_emoji,
           menu_bar_popup_enabled, menu_bar_popup_modules,
           menu_bar_popup_show_reset_timeline, menu_bar_popup_show_actions,
           last_scan_started_at, last_scan_completed_at, updated_at
    FROM sync_settings
    WHERE singleton_id = 1
    ",
        [],
        |row| {
            Ok(SyncSettings {
                codex_home: row.get(0)?,
                auto_scan_enabled: i64_to_bool(row.get::<_, i64>(1)?),
                auto_scan_interval_minutes: row.get(2)?,
                remote_auto_update_enabled: i64_to_bool(row.get::<_, i64>(3)?),
                remote_auto_update_interval_minutes: row.get(4)?,
                live_quota_refresh_interval_seconds: row.get(5)?,
                hide_dock_icon_when_menu_bar_visible: i64_to_bool(row.get::<_, i64>(6)?),
                show_menu_bar_logo: i64_to_bool(row.get::<_, i64>(7)?),
                show_menu_bar_daily_api_value: i64_to_bool(row.get::<_, i64>(8)?),
                show_menu_bar_live_quota_percent: i64_to_bool(row.get::<_, i64>(9)?),
                menu_bar_live_quota_metric: row.get(10)?,
                menu_bar_live_quota_bucket: row.get(11)?,
                menu_bar_bucket: row.get(12)?,
                menu_bar_speed_show_emoji: i64_to_bool(row.get::<_, i64>(13)?),
                menu_bar_speed_fast_threshold_percent: row.get(14)?,
                menu_bar_speed_slow_threshold_percent: row.get(15)?,
                menu_bar_speed_healthy_emoji: row.get(16)?,
                menu_bar_speed_fast_emoji: row.get(17)?,
                menu_bar_speed_slow_emoji: row.get(18)?,
                menu_bar_popup_enabled: i64_to_bool(row.get::<_, i64>(19)?),
                menu_bar_popup_modules: deserialize_menu_bar_popup_modules(row.get(20)?),
                menu_bar_popup_show_reset_timeline: i64_to_bool(row.get::<_, i64>(21)?),
                menu_bar_popup_show_actions: i64_to_bool(row.get::<_, i64>(22)?),
                last_scan_started_at: row.get(23)?,
                last_scan_completed_at: row.get(24)?,
                updated_at: row.get(25)?,
            })
        },
    )
}

pub fn save_sync_settings(
    conn: &Connection,
    settings: &SyncSettings,
) -> rusqlite::Result<SyncSettings> {
    let updated_at = now_utc_string();
    conn.execute(
        "
    INSERT INTO sync_settings (
      singleton_id, codex_home, auto_scan_enabled, auto_scan_interval_minutes,
      remote_auto_update_enabled, remote_auto_update_interval_minutes,
      live_quota_refresh_interval_seconds, hide_dock_icon_when_menu_bar_visible,
      show_menu_bar_logo,
      show_menu_bar_daily_api_value,
      show_menu_bar_live_quota_percent, menu_bar_live_quota_metric,
      menu_bar_live_quota_bucket, menu_bar_bucket,
      menu_bar_speed_show_emoji, menu_bar_speed_fast_threshold_percent,
      menu_bar_speed_slow_threshold_percent, menu_bar_speed_healthy_emoji,
      menu_bar_speed_fast_emoji, menu_bar_speed_slow_emoji,
      menu_bar_popup_enabled, menu_bar_popup_modules,
      menu_bar_popup_show_reset_timeline, menu_bar_popup_show_actions,
      last_scan_started_at, last_scan_completed_at, updated_at
    )
    VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
    ON CONFLICT(singleton_id) DO UPDATE SET
      codex_home = excluded.codex_home,
      auto_scan_enabled = excluded.auto_scan_enabled,
      auto_scan_interval_minutes = excluded.auto_scan_interval_minutes,
      remote_auto_update_enabled = excluded.remote_auto_update_enabled,
      remote_auto_update_interval_minutes = excluded.remote_auto_update_interval_minutes,
      live_quota_refresh_interval_seconds = excluded.live_quota_refresh_interval_seconds,
      hide_dock_icon_when_menu_bar_visible = excluded.hide_dock_icon_when_menu_bar_visible,
      show_menu_bar_logo = excluded.show_menu_bar_logo,
      show_menu_bar_daily_api_value = excluded.show_menu_bar_daily_api_value,
      show_menu_bar_live_quota_percent = excluded.show_menu_bar_live_quota_percent,
      menu_bar_live_quota_metric = excluded.menu_bar_live_quota_metric,
      menu_bar_live_quota_bucket = excluded.menu_bar_live_quota_bucket,
      menu_bar_bucket = excluded.menu_bar_bucket,
      menu_bar_speed_show_emoji = excluded.menu_bar_speed_show_emoji,
      menu_bar_speed_fast_threshold_percent = excluded.menu_bar_speed_fast_threshold_percent,
      menu_bar_speed_slow_threshold_percent = excluded.menu_bar_speed_slow_threshold_percent,
      menu_bar_speed_healthy_emoji = excluded.menu_bar_speed_healthy_emoji,
      menu_bar_speed_fast_emoji = excluded.menu_bar_speed_fast_emoji,
      menu_bar_speed_slow_emoji = excluded.menu_bar_speed_slow_emoji,
      menu_bar_popup_enabled = excluded.menu_bar_popup_enabled,
      menu_bar_popup_modules = excluded.menu_bar_popup_modules,
      menu_bar_popup_show_reset_timeline = excluded.menu_bar_popup_show_reset_timeline,
      menu_bar_popup_show_actions = excluded.menu_bar_popup_show_actions,
      last_scan_started_at = excluded.last_scan_started_at,
      last_scan_completed_at = excluded.last_scan_completed_at,
      updated_at = excluded.updated_at
    ",
        params![
            settings.codex_home,
            bool_to_i64(settings.auto_scan_enabled),
            settings.auto_scan_interval_minutes.max(1),
            bool_to_i64(settings.remote_auto_update_enabled),
            settings.remote_auto_update_interval_minutes.max(1),
            settings.live_quota_refresh_interval_seconds.clamp(60, 3600),
            bool_to_i64(settings.hide_dock_icon_when_menu_bar_visible),
            bool_to_i64(settings.show_menu_bar_logo),
            bool_to_i64(settings.show_menu_bar_daily_api_value),
            bool_to_i64(settings.show_menu_bar_live_quota_percent),
            settings.menu_bar_live_quota_metric,
            settings.menu_bar_live_quota_bucket,
            settings.menu_bar_bucket,
            bool_to_i64(settings.menu_bar_speed_show_emoji),
            settings.menu_bar_speed_fast_threshold_percent.clamp(0, 1000),
            settings.menu_bar_speed_slow_threshold_percent.clamp(0, 1000),
            settings.menu_bar_speed_healthy_emoji,
            settings.menu_bar_speed_fast_emoji,
            settings.menu_bar_speed_slow_emoji,
            bool_to_i64(settings.menu_bar_popup_enabled),
            serialize_menu_bar_popup_modules(&settings.menu_bar_popup_modules),
            bool_to_i64(settings.menu_bar_popup_show_reset_timeline),
            bool_to_i64(settings.menu_bar_popup_show_actions),
            settings.last_scan_started_at,
            settings.last_scan_completed_at,
            updated_at,
        ],
    )?;
    get_sync_settings(conn)
}

pub fn set_last_scan_started(conn: &Connection, timestamp: &str) -> rusqlite::Result<()> {
    conn.execute(
        "
    UPDATE sync_settings
    SET last_scan_started_at = ?1, updated_at = ?1
    WHERE singleton_id = 1
    ",
        params![timestamp],
    )?;
    Ok(())
}

pub fn set_last_scan_completed(conn: &Connection, timestamp: &str) -> rusqlite::Result<()> {
    conn.execute(
        "
    UPDATE sync_settings
    SET last_scan_completed_at = ?1, updated_at = ?1
    WHERE singleton_id = 1
    ",
        params![timestamp],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_db;

    #[test]
    fn save_sync_settings_round_trips_dock_visibility_preference() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        init_db(&conn).expect("init database");

        save_sync_settings(
            &conn,
            &SyncSettings {
                hide_dock_icon_when_menu_bar_visible: true,
                ..SyncSettings::default()
            },
        )
        .expect("save settings");

        let settings = get_sync_settings(&conn).expect("load settings");

        assert!(settings.hide_dock_icon_when_menu_bar_visible);
    }
}
