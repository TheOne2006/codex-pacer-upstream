use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use rusqlite::{params, Connection};

mod rate_limit_samples;
mod sources;
mod subscriptions;
mod sync_settings;

pub use rate_limit_samples::{insert_live_rate_limit_snapshot, replace_session_rate_limit_samples};
pub use sources::{
    delete_codex_source, ensure_local_codex_source, get_codex_source, list_codex_sources,
    set_codex_source_selected, update_codex_source_download_state, upsert_ssh_codex_source,
};
pub use subscriptions::{
    canonical_subscription_currency, create_subscription_record, delete_subscription_record,
    get_subscription_profile, list_subscription_records, save_subscription_profile,
    update_subscription_record,
};
pub use sync_settings::{
    get_display_language, get_sync_settings, save_sync_settings, set_display_language,
    set_last_scan_completed, set_last_scan_started,
};

pub fn now_utc_string() -> String {
    Utc::now().to_rfc3339()
}

pub fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

pub fn i64_to_bool(value: i64) -> bool {
    value != 0
}

pub fn open_connection(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

pub fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(include_str!("../sql/schema.sql"))?;

    ensure_sync_settings_schema(conn)?;
    ensure_singletons(conn)?;
    ensure_local_codex_source(conn, None)?;

    conn.execute_batch(include_str!("../sql/indexes.sql"))?;
    Ok(())
}

fn ensure_sync_settings_schema(conn: &Connection) -> rusqlite::Result<()> {
    let has_display_language: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('sync_settings') WHERE name = 'display_language'",
        [],
        |row| row.get(0),
    )?;
    if has_display_language == 0 {
        conn.execute(
            "ALTER TABLE sync_settings ADD COLUMN display_language TEXT NOT NULL DEFAULT 'zh-CN'",
            [],
        )?;
    }
    Ok(())
}

fn ensure_singletons(conn: &Connection) -> rusqlite::Result<()> {
    let now = now_utc_string();
    conn.execute(
        "
    INSERT INTO subscription_profile (
      singleton_id, plan_type, currency, monthly_price, billing_anchor_day, updated_at
    )
    VALUES (1, 'plus', ?2, 20.0, 1, ?1)
    ON CONFLICT(singleton_id) DO NOTHING
    ",
        params![now, canonical_subscription_currency()],
    )?;

    let now = now_utc_string();
    conn.execute(
        "
    INSERT INTO sync_settings (
      singleton_id, sync_settings_schema_version,
      codex_home, auto_scan_enabled, auto_scan_interval_minutes,
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
    VALUES (1, 4, NULL, 1, 5, 300, 1, 1, 1, 0, 'remaining_percent', 'five_hour', 'day', 1, 85, 115, '🟢', '🔥', '🐢', 1, ?2, 1, 1, NULL, NULL, ?1)
    ON CONFLICT(singleton_id) DO NOTHING
    ",
        params![now, sync_settings::default_menu_bar_popup_modules_json()],
    )?;

    conn.execute(
        "
    UPDATE sync_settings
    SET sync_settings_schema_version = 4,
        updated_at = ?1
    WHERE singleton_id = 1
      AND sync_settings_schema_version >= 3
      AND sync_settings_schema_version < 4
    ",
        params![now],
    )?;

    conn.execute(
        "
    UPDATE sync_settings
    SET sync_settings_schema_version = 4,
        hide_dock_icon_when_menu_bar_visible = 1,
        updated_at = ?1
    WHERE singleton_id = 1
      AND sync_settings_schema_version < 3
    ",
        params![now],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_db_creates_current_schema_and_bootstrap_records() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        init_db(&conn).expect("init database");

        let source_index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN ('idx_sessions_source_id', 'idx_import_state_source_id')",
                [],
                |row| row.get(0),
            )
            .expect("source indexes");
        assert_eq!(source_index_count, 2);

        let sources = list_codex_sources(&conn).expect("list sources");
        assert!(sources.iter().any(|source| source.id == "local"));

        let settings = get_sync_settings(&conn).expect("load settings");
        assert!(settings.show_menu_bar_logo);
        assert!(settings.hide_dock_icon_when_menu_bar_visible);
        assert_eq!(settings.live_quota_refresh_interval_seconds, 300);
        assert_eq!(
            settings.menu_bar_popup_modules,
            vec!["api_value".to_string(), "scan_freshness".to_string()]
        );
    }
}
