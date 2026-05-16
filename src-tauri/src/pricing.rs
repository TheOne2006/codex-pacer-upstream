use std::collections::{HashMap, HashSet};
use std::time::Duration;

use rusqlite::{params, Connection};

use crate::database::{bool_to_i64, i64_to_bool, now_utc_string};
use crate::models::{PricingCatalogEntry, TokenUsage};

pub const OPENAI_API_PRICING_URL: &str = "https://developers.openai.com/api/docs/pricing";
const FALLBACK_PRICING_NOTE: &str =
    "Bundled fallback for OpenAI Standard short-context API pricing.";

#[derive(Debug, Clone)]
pub struct ResolvedPricing {
    pub input_price_per_million: f64,
    pub cached_input_price_per_million: f64,
    pub output_price_per_million: f64,
}

#[derive(Debug, Clone, Copy)]
enum PricingUpsertMode {
    PreserveOfficial,
    Overwrite,
}

#[derive(Debug, Clone)]
struct OfficialPricingRow {
    model_id: String,
    input_price_per_million: f64,
    cached_input_price_per_million: f64,
    output_price_per_million: f64,
}

fn pricing_seed() -> Vec<PricingCatalogEntry> {
    let updated_at = now_utc_string();
    vec![
        fallback_entry("gpt-5.5", "GPT-5.5", 5.00, 0.50, 30.00, &updated_at),
        fallback_entry("gpt-5.4", "GPT-5.4", 2.50, 0.25, 15.00, &updated_at),
        fallback_entry(
            "gpt-5.4-mini",
            "GPT-5.4 Mini",
            0.75,
            0.075,
            4.50,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5.4-nano",
            "GPT-5.4 Nano",
            0.20,
            0.02,
            1.25,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5.3-codex",
            "GPT-5.3 Codex",
            1.75,
            0.175,
            14.00,
            &updated_at,
        ),
        PricingCatalogEntry {
            model_id: "gpt-5.3-codex-spark".to_string(),
            display_name: "GPT-5.3 Codex Spark".to_string(),
            input_price_per_million: 1.75,
            cached_input_price_per_million: 0.175,
            output_price_per_million: 14.00,
            effective_model_id: "gpt-5.3-codex".to_string(),
            is_official: false,
            note: Some(
                "No public Spark API price was found. Using GPT-5.3 Codex fallback pricing."
                    .to_string(),
            ),
            source_url: OPENAI_API_PRICING_URL.to_string(),
            updated_at: updated_at.clone(),
        },
        fallback_entry("gpt-5.2", "GPT-5.2", 1.75, 0.175, 14.00, &updated_at),
        fallback_entry(
            "gpt-5.2-codex",
            "GPT-5.2 Codex",
            1.75,
            0.175,
            14.00,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5-codex",
            "GPT-5 Codex",
            1.25,
            0.125,
            10.00,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5.1-codex-max",
            "GPT-5.1 Codex Max",
            1.25,
            0.125,
            10.00,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5.1-codex",
            "GPT-5.1 Codex",
            1.25,
            0.125,
            10.00,
            &updated_at,
        ),
        fallback_entry(
            "gpt-5.1-codex-mini",
            "GPT-5.1 Codex Mini",
            0.25,
            0.025,
            2.00,
            &updated_at,
        ),
    ]
}

fn fallback_entry(
    model_id: &str,
    display_name: &str,
    input_price_per_million: f64,
    cached_input_price_per_million: f64,
    output_price_per_million: f64,
    updated_at: &str,
) -> PricingCatalogEntry {
    PricingCatalogEntry {
        model_id: model_id.to_string(),
        display_name: display_name.to_string(),
        input_price_per_million,
        cached_input_price_per_million,
        output_price_per_million,
        effective_model_id: model_id.to_string(),
        is_official: false,
        note: Some(FALLBACK_PRICING_NOTE.to_string()),
        source_url: OPENAI_API_PRICING_URL.to_string(),
        updated_at: updated_at.to_string(),
    }
}

fn official_entry(row: OfficialPricingRow, updated_at: &str) -> PricingCatalogEntry {
    PricingCatalogEntry {
        display_name: display_name_for_model(&row.model_id),
        effective_model_id: row.model_id.clone(),
        model_id: row.model_id,
        input_price_per_million: row.input_price_per_million,
        cached_input_price_per_million: row.cached_input_price_per_million,
        output_price_per_million: row.output_price_per_million,
        is_official: true,
        note: Some("OpenAI API Standard short-context pricing.".to_string()),
        source_url: OPENAI_API_PRICING_URL.to_string(),
        updated_at: updated_at.to_string(),
    }
}

pub fn seed_pricing_catalog(conn: &Connection) -> rusqlite::Result<Vec<PricingCatalogEntry>> {
    let entries = pricing_seed();
    upsert_pricing_entries(conn, &entries, PricingUpsertMode::PreserveOfficial)?;
    load_catalog(conn)
}

pub fn refresh_pricing_catalog_from_openai(
    conn: &Connection,
) -> Result<Vec<PricingCatalogEntry>, String> {
    match fetch_official_pricing_catalog() {
        Ok(entries) => {
            upsert_pricing_entries(conn, &entries, PricingUpsertMode::Overwrite)
                .map_err(|error| error.to_string())?;
            seed_pricing_catalog(conn).map_err(|error| error.to_string())
        }
        Err(error) => {
            log::warn!(
                "Failed to refresh OpenAI API pricing from {OPENAI_API_PRICING_URL}: {error}; using bundled fallback pricing."
            );
            seed_pricing_catalog(conn).map_err(|error| error.to_string())
        }
    }
}

fn fetch_official_pricing_catalog() -> Result<Vec<PricingCatalogEntry>, String> {
    let response = ureq::get(OPENAI_API_PRICING_URL)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(format!("OpenAI pricing page returned HTTP {status}."));
    }
    let body = response.into_string().map_err(|error| error.to_string())?;
    parse_official_pricing_catalog(&body)
}

pub fn parse_official_pricing_catalog(document: &str) -> Result<Vec<PricingCatalogEntry>, String> {
    let updated_at = now_utc_string();
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for block in pricing_component_blocks(document) {
        for row in extract_pricing_rows(block) {
            if seen.insert(row.model_id.clone()) {
                entries.push(official_entry(row, &updated_at));
            }
        }
    }

    let required_models = [
        "gpt-5.5",
        "gpt-5.4",
        "gpt-5.4-mini",
        "gpt-5.4-nano",
        "gpt-5.3-codex",
    ];
    let missing = required_models
        .iter()
        .filter(|model_id| !seen.contains(**model_id))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "OpenAI pricing page did not include required Standard short-context rows: {}.",
            missing.join(", ")
        ));
    }

    Ok(entries)
}

fn pricing_component_blocks(document: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = document[cursor..].find("<astro-island") {
        let start = cursor + relative_start;
        let Some(relative_end) = document[start..].find("</astro-island>") else {
            break;
        };
        let end = start + relative_end;
        let block = &document[start..end];
        let is_standard_text_table = block.contains("TextTokenPricingTables")
            && block.contains("&quot;tier&quot;:[0,&quot;standard&quot;]");
        let is_grouped_pricing_table = block.contains("GroupedPricingTable");
        if is_standard_text_table || is_grouped_pricing_table {
            blocks.push(block);
        }
        cursor = end + "</astro-island>".len();
    }
    blocks
}

fn extract_pricing_rows(block: &str) -> Vec<OfficialPricingRow> {
    let marker = "[[0,&quot;";
    let mut rows = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = block[cursor..].find(marker) {
        let name_start = cursor + relative_start + marker.len();
        let Some(relative_name_end) = block[name_start..].find("&quot;]") else {
            break;
        };
        let name_end = name_start + relative_name_end;
        let raw_name = html_unescape(&block[name_start..name_end]);
        let mut value_cursor = name_end + "&quot;]".len();

        let input = parse_next_pricing_value(block, &mut value_cursor).flatten();
        let cached_input = parse_next_pricing_value(block, &mut value_cursor).flatten();
        let output = parse_next_pricing_value(block, &mut value_cursor).flatten();

        if let (Some(input), Some(output)) = (input, output) {
            let model_id = normalize_official_model_id(&raw_name);
            if should_include_official_pricing_model(&model_id) {
                rows.push(OfficialPricingRow {
                    model_id,
                    input_price_per_million: input,
                    cached_input_price_per_million: cached_input.unwrap_or(input),
                    output_price_per_million: output,
                });
            }
        }

        cursor = name_end;
    }

    rows
}

fn parse_next_pricing_value(source: &str, cursor: &mut usize) -> Option<Option<f64>> {
    let marker = ",[0,";
    let value_start = *cursor + source[*cursor..].find(marker)? + marker.len();
    if source[value_start..].starts_with("&quot;") {
        let inner_start = value_start + "&quot;".len();
        let inner_end = inner_start + source[inner_start..].find("&quot;")?;
        *cursor = inner_end + "&quot;".len();
        Some(parse_price_literal(&html_unescape(
            &source[inner_start..inner_end],
        )))
    } else {
        let value_end = value_start + source[value_start..].find(']')?;
        *cursor = value_end;
        Some(parse_price_literal(&source[value_start..value_end]))
    }
}

fn parse_price_literal(value: &str) -> Option<f64> {
    let cleaned = value.trim().trim_start_matches('$').replace(',', "");
    if cleaned.is_empty()
        || cleaned == "-"
        || cleaned.eq_ignore_ascii_case("null")
        || cleaned.starts_with('{')
    {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

fn normalize_official_model_id(raw_name: &str) -> String {
    let base = raw_name
        .split(" (")
        .next()
        .unwrap_or(raw_name)
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or(raw_name)
        .trim();
    normalize_model_id(base)
}

fn should_include_official_pricing_model(model_id: &str) -> bool {
    if model_id.is_empty() {
        return false;
    }
    let excluded_fragments = [
        "image",
        "realtime",
        "transcribe",
        "tts",
        "sora",
        "embedding",
        "moderation",
        "computer-use",
        "deep-research",
    ];
    if excluded_fragments
        .iter()
        .any(|fragment| model_id.contains(fragment))
    {
        return false;
    }
    model_id.starts_with("gpt-")
        || model_id.starts_with('o')
        || model_id.starts_with("chatgpt-")
        || model_id.starts_with("codex-")
        || model_id.contains("codex")
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn upsert_pricing_entries(
    conn: &Connection,
    entries: &[PricingCatalogEntry],
    mode: PricingUpsertMode,
) -> rusqlite::Result<()> {
    let sql = match mode {
        PricingUpsertMode::Overwrite => {
            "
      INSERT INTO pricing_catalog (
        model_id, display_name, input_price_per_million, cached_input_price_per_million,
        output_price_per_million, effective_model_id, is_official, note, source_url, updated_at
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
      ON CONFLICT(model_id) DO UPDATE SET
        display_name = excluded.display_name,
        input_price_per_million = excluded.input_price_per_million,
        cached_input_price_per_million = excluded.cached_input_price_per_million,
        output_price_per_million = excluded.output_price_per_million,
        effective_model_id = excluded.effective_model_id,
        is_official = excluded.is_official,
        note = excluded.note,
        source_url = excluded.source_url,
        updated_at = excluded.updated_at
      "
        }
        PricingUpsertMode::PreserveOfficial => {
            "
      INSERT INTO pricing_catalog (
        model_id, display_name, input_price_per_million, cached_input_price_per_million,
        output_price_per_million, effective_model_id, is_official, note, source_url, updated_at
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
      ON CONFLICT(model_id) DO UPDATE SET
        display_name = excluded.display_name,
        input_price_per_million = excluded.input_price_per_million,
        cached_input_price_per_million = excluded.cached_input_price_per_million,
        output_price_per_million = excluded.output_price_per_million,
        effective_model_id = excluded.effective_model_id,
        is_official = excluded.is_official,
        note = excluded.note,
        source_url = excluded.source_url,
        updated_at = excluded.updated_at
      WHERE pricing_catalog.is_official = 0
      "
        }
    };

    for entry in entries {
        conn.execute(
            sql,
            params![
                entry.model_id,
                entry.display_name,
                entry.input_price_per_million,
                entry.cached_input_price_per_million,
                entry.output_price_per_million,
                entry.effective_model_id,
                bool_to_i64(entry.is_official),
                entry.note,
                entry.source_url,
                entry.updated_at,
            ],
        )?;
    }

    Ok(())
}

pub fn load_catalog(conn: &Connection) -> rusqlite::Result<Vec<PricingCatalogEntry>> {
    let mut stmt = conn.prepare(
        "
    SELECT model_id, display_name, input_price_per_million, cached_input_price_per_million,
           output_price_per_million, effective_model_id, is_official, note, source_url, updated_at
    FROM pricing_catalog
    ORDER BY model_id
    ",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PricingCatalogEntry {
            model_id: row.get(0)?,
            display_name: row.get(1)?,
            input_price_per_million: row.get(2)?,
            cached_input_price_per_million: row.get(3)?,
            output_price_per_million: row.get(4)?,
            effective_model_id: row.get(5)?,
            is_official: i64_to_bool(row.get::<_, i64>(6)?),
            note: row.get(7)?,
            source_url: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;

    rows.collect()
}

pub fn load_catalog_map(
    conn: &Connection,
) -> rusqlite::Result<HashMap<String, PricingCatalogEntry>> {
    Ok(load_catalog(conn)?
        .into_iter()
        .map(|entry| (entry.model_id.clone(), entry))
        .collect())
}

pub fn resolve_pricing(
    catalog: &HashMap<String, PricingCatalogEntry>,
    model_id: &str,
) -> Option<ResolvedPricing> {
    let normalized = normalize_model_id(model_id);
    let entry = if let Some(entry) = catalog.get(&normalized) {
        entry.clone()
    } else if normalized.starts_with("gpt-5.5-pro") {
        catalog.get("gpt-5.5-pro")?.clone()
    } else if normalized.starts_with("gpt-5.5") {
        catalog.get("gpt-5.5")?.clone()
    } else if normalized.starts_with("gpt-5.4-mini") {
        catalog.get("gpt-5.4-mini")?.clone()
    } else if normalized.starts_with("gpt-5.4-nano") {
        catalog.get("gpt-5.4-nano")?.clone()
    } else if normalized.starts_with("gpt-5.4-pro") {
        catalog.get("gpt-5.4-pro")?.clone()
    } else if normalized.starts_with("gpt-5.4") {
        catalog.get("gpt-5.4")?.clone()
    } else if normalized.starts_with("gpt-5.3-codex-spark") {
        catalog.get("gpt-5.3-codex-spark")?.clone()
    } else if normalized.starts_with("gpt-5.3-codex") {
        catalog.get("gpt-5.3-codex")?.clone()
    } else if normalized.starts_with("gpt-5.2-codex") {
        catalog.get("gpt-5.2-codex")?.clone()
    } else if normalized.starts_with("gpt-5.2") {
        catalog.get("gpt-5.2")?.clone()
    } else if normalized.starts_with("gpt-5-codex") {
        catalog.get("gpt-5-codex")?.clone()
    } else if normalized.starts_with("gpt-5.1-codex-max") {
        catalog.get("gpt-5.1-codex-max")?.clone()
    } else if normalized.starts_with("gpt-5.1-codex-mini") {
        catalog.get("gpt-5.1-codex-mini")?.clone()
    } else if normalized.starts_with("gpt-5.1-codex") {
        catalog.get("gpt-5.1-codex")?.clone()
    } else {
        return None;
    };

    Some(ResolvedPricing {
        input_price_per_million: entry.input_price_per_million,
        cached_input_price_per_million: entry.cached_input_price_per_million,
        output_price_per_million: entry.output_price_per_million,
    })
}

pub fn normalize_model_id(model_id: &str) -> String {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

pub fn display_name_for_model(model_id: &str) -> String {
    match normalize_model_id(model_id).as_str() {
        "codex-auto-review" => "Codex Auto Review".to_string(),
        "codex-mini-latest" => "Codex Mini Latest".to_string(),
        "gpt-5.5" => "GPT-5.5".to_string(),
        "gpt-5.5-pro" => "GPT-5.5 Pro".to_string(),
        "gpt-5.4" => "GPT-5.4".to_string(),
        "gpt-5.4-mini" => "GPT-5.4 Mini".to_string(),
        "gpt-5.4-nano" => "GPT-5.4 Nano".to_string(),
        "gpt-5.4-pro" => "GPT-5.4 Pro".to_string(),
        "gpt-5.3-codex" => "GPT-5.3 Codex".to_string(),
        "gpt-5.3-codex-spark" => "GPT-5.3 Codex Spark".to_string(),
        "gpt-5.3-chat-latest" => "GPT-5.3 Chat Latest".to_string(),
        "gpt-5.2" => "GPT-5.2".to_string(),
        "gpt-5.2-codex" => "GPT-5.2 Codex".to_string(),
        "gpt-5.2-chat-latest" => "GPT-5.2 Chat Latest".to_string(),
        "gpt-5.1" => "GPT-5.1".to_string(),
        "gpt-5.1-codex" => "GPT-5.1 Codex".to_string(),
        "gpt-5.1-codex-max" => "GPT-5.1 Codex Max".to_string(),
        "gpt-5.1-codex-mini" => "GPT-5.1 Codex Mini".to_string(),
        "gpt-5.1-chat-latest" => "GPT-5.1 Chat Latest".to_string(),
        "gpt-5" => "GPT-5".to_string(),
        "gpt-5-codex" => "GPT-5 Codex".to_string(),
        "gpt-5-chat-latest" => "GPT-5 Chat Latest".to_string(),
        "unknown" => "Unknown".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

pub fn model_color(model_id: &str) -> &'static str {
    match normalize_model_id(model_id).as_str() {
        "codex-auto-review" => "#60A5FA",
        "gpt-5.5" => "#E879F9",
        "gpt-5.5-pro" => "#C084FC",
        "gpt-5.4" => "#F97316",
        "gpt-5.4-mini" => "#FB923C",
        "gpt-5.4-nano" => "#FDBA74",
        "gpt-5.4-pro" => "#EA580C",
        "gpt-5.3-codex" => "#F59E0B",
        "gpt-5.3-codex-spark" => "#FACC15",
        "gpt-5.2" => "#14B8A6",
        "gpt-5.2-codex" => "#34D399",
        "gpt-5-codex" => "#3B82F6",
        "gpt-5.1-codex-max" => "#6366F1",
        "gpt-5.1-codex" => "#64748B",
        "gpt-5.1-codex-mini" => "#38BDF8",
        _ => "#94A3B8",
    }
}

pub fn calculate_value_usd(usage: &TokenUsage, resolved_pricing: Option<&ResolvedPricing>) -> f64 {
    let Some(pricing) = resolved_pricing else {
        return 0.0;
    };

    let input_tokens = usage.input_tokens as f64;
    let cached_input_tokens = usage.cached_input_tokens as f64;
    let output_tokens = usage.output_tokens as f64;
    let uncached_input_tokens = (input_tokens - cached_input_tokens).max(0.0);

    (uncached_input_tokens / 1_000_000.0) * pricing.input_price_per_million
        + (cached_input_tokens / 1_000_000.0) * pricing.cached_input_price_per_million
        + (output_tokens / 1_000_000.0) * pricing.output_price_per_million
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_input_is_not_billed_twice() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 400_000,
            output_tokens: 100_000,
            reasoning_output_tokens: 0,
            total_tokens: 1_100_000,
        };
        let pricing = ResolvedPricing {
            input_price_per_million: 2.0,
            cached_input_price_per_million: 0.5,
            output_price_per_million: 10.0,
        };

        let value = calculate_value_usd(&usage, Some(&pricing));
        let expected = (600_000.0 / 1_000_000.0) * 2.0
            + (400_000.0 / 1_000_000.0) * 0.5
            + (100_000.0 / 1_000_000.0) * 10.0;

        assert!((value - expected).abs() < 1e-9);
    }

    #[test]
    fn large_inputs_still_use_the_same_short_context_rate() {
        let usage = TokenUsage {
            input_tokens: 300_000,
            cached_input_tokens: 50_000,
            output_tokens: 10_000,
            reasoning_output_tokens: 0,
            total_tokens: 310_000,
        };
        let pricing = ResolvedPricing {
            input_price_per_million: 5.0,
            cached_input_price_per_million: 0.5,
            output_price_per_million: 30.0,
        };

        let value = calculate_value_usd(&usage, Some(&pricing));
        let expected = (250_000.0 / 1_000_000.0) * 5.0
            + (50_000.0 / 1_000_000.0) * 0.5
            + (10_000.0 / 1_000_000.0) * 30.0;

        assert!((value - expected).abs() < 1e-9);
    }

    #[test]
    fn resolve_pricing_distinguishes_gpt_54_variants() {
        let entries = pricing_seed();
        let catalog = entries
            .into_iter()
            .map(|entry| (entry.model_id.clone(), entry))
            .collect::<HashMap<_, _>>();

        let flagship = resolve_pricing(&catalog, "gpt-5.4").expect("gpt-5.4 pricing");
        let mini = resolve_pricing(&catalog, "gpt-5.4-mini").expect("gpt-5.4-mini pricing");
        let nano = resolve_pricing(&catalog, "gpt-5.4-nano").expect("gpt-5.4-nano pricing");

        assert_eq!(flagship.input_price_per_million, 2.50);
        assert_eq!(mini.input_price_per_million, 0.75);
        assert_eq!(nano.input_price_per_million, 0.20);
        assert!(flagship.input_price_per_million > mini.input_price_per_million);
        assert!(mini.input_price_per_million > nano.input_price_per_million);
    }

    #[test]
    fn resolve_pricing_includes_gpt_55() {
        let entries = pricing_seed();
        let catalog = entries
            .into_iter()
            .map(|entry| (entry.model_id.clone(), entry))
            .collect::<HashMap<_, _>>();

        let pricing = resolve_pricing(&catalog, "gpt-5.5").expect("gpt-5.5 pricing");

        assert_eq!(pricing.input_price_per_million, 5.00);
        assert_eq!(pricing.cached_input_price_per_million, 0.50);
        assert_eq!(pricing.output_price_per_million, 30.00);
    }

    #[test]
    fn parses_official_standard_short_context_pricing_rows() {
        let html = concat!(
            "<astro-island component-export=\"TextTokenPricingTables\" props=\"{&quot;tier&quot;:[0,&quot;standard&quot;],&quot;rows&quot;:[1,[[1,[[0,&quot;gpt-5.5 (&lt;272K context length)&quot;],[0,5],[0,0.5],[0,30]]],[1,[[0,&quot;gpt-5.5 (&gt;=272K context length)&quot;],[0,10],[0,1],[0,45]]],[1,[[0,&quot;gpt-5.4 (&lt;272K context length)&quot;],[0,2.5],[0,0.25],[0,15]]],[1,[[0,&quot;gpt-5.4-mini&quot;],[0,0.75],[0,0.075],[0,4.5]]],[1,[[0,&quot;gpt-5.4-nano&quot;],[0,0.2],[0,0.02],[0,1.25]]]]]}\"></astro-island>",
            "<astro-island component-export=\"GroupedPricingTable\" props=\"{&quot;groups&quot;:[1,[[0,{&quot;model&quot;:[0,&quot;Codex&quot;],&quot;rows&quot;:[1,[[1,[[0,&quot;gpt-5.3-codex&quot;],[0,1.75],[0,0.175],[0,14]]]]]}]]]}\"></astro-island>"
        );

        let entries = parse_official_pricing_catalog(html).expect("parse official pricing");
        let catalog = entries
            .into_iter()
            .map(|entry| (entry.model_id.clone(), entry))
            .collect::<HashMap<_, _>>();

        let gpt55 = catalog.get("gpt-5.5").expect("gpt-5.5");
        assert_eq!(gpt55.input_price_per_million, 5.0);
        assert_eq!(gpt55.cached_input_price_per_million, 0.5);
        assert_eq!(gpt55.output_price_per_million, 30.0);
        assert!(gpt55.is_official);

        let codex = catalog.get("gpt-5.3-codex").expect("gpt-5.3-codex");
        assert_eq!(codex.input_price_per_million, 1.75);
        assert_eq!(codex.cached_input_price_per_million, 0.175);
        assert_eq!(codex.output_price_per_million, 14.0);
    }
}
