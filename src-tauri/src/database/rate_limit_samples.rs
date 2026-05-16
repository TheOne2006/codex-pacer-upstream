use rusqlite::{params, Connection};

use crate::models::{
    LiveRateLimitSnapshot, RateLimitCreditsSnapshot, RateLimitMetadataSampleRecord,
    RateLimitSampleRecord,
};

use super::{i64_to_bool, now_utc_string};

#[derive(Debug, Clone)]
pub struct PersistedRateLimitMetadata {
    pub sample_timestamp: String,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub credits: Option<RateLimitCreditsSnapshot>,
    pub rate_limit_reached_type: Option<String>,
}

pub fn replace_session_rate_limit_samples(
    conn: &Connection,
    session_id: &str,
    samples: &[RateLimitSampleRecord],
) -> rusqlite::Result<()> {
    conn.execute(
        "
    DELETE FROM rate_limit_samples
    WHERE source_kind = 'session' AND source_session_id = ?1
    ",
        params![session_id],
    )?;
    insert_rate_limit_samples(conn, samples)
}

pub fn replace_session_rate_limit_metadata_samples(
    conn: &Connection,
    session_id: &str,
    samples: &[RateLimitMetadataSampleRecord],
) -> rusqlite::Result<()> {
    conn.execute(
        "
    DELETE FROM rate_limit_metadata_samples
    WHERE source_kind = 'session' AND source_session_id = ?1
    ",
        params![session_id],
    )?;
    insert_rate_limit_metadata_samples(conn, samples)
}

pub fn insert_rate_limit_samples(
    conn: &Connection,
    samples: &[RateLimitSampleRecord],
) -> rusqlite::Result<()> {
    let created_at = now_utc_string();
    let mut stmt = conn.prepare(
        "
    INSERT OR IGNORE INTO rate_limit_samples (
      source_kind, source_session_id, bucket, sample_timestamp, limit_id, limit_name, plan_type,
      window_start, resets_at, used_percent, remaining_percent, created_at
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
    ",
    )?;

    for sample in samples {
        stmt.execute(params![
            sample.source_kind,
            sample.source_session_id.clone().unwrap_or_default(),
            sample.bucket,
            sample.sample_timestamp,
            sample.limit_id.clone().unwrap_or_default(),
            sample.limit_name.clone().unwrap_or_default(),
            sample.plan_type.clone().unwrap_or_default(),
            sample.window_start,
            sample.resets_at,
            sample.used_percent.clamp(0, 100),
            sample.remaining_percent.clamp(0, 100),
            created_at,
        ])?;
    }

    Ok(())
}

pub fn insert_rate_limit_metadata_samples(
    conn: &Connection,
    samples: &[RateLimitMetadataSampleRecord],
) -> rusqlite::Result<()> {
    let created_at = now_utc_string();
    let mut stmt = conn.prepare(
        "
    INSERT OR IGNORE INTO rate_limit_metadata_samples (
      source_kind, source_session_id, sample_timestamp, limit_id, limit_name, plan_type,
      credits_has_credits, credits_unlimited, credits_balance, rate_limit_reached_type,
      raw_rate_limits_json, created_at
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
    ",
    )?;

    for sample in samples {
        stmt.execute(params![
            sample.source_kind,
            sample.source_session_id.clone().unwrap_or_default(),
            sample.sample_timestamp,
            sample.limit_id.clone().unwrap_or_default(),
            sample.limit_name.clone().unwrap_or_default(),
            sample.plan_type.clone().unwrap_or_default(),
            sample
                .credits
                .as_ref()
                .and_then(|credits| credits.has_credits)
                .map(|value| value as i64),
            sample
                .credits
                .as_ref()
                .and_then(|credits| credits.unlimited)
                .map(|value| value as i64),
            sample
                .credits
                .as_ref()
                .and_then(|credits| credits.balance.clone()),
            sample.rate_limit_reached_type.clone().unwrap_or_default(),
            sample.raw_rate_limits_json.clone(),
            created_at,
        ])?;
    }

    Ok(())
}

pub fn insert_live_rate_limit_snapshot(
    conn: &Connection,
    snapshot: &LiveRateLimitSnapshot,
) -> rusqlite::Result<()> {
    let mut samples = Vec::new();
    for (bucket, window) in [
        ("five_hour", snapshot.primary.as_ref()),
        ("seven_day", snapshot.secondary.as_ref()),
    ] {
        let Some(window) = window else {
            continue;
        };
        let (Some(window_start), Some(resets_at)) =
            (window.window_start.clone(), window.resets_at.clone())
        else {
            continue;
        };
        samples.push(RateLimitSampleRecord {
            source_kind: "live".to_string(),
            source_session_id: None,
            bucket: bucket.to_string(),
            sample_timestamp: snapshot.fetched_at.clone(),
            limit_id: snapshot.limit_id.clone(),
            limit_name: snapshot.limit_name.clone(),
            plan_type: snapshot.plan_type.clone(),
            window_start,
            resets_at,
            used_percent: window.used_percent,
            remaining_percent: window.remaining_percent,
        });
    }
    insert_rate_limit_samples(conn, &samples)?;

    if let Some(metadata) = rate_limit_metadata_from_live_snapshot(snapshot) {
        insert_rate_limit_metadata_samples(conn, &[metadata])?;
    }

    Ok(())
}

pub fn load_latest_rate_limit_metadata(
    conn: &Connection,
    source_kind: Option<&str>,
) -> rusqlite::Result<Option<PersistedRateLimitMetadata>> {
    let mut stmt = conn.prepare(
        "
    SELECT
      sample_timestamp, limit_id, limit_name, plan_type,
      credits_has_credits, credits_unlimited, credits_balance, rate_limit_reached_type
    FROM rate_limit_metadata_samples
    WHERE (?1 IS NULL OR source_kind = ?1)
      AND (limit_id = '' OR limit_id = 'codex')
      AND (limit_name = '' OR limit_name NOT LIKE 'GPT-%')
    ORDER BY sample_timestamp DESC, id DESC
    LIMIT 1
    ",
    )?;

    let mut rows = stmt.query(params![source_kind])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let credits_has_credits = row.get::<_, Option<i64>>(4)?.map(i64_to_bool);
    let credits_unlimited = row.get::<_, Option<i64>>(5)?.map(i64_to_bool);
    let credits_balance = row.get::<_, Option<String>>(6)?;
    let credits = if credits_has_credits.is_some()
        || credits_unlimited.is_some()
        || credits_balance.is_some()
    {
        Some(RateLimitCreditsSnapshot {
            has_credits: credits_has_credits,
            unlimited: credits_unlimited,
            balance: credits_balance,
        })
    } else {
        None
    };

    Ok(Some(PersistedRateLimitMetadata {
        sample_timestamp: row.get(0)?,
        limit_id: row.get::<_, String>(1).ok().and_then(non_empty),
        limit_name: row.get::<_, String>(2).ok().and_then(non_empty),
        plan_type: row.get::<_, String>(3).ok().and_then(non_empty),
        credits,
        rate_limit_reached_type: row.get::<_, String>(7).ok().and_then(non_empty),
    }))
}

fn rate_limit_metadata_from_live_snapshot(
    snapshot: &LiveRateLimitSnapshot,
) -> Option<RateLimitMetadataSampleRecord> {
    let has_metadata = snapshot.limit_id.is_some()
        || snapshot.limit_name.is_some()
        || snapshot.plan_type.is_some()
        || snapshot.credits.is_some()
        || snapshot.rate_limit_reached_type.is_some();
    if !has_metadata {
        return None;
    }

    Some(RateLimitMetadataSampleRecord {
        source_kind: "live".to_string(),
        source_session_id: None,
        sample_timestamp: snapshot.fetched_at.clone(),
        limit_id: snapshot.limit_id.clone(),
        limit_name: snapshot.limit_name.clone(),
        plan_type: snapshot.plan_type.clone(),
        credits: snapshot.credits.clone(),
        rate_limit_reached_type: snapshot.rate_limit_reached_type.clone(),
        raw_rate_limits_json: None,
    })
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_db;

    #[test]
    fn rate_limit_metadata_roundtrips_latest_credits() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        init_db(&conn).expect("init db");

        insert_rate_limit_metadata_samples(
            &conn,
            &[RateLimitMetadataSampleRecord {
                source_kind: "session".to_string(),
                source_session_id: Some("session-a".to_string()),
                sample_timestamp: "2026-05-15T10:00:00+08:00".to_string(),
                limit_id: Some("codex".to_string()),
                limit_name: None,
                plan_type: Some("pro".to_string()),
                credits: Some(RateLimitCreditsSnapshot {
                    has_credits: Some(true),
                    unlimited: Some(false),
                    balance: Some("promo-balance-42".to_string()),
                }),
                rate_limit_reached_type: Some("secondary".to_string()),
                raw_rate_limits_json: Some(
                    "{\"credits\":{\"balance\":\"promo-balance-42\"}}".to_string(),
                ),
            }],
        )
        .expect("insert metadata");

        let source_id = conn
            .query_row(
                "SELECT source_id FROM rate_limit_metadata_samples LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("source id");
        assert_eq!(source_id, "local");

        let metadata = load_latest_rate_limit_metadata(&conn, Some("session"))
            .expect("load metadata")
            .expect("metadata");

        assert_eq!(metadata.limit_id.as_deref(), Some("codex"));
        assert_eq!(metadata.plan_type.as_deref(), Some("pro"));
        assert_eq!(
            metadata
                .credits
                .as_ref()
                .and_then(|credits| credits.balance.as_deref()),
            Some("promo-balance-42")
        );
        assert_eq!(
            metadata.rate_limit_reached_type.as_deref(),
            Some("secondary")
        );
    }
}
