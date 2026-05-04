use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tauri::{AppHandle, Emitter};

use crate::database::{
    get_codex_source, now_utc_string, open_connection, update_codex_source_download_state,
};
use crate::importer::perform_scan_for_source;
use crate::models::{CodexSourceCandidate, CodexSourceDownloadProgress, CodexSourceDownloadResult};

const DOWNLOAD_PROGRESS_EVENT: &str = "codex-pacer://source-download-progress";

#[derive(Debug, Default, Clone)]
struct HostBlock {
    aliases: Vec<String>,
    host_name: Option<String>,
    user: Option<String>,
    port: Option<i64>,
}

pub fn discover_ssh_codex_sources() -> Vec<CodexSourceCandidate> {
    let Some(home_dir) = dirs::home_dir() else {
        return Vec::new();
    };
    let config_path = home_dir.join(".ssh").join("config");
    let mut visited = HashSet::new();
    let mut candidates = Vec::new();
    parse_ssh_config_file(&config_path, &mut visited, &mut candidates);
    dedupe_candidates(candidates)
}

pub fn source_cache_codex_home(app_data_dir: &Path, source_id: &str) -> PathBuf {
    app_data_dir
        .join("codex-sources")
        .join(sanitize_component(source_id))
        .join("codex-cache")
}

pub fn download_codex_source(
    app: &AppHandle,
    db_path: &Path,
    app_data_dir: &Path,
    source_id: &str,
) -> Result<CodexSourceDownloadResult, String> {
    let conn = open_connection(db_path).map_err(|error| error.to_string())?;
    let source = get_codex_source(&conn, source_id).map_err(|error| error.to_string())?;
    if source.kind != "ssh" {
        return Err("Only SSH sources can be downloaded.".to_string());
    }
    let ssh_alias = source
        .ssh_alias
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "SSH source is missing an alias.".to_string())?;
    let remote_home = source
        .remote_codex_home
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("~/.codex");

    let _ = update_codex_source_download_state(&conn, source_id, "downloading", None, None, None);
    emit_progress(app, source_id, "connecting", None, "连接远程服务器");

    let cache_dir = source_cache_codex_home(app_data_dir, source_id);
    let parent = cache_dir
        .parent()
        .ok_or_else(|| "Failed to resolve source cache parent.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp_dir = parent.join(format!(
        ".download-{}",
        now_utc_string().replace([':', '.'], "-")
    ));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;

    emit_progress(
        app,
        source_id,
        "downloading",
        None,
        "下载远程 Codex usage 缓存",
    );
    let remote_command = remote_tar_command(remote_home);
    let mut ssh = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(ssh_alias)
        .arg(remote_command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start ssh: {error}"))?;

    let stdout = ssh
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture ssh stdout.".to_string())?;
    let tar = Command::new("tar")
        .arg("-xf")
        .arg("-")
        .arg("-C")
        .arg(&temp_dir)
        .stdin(Stdio::from(stdout))
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start local tar: {error}"))?;

    let tar_output = tar
        .wait_with_output()
        .map_err(|error| format!("Failed while extracting remote cache: {error}"))?;
    let ssh_output = ssh
        .wait_with_output()
        .map_err(|error| format!("Failed while waiting for ssh: {error}"))?;

    if !ssh_output.status.success() || !tar_output.status.success() {
        let raw_message = [
            String::from_utf8_lossy(&ssh_output.stderr)
                .trim()
                .to_string(),
            String::from_utf8_lossy(&tar_output.stderr)
                .trim()
                .to_string(),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        let error = remote_download_error_message(
            &raw_message,
            ssh_output.status.code(),
            tar_output.status.code(),
            remote_home,
        );
        let _ = update_codex_source_download_state(
            &conn,
            source_id,
            "failed",
            None,
            None,
            Some(&error),
        );
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(error);
    }

    emit_progress(app, source_id, "installing", Some(0.8), "写入本地缓存");
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).map_err(|error| error.to_string())?;
    }
    fs::rename(&temp_dir, &cache_dir).map_err(|error| error.to_string())?;

    emit_progress(app, source_id, "scanning", Some(0.9), "导入缓存");
    let scan_result = perform_scan_for_source(
        db_path,
        source_id,
        Some(cache_dir.to_string_lossy().to_string()),
    )?;
    let now = now_utc_string();
    let conn = open_connection(db_path).map_err(|error| error.to_string())?;
    let source = update_codex_source_download_state(
        &conn,
        source_id,
        "ready",
        Some(&now),
        Some(&scan_result.last_completed_at),
        None,
    )
    .map_err(|error| error.to_string())?;
    emit_progress(app, source_id, "done", Some(1.0), "完成");

    Ok(CodexSourceDownloadResult {
        source,
        scan_result,
    })
}

fn emit_progress(
    app: &AppHandle,
    source_id: &str,
    stage: &str,
    progress: Option<f64>,
    message: &str,
) {
    let _ = app.emit(
        DOWNLOAD_PROGRESS_EVENT,
        CodexSourceDownloadProgress {
            source_id: source_id.to_string(),
            stage: stage.to_string(),
            progress,
            message: message.to_string(),
        },
    );
}

fn remote_tar_command(remote_home: &str) -> String {
    let home = remote_path_expr(remote_home);
    format!(
    "cd {home} || exit 2; set --; [ -f session_index.jsonl ] && set -- \"$@\" session_index.jsonl; [ -d sessions ] && set -- \"$@\" sessions; [ -d archived_sessions ] && set -- \"$@\" archived_sessions; [ \"$#\" -gt 0 ] || exit 3; tar -cf - \"$@\""
  )
}

fn remote_path_expr(remote_home: &str) -> String {
    let trimmed = remote_home.trim();
    if trimmed == "~" {
        return "\"$HOME\"".to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if rest.is_empty() {
            "\"$HOME\"".to_string()
        } else {
            format!("\"$HOME\"/{}", shell_quote(rest))
        }
    } else {
        shell_quote(trimmed)
    }
}

fn remote_download_error_message(
    raw_message: &str,
    ssh_status: Option<i32>,
    tar_status: Option<i32>,
    remote_home: &str,
) -> String {
    if ssh_status == Some(2) || raw_message.contains("cd:") {
        return format!(
            "远程 Codex 目录不存在：{}。请确认这台服务器已运行过 Codex，或重新添加服务器时改成实际目录。",
            remote_home
        );
    }
    if ssh_status == Some(3) {
        return format!(
            "远程目录 {} 存在，但没有找到 Codex 会话缓存（session_index.jsonl / sessions / archived_sessions）。",
            remote_home
        );
    }
    if raw_message.is_empty() {
        if let Some(code) = tar_status {
            return format!("下载远程 Codex 缓存失败，本地解压退出码 {code}。");
        }
        "下载远程 Codex 缓存失败。".to_string()
    } else {
        raw_message.to_string()
    }
}

fn parse_ssh_config_file(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    candidates: &mut Vec<CodexSourceCandidate>,
) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return;
    }
    let Ok(mut file) = fs::File::open(&canonical) else {
        return;
    };
    let mut content = String::new();
    if file.read_to_string(&mut content).is_err() {
        return;
    }

    let base_dir = canonical.parent().unwrap_or_else(|| Path::new("/"));
    let mut current = HostBlock::default();
    for line in content.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(keyword) = parts.next() else {
            continue;
        };
        let rest = parts.collect::<Vec<_>>();
        match keyword.to_ascii_lowercase().as_str() {
            "include" => {
                flush_host_block(&current, candidates);
                current = HostBlock::default();
                for pattern in rest {
                    for include_path in expand_include_pattern(base_dir, pattern) {
                        parse_ssh_config_file(&include_path, visited, candidates);
                    }
                }
            }
            "host" => {
                flush_host_block(&current, candidates);
                current = HostBlock {
                    aliases: rest.into_iter().map(ToString::to_string).collect(),
                    ..HostBlock::default()
                };
            }
            "hostname" => current.host_name = rest.first().map(|value| value.to_string()),
            "user" => current.user = rest.first().map(|value| value.to_string()),
            "port" => current.port = rest.first().and_then(|value| value.parse::<i64>().ok()),
            _ => {}
        }
    }
    flush_host_block(&current, candidates);
}

fn flush_host_block(block: &HostBlock, candidates: &mut Vec<CodexSourceCandidate>) {
    for alias in &block.aliases {
        let ignored_reason =
            ignored_host_reason(alias, block.host_name.as_deref(), block.user.as_deref());
        if ignored_reason.is_some() {
            continue;
        }
        candidates.push(CodexSourceCandidate {
            id: source_id_for_alias(alias),
            label: alias.to_string(),
            ssh_alias: alias.to_string(),
            host_name: block.host_name.clone(),
            user: block.user.clone(),
            port: block.port,
            remote_codex_home: "~/.codex".to_string(),
            ignored_reason: None,
        });
    }
}

fn ignored_host_reason(alias: &str, host_name: Option<&str>, user: Option<&str>) -> Option<String> {
    if alias.contains('*') || alias.contains('?') || alias.starts_with('!') {
        return Some("pattern".to_string());
    }
    let lower_alias = alias.to_ascii_lowercase();
    let lower_host = host_name.unwrap_or(alias).to_ascii_lowercase();
    let lower_user = user.unwrap_or_default().to_ascii_lowercase();
    if lower_user == "git" {
        return Some("code-host".to_string());
    }
    let ignored = [
        "github.com",
        "gitlab.com",
        "bitbucket.org",
        "ssh.dev.azure.com",
        "gist.github.com",
        "gitee.com",
        "codeberg.org",
        "sr.ht",
        "sourcehut",
    ];
    if ignored
        .iter()
        .any(|needle| lower_alias.contains(needle) || lower_host.contains(needle))
    {
        return Some("code-host".to_string());
    }
    if looks_like_git_host(&lower_alias) || looks_like_git_host(&lower_host) {
        return Some("code-host".to_string());
    }
    None
}

fn looks_like_git_host(value: &str) -> bool {
    let normalized = value.trim_matches('.');
    normalized == "git"
        || normalized.starts_with("git.")
        || normalized.ends_with(".git")
        || normalized.contains(".git.")
        || normalized.starts_with("git-")
        || normalized.contains("gitlab")
        || normalized.contains("github")
}

fn dedupe_candidates(candidates: Vec<CodexSourceCandidate>) -> Vec<CodexSourceCandidate> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.id.clone()) {
            result.push(candidate);
        }
    }
    result.sort_by(|left, right| {
        left.label
            .to_ascii_lowercase()
            .cmp(&right.label.to_ascii_lowercase())
    });
    result
}

fn expand_include_pattern(base_dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let expanded = expand_tilde(pattern);
    let path = PathBuf::from(expanded);
    let path = if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    };
    let pattern_string = path.to_string_lossy().to_string();
    if !pattern_string.contains('*') {
        return vec![path];
    }
    let parent = path.parent().unwrap_or(base_dir);
    let file_pattern = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("*");
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry_path| {
            entry_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|name| simple_star_match(file_pattern, name))
                .unwrap_or(false)
        })
        .collect()
}

fn expand_tilde(value: &str) -> String {
    if value == "~" || value.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return value.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    value.to_string()
}

fn simple_star_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let pieces = pattern.split('*').collect::<Vec<_>>();
    if pieces.len() == 1 {
        return pattern == value;
    }
    let mut rest = value;
    if !pieces[0].is_empty() {
        if !rest.starts_with(pieces[0]) {
            return false;
        }
        rest = &rest[pieces[0].len()..];
    }
    for piece in pieces.iter().skip(1).take(pieces.len().saturating_sub(2)) {
        if piece.is_empty() {
            continue;
        }
        let Some(index) = rest.find(piece) else {
            return false;
        };
        rest = &rest[index + piece.len()..];
    }
    if let Some(last) = pieces.last() {
        last.is_empty() || rest.ends_with(last)
    } else {
        true
    }
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or(line)
}

fn source_id_for_alias(alias: &str) -> String {
    let sanitized = sanitize_component(alias);
    if sanitized.is_empty() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        alias.hash(&mut hasher);
        format!("ssh_{:x}", hasher.finish())
    } else {
        format!("ssh_{sanitized}")
    }
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_host_reason_filters_common_code_hosts() {
        assert_eq!(
            ignored_host_reason("github.com", Some("github.com"), Some("git")).as_deref(),
            Some("code-host")
        );
        assert_eq!(
            ignored_host_reason("git.galbot.com", Some("git.galbot.com"), Some("git")).as_deref(),
            Some("code-host")
        );
        assert_eq!(
            ignored_host_reason("internal-gitlab", Some("gitlab.example.com"), None).as_deref(),
            Some("code-host")
        );
        assert_eq!(
            ignored_host_reason("4060_wtxy_dorm", Some("192.168.31.197"), Some("wtxy")),
            None
        );
    }

    #[test]
    fn ignored_host_reason_filters_patterns() {
        assert_eq!(
            ignored_host_reason("*", None, None).as_deref(),
            Some("pattern")
        );
        assert_eq!(
            ignored_host_reason("!bastion", None, None).as_deref(),
            Some("pattern")
        );
    }

    #[test]
    fn remote_tar_command_expands_tilde_on_remote_shell() {
        let command = remote_tar_command("~/.codex");
        assert!(command.starts_with("cd \"$HOME\"/'.codex' || exit 2;"));
        assert!(!command.contains("cd '~/.codex'"));

        let quoted = remote_tar_command("~/Codex Data");
        assert!(quoted.starts_with("cd \"$HOME\"/'Codex Data' || exit 2;"));

        let absolute = remote_tar_command("/opt/codex data");
        assert!(absolute.starts_with("cd '/opt/codex data' || exit 2;"));
    }

    #[test]
    fn remote_download_error_message_hides_raw_bash_cd_error() {
        let error = remote_download_error_message(
            "bash: line 1: cd: ~/.codex: No such file or directory",
            Some(2),
            Some(0),
            "~/.codex",
        );

        assert!(error.contains("远程 Codex 目录不存在"));
        assert!(!error.contains("bash: line 1"));
    }
}
