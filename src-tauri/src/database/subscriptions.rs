use chrono::NaiveDate;
use rusqlite::{params, Connection};

use crate::models::{SubscriptionProfile, SubscriptionRecord, SubscriptionRecordInput};

use super::now_utc_string;

pub fn canonical_subscription_currency() -> &'static str {
    "USD"
}

pub fn get_subscription_profile(conn: &Connection) -> rusqlite::Result<SubscriptionProfile> {
    conn.query_row(
        "
    SELECT plan_type, currency, monthly_price, billing_anchor_day, updated_at
    FROM subscription_profile
    WHERE singleton_id = 1
    ",
        [],
        |row| {
            Ok(SubscriptionProfile {
                plan_type: row.get(0)?,
                currency: {
                    let _: String = row.get(1)?;
                    canonical_subscription_currency().to_string()
                },
                monthly_price: row.get(2)?,
                billing_anchor_day: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
}

pub fn save_subscription_profile(
    conn: &Connection,
    profile: &SubscriptionProfile,
) -> rusqlite::Result<SubscriptionProfile> {
    let updated_at = now_utc_string();
    conn.execute(
        "
    INSERT INTO subscription_profile (
      singleton_id, plan_type, currency, monthly_price, billing_anchor_day, updated_at
    )
    VALUES (1, ?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(singleton_id) DO UPDATE SET
      plan_type = excluded.plan_type,
      currency = excluded.currency,
      monthly_price = excluded.monthly_price,
      billing_anchor_day = excluded.billing_anchor_day,
      updated_at = excluded.updated_at
    ",
        params![
            profile.plan_type,
            canonical_subscription_currency(),
            profile.monthly_price.max(0.0),
            profile.billing_anchor_day.clamp(1, 28),
            updated_at,
        ],
    )?;
    get_subscription_profile(conn)
}

pub fn list_subscription_records(conn: &Connection) -> rusqlite::Result<Vec<SubscriptionRecord>> {
    let mut stmt = conn.prepare(
    "
    SELECT id, paid_at, service_start, service_end, amount_usd, plan_type, note, created_at, updated_at
    FROM subscription_records
    ORDER BY service_start DESC, paid_at DESC, id DESC
    ",
  )?;

    let records = stmt
        .query_map([], subscription_record_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(records)
}

pub fn create_subscription_record(
    conn: &Connection,
    input: &SubscriptionRecordInput,
) -> Result<SubscriptionRecord, String> {
    let normalized = normalize_subscription_record_input(input)?;
    let now = now_utc_string();
    conn.execute(
        "
      INSERT INTO subscription_records (
        paid_at, service_start, service_end, amount_usd, plan_type, note, created_at, updated_at
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
      ",
        params![
            normalized.paid_at,
            normalized.service_start,
            normalized.service_end,
            normalized.amount_usd,
            normalized.plan_type,
            normalized.note,
            now,
        ],
    )
    .map_err(|error| error.to_string())?;

    get_subscription_record(conn, conn.last_insert_rowid())
}

pub fn update_subscription_record(
    conn: &Connection,
    id: i64,
    input: &SubscriptionRecordInput,
) -> Result<SubscriptionRecord, String> {
    let normalized = normalize_subscription_record_input(input)?;
    let updated_at = now_utc_string();
    let changed = conn
        .execute(
            "
      UPDATE subscription_records
      SET
        paid_at = ?1,
        service_start = ?2,
        service_end = ?3,
        amount_usd = ?4,
        plan_type = ?5,
        note = ?6,
        updated_at = ?7
      WHERE id = ?8
      ",
            params![
                normalized.paid_at,
                normalized.service_start,
                normalized.service_end,
                normalized.amount_usd,
                normalized.plan_type,
                normalized.note,
                updated_at,
                id,
            ],
        )
        .map_err(|error| error.to_string())?;

    if changed == 0 {
        return Err(format!("Subscription record {id} was not found."));
    }

    get_subscription_record(conn, id)
}

pub fn delete_subscription_record(conn: &Connection, id: i64) -> Result<bool, String> {
    let changed = conn
        .execute(
            "DELETE FROM subscription_records WHERE id = ?1",
            params![id],
        )
        .map_err(|error| error.to_string())?;
    Ok(changed > 0)
}

fn get_subscription_record(conn: &Connection, id: i64) -> Result<SubscriptionRecord, String> {
    conn
    .query_row(
      "
      SELECT id, paid_at, service_start, service_end, amount_usd, plan_type, note, created_at, updated_at
      FROM subscription_records
      WHERE id = ?1
      ",
      params![id],
      subscription_record_from_row,
    )
    .map_err(|error| error.to_string())
}

fn subscription_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SubscriptionRecord> {
    Ok(SubscriptionRecord {
        id: row.get(0)?,
        paid_at: row.get(1)?,
        service_start: row.get(2)?,
        service_end: row.get(3)?,
        amount_usd: row.get(4)?,
        plan_type: row.get(5)?,
        note: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

struct NormalizedSubscriptionRecordInput {
    paid_at: String,
    service_start: String,
    service_end: String,
    amount_usd: f64,
    plan_type: String,
    note: Option<String>,
}

fn normalize_subscription_record_input(
    input: &SubscriptionRecordInput,
) -> Result<NormalizedSubscriptionRecordInput, String> {
    let paid_at = normalize_date_field("paidAt", &input.paid_at)?;
    let service_start = normalize_date_field("serviceStart", &input.service_start)?;
    let service_end = normalize_date_field("serviceEnd", &input.service_end)?;
    let start_date = parse_date_field("serviceStart", &service_start)?;
    let end_date = parse_date_field("serviceEnd", &service_end)?;
    if end_date <= start_date {
        return Err("serviceEnd must be later than serviceStart.".to_string());
    }
    if !input.amount_usd.is_finite() {
        return Err("amountUsd must be a finite number.".to_string());
    }

    let plan_type = input.plan_type.trim();
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>());

    Ok(NormalizedSubscriptionRecordInput {
        paid_at,
        service_start,
        service_end,
        amount_usd: input.amount_usd.max(0.0),
        plan_type: if plan_type.is_empty() {
            "unknown".to_string()
        } else {
            plan_type.chars().take(64).collect()
        },
        note,
    })
}

fn normalize_date_field(field_name: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    parse_date_field(field_name, trimmed)?;
    Ok(trimmed.to_string())
}

fn parse_date_field(field_name: &str, value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("{field_name} must use YYYY-MM-DD format."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_db;

    #[test]
    fn subscription_profile_is_normalized_to_usd() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        init_db(&conn).expect("init database");
        save_subscription_profile(
            &conn,
            &SubscriptionProfile {
                plan_type: "pro".to_string(),
                currency: "eur".to_string(),
                monthly_price: 42.0,
                billing_anchor_day: 9,
                updated_at: "2026-04-07T00:00:00Z".to_string(),
            },
        )
        .expect("save profile");

        let profile = get_subscription_profile(&conn).expect("load profile");

        assert_eq!(profile.currency, "USD");
        assert_eq!(profile.monthly_price, 42.0);
        assert_eq!(profile.billing_anchor_day, 9);
    }

    #[test]
    fn subscription_records_round_trip() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        init_db(&conn).expect("init database");
        assert!(list_subscription_records(&conn)
            .expect("list empty")
            .is_empty());

        let created = create_subscription_record(
            &conn,
            &SubscriptionRecordInput {
                paid_at: "2026-04-16".to_string(),
                service_start: "2026-04-16".to_string(),
                service_end: "2026-05-16".to_string(),
                amount_usd: 20.0,
                plan_type: " plus ".to_string(),
                note: Some(" renewal ".to_string()),
            },
        )
        .expect("create record");

        assert_eq!(created.plan_type, "plus");
        assert_eq!(created.note.as_deref(), Some("renewal"));
        assert_eq!(
            list_subscription_records(&conn)
                .expect("list records")
                .len(),
            1
        );

        let updated = update_subscription_record(
            &conn,
            created.id,
            &SubscriptionRecordInput {
                paid_at: "2026-04-16".to_string(),
                service_start: "2026-04-16".to_string(),
                service_end: "2026-05-16".to_string(),
                amount_usd: 200.0,
                plan_type: "pro".to_string(),
                note: None,
            },
        )
        .expect("update record");

        assert_eq!(updated.amount_usd, 200.0);
        assert_eq!(updated.note, None);
        assert!(delete_subscription_record(&conn, created.id).expect("delete record"));
        assert!(list_subscription_records(&conn)
            .expect("list after delete")
            .is_empty());
    }

    #[test]
    fn subscription_record_rejects_invalid_service_period() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        init_db(&conn).expect("init database");

        let result = create_subscription_record(
            &conn,
            &SubscriptionRecordInput {
                paid_at: "2026-04-16".to_string(),
                service_start: "2026-05-16".to_string(),
                service_end: "2026-04-16".to_string(),
                amount_usd: 20.0,
                plan_type: "plus".to_string(),
                note: None,
            },
        );

        assert!(result.is_err());
        assert!(list_subscription_records(&conn)
            .expect("list after reject")
            .is_empty());
    }
}
