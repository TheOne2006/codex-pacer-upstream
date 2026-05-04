use rusqlite::{params, Connection};

use crate::models::{CodexSource, CodexSourceInput};

use super::{bool_to_i64, i64_to_bool, now_utc_string};

pub fn ensure_local_codex_source(
    conn: &Connection,
    local_codex_home: Option<&str>,
) -> rusqlite::Result<()> {
    let now = now_utc_string();
    conn.execute(
        "
    INSERT INTO codex_sources (
      id, kind, label, local_codex_home, selected, status, created_at, updated_at
    )
    VALUES ('local', 'local', 'localhost', ?1, 1, 'ready', ?2, ?2)
    ON CONFLICT(id) DO UPDATE SET
      local_codex_home = COALESCE(?1, codex_sources.local_codex_home),
      updated_at = excluded.updated_at
    ",
        params![local_codex_home, now],
    )?;
    Ok(())
}

pub fn list_codex_sources(conn: &Connection) -> rusqlite::Result<Vec<CodexSource>> {
    ensure_local_codex_source(conn, None)?;
    let mut stmt = conn.prepare(
        "
    SELECT id, kind, label, ssh_alias, host_name, user, port, remote_codex_home, local_codex_home,
           selected, status, last_discovered_at, last_downloaded_at, last_scanned_at,
           last_error, created_at, updated_at
    FROM codex_sources
    ORDER BY CASE WHEN id = 'local' THEN 0 ELSE 1 END, label COLLATE NOCASE ASC, id ASC
    ",
    )?;
    let rows = stmt
        .query_map([], codex_source_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_codex_source(conn: &Connection, id: &str) -> rusqlite::Result<CodexSource> {
    conn.query_row(
        "
    SELECT id, kind, label, ssh_alias, host_name, user, port, remote_codex_home, local_codex_home,
           selected, status, last_discovered_at, last_downloaded_at, last_scanned_at,
           last_error, created_at, updated_at
    FROM codex_sources
    WHERE id = ?1
    ",
        params![id],
        codex_source_from_row,
    )
}

pub fn upsert_ssh_codex_source(
    conn: &Connection,
    id: &str,
    input: &CodexSourceInput,
    local_codex_home: &str,
) -> rusqlite::Result<CodexSource> {
    let now = now_utc_string();
    conn.execute(
        "
    INSERT INTO codex_sources (
      id, kind, label, ssh_alias, host_name, user, port, remote_codex_home, local_codex_home,
      selected, status, last_discovered_at, created_at, updated_at
    )
    VALUES (?1, 'ssh', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'idle', ?10, ?10, ?10)
    ON CONFLICT(id) DO UPDATE SET
      label = excluded.label,
      ssh_alias = excluded.ssh_alias,
      host_name = excluded.host_name,
      user = excluded.user,
      port = excluded.port,
      remote_codex_home = excluded.remote_codex_home,
      local_codex_home = excluded.local_codex_home,
      selected = excluded.selected,
      last_discovered_at = excluded.last_discovered_at,
      updated_at = excluded.updated_at
    ",
        params![
            id,
            normalize_label(&input.label, &input.ssh_alias),
            input.ssh_alias.trim(),
            input
                .host_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            input
                .user
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            input.port,
            normalize_remote_codex_home(&input.remote_codex_home),
            local_codex_home,
            bool_to_i64(input.selected),
            now,
        ],
    )?;
    get_codex_source(conn, id)
}

pub fn set_codex_source_selected(
    conn: &Connection,
    id: &str,
    selected: bool,
) -> rusqlite::Result<CodexSource> {
    let now = now_utc_string();
    conn.execute(
        "UPDATE codex_sources SET selected = ?1, updated_at = ?2 WHERE id = ?3",
        params![bool_to_i64(selected), now, id],
    )?;
    get_codex_source(conn, id)
}

pub fn update_codex_source_download_state(
    conn: &Connection,
    id: &str,
    status: &str,
    last_downloaded_at: Option<&str>,
    last_scanned_at: Option<&str>,
    error: Option<&str>,
) -> rusqlite::Result<CodexSource> {
    let now = now_utc_string();
    conn.execute(
        "
    UPDATE codex_sources
    SET status = ?1,
        last_downloaded_at = COALESCE(?2, last_downloaded_at),
        last_scanned_at = COALESCE(?3, last_scanned_at),
        last_error = ?4,
        updated_at = ?5
    WHERE id = ?6
    ",
        params![status, last_downloaded_at, last_scanned_at, error, now, id],
    )?;
    get_codex_source(conn, id)
}

pub fn delete_codex_source(conn: &mut Connection, id: &str) -> rusqlite::Result<bool> {
    if id == "local" {
        return Ok(false);
    }

    let tx = conn.transaction()?;
    tx.execute(
        "
    DELETE FROM usage_events
    WHERE session_id IN (SELECT session_id FROM sessions WHERE source_id = ?1)
    ",
        params![id],
    )?;
    tx.execute(
        "
    DELETE FROM session_overrides
    WHERE session_id IN (SELECT session_id FROM sessions WHERE source_id = ?1)
    ",
        params![id],
    )?;
    tx.execute(
        "
    DELETE FROM conversation_links
    WHERE session_id IN (SELECT session_id FROM sessions WHERE source_id = ?1)
       OR root_session_id IN (SELECT session_id FROM sessions WHERE source_id = ?1)
       OR parent_session_id IN (SELECT session_id FROM sessions WHERE source_id = ?1)
    ",
        params![id],
    )?;
    tx.execute("DELETE FROM sessions WHERE source_id = ?1", params![id])?;
    tx.execute("DELETE FROM import_state WHERE source_id = ?1", params![id])?;
    tx.execute(
        "DELETE FROM rate_limit_samples WHERE source_id = ?1",
        params![id],
    )?;
    let deleted = tx.execute(
        "DELETE FROM codex_sources WHERE id = ?1 AND kind <> 'local'",
        params![id],
    )?;
    tx.commit()?;
    Ok(deleted > 0)
}

fn codex_source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodexSource> {
    Ok(CodexSource {
        id: row.get(0)?,
        kind: row.get(1)?,
        label: row.get(2)?,
        ssh_alias: row.get(3)?,
        host_name: row.get(4)?,
        user: row.get(5)?,
        port: row.get(6)?,
        remote_codex_home: row.get(7)?,
        local_codex_home: row.get(8)?,
        selected: i64_to_bool(row.get::<_, i64>(9)?),
        status: row.get(10)?,
        last_discovered_at: row.get(11)?,
        last_downloaded_at: row.get(12)?,
        last_scanned_at: row.get(13)?,
        last_error: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn normalize_label(label: &str, fallback: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        fallback.trim().chars().take(80).collect()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn normalize_remote_codex_home(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "~/.codex".to_string()
    } else {
        trimmed.chars().take(500).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_db;

    #[test]
    fn delete_codex_source_removes_imported_source_data() {
        let mut conn = Connection::open_in_memory().expect("open in-memory database");
        init_db(&conn).expect("init database");
        upsert_ssh_codex_source(
            &conn,
            "ssh_test_box",
            &CodexSourceInput {
                label: "test_box".to_string(),
                ssh_alias: "test_box".to_string(),
                host_name: Some("10.0.0.2".to_string()),
                user: Some("me".to_string()),
                port: Some(22),
                remote_codex_home: "~/.codex".to_string(),
                selected: true,
            },
            "/tmp/codex-source",
        )
        .expect("create source");

        conn.execute_batch(
            "
        INSERT INTO sessions (
          session_id, source_id, root_session_id, parent_session_id, title, source_state,
          source_path, source_bucket, started_at, updated_at, agent_nickname, agent_role,
          explicit_fast_mode, fast_mode_default, latest_plan_type, last_model_id,
          contains_subagents, created_at, imported_at
        )
        VALUES (
          'session-1', 'ssh_test_box', 'session-1', NULL, 'remote session', 'present',
          '/tmp/one.jsonl', 'sessions', '2026-04-01T00:00:00Z', '2026-04-01T00:00:00Z',
          NULL, NULL, NULL, 0, NULL, 'gpt-5.1', 0, '2026-04-01T00:00:00Z', '2026-04-01T00:00:00Z'
        );
        INSERT INTO usage_events (
          session_id, timestamp, model_id, input_tokens, cached_input_tokens, output_tokens,
          reasoning_output_tokens, total_tokens, value_usd, fast_mode_auto, fast_mode_effective
        )
        VALUES ('session-1', '2026-04-01T00:00:00Z', 'gpt-5.1', 10, 0, 10, 0, 20, 0.1, 0, 0);
        INSERT INTO import_state (
          source_path, source_id, session_id, source_bucket, file_size, file_mtime_ms, last_imported_at
        )
        VALUES ('/tmp/one.jsonl', 'ssh_test_box', 'session-1', 'sessions', 1, 1, '2026-04-01T00:00:00Z');
        INSERT INTO rate_limit_samples (
          source_id, source_kind, source_session_id, bucket, sample_timestamp, limit_id,
          limit_name, plan_type, window_start, resets_at, used_percent, remaining_percent, created_at
        )
        VALUES (
          'ssh_test_box', 'session', 'session-1', 'five_hour', '2026-04-01T00:00:00Z',
          'limit', 'Limit', 'pro', '2026-04-01T00:00:00Z', '2026-04-01T05:00:00Z',
          10, 90, '2026-04-01T00:00:00Z'
        );
        ",
        )
        .expect("seed source data");

        assert!(delete_codex_source(&mut conn, "ssh_test_box").expect("delete source"));
        assert!(!delete_codex_source(&mut conn, "local").expect("cannot delete local"));

        let source_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM codex_sources WHERE id = 'ssh_test_box'",
                [],
                |row| row.get(0),
            )
            .expect("source count");
        let session_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE source_id = 'ssh_test_box'",
                [],
                |row| row.get(0),
            )
            .expect("session count");
        let usage_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE session_id = 'session-1'",
                [],
                |row| row.get(0),
            )
            .expect("usage count");
        let import_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM import_state WHERE source_id = 'ssh_test_box'",
                [],
                |row| row.get(0),
            )
            .expect("import count");
        let sample_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rate_limit_samples WHERE source_id = 'ssh_test_box'",
                [],
                |row| row.get(0),
            )
            .expect("sample count");

        assert_eq!(source_count, 0);
        assert_eq!(session_count, 0);
        assert_eq!(usage_count, 0);
        assert_eq!(import_count, 0);
        assert_eq!(sample_count, 0);
    }
}
