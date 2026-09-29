use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

const NON_SESSION_STEMS: [&str; 2] = ["cwd_latest", "scan_cache"];
const SESSION_EXTENSIONS: [&str; 2] = ["jsonl", "json"];
const ARCHIVE_DIR: &str = "archive";
const LOCKS_DIR: &str = "locks";
const CWD_LATEST_FILE: &str = "cwd_latest.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakiSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub model: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Deserialize)]
struct HeaderRecord {
    t: String,
    id: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    created_at: Option<u64>,
}

#[derive(Deserialize)]
struct MetaRecord {
    t: String,
    title: Option<String>,
    updated_at: Option<u64>,
}

/// Locate candidate maki data roots, most specific first. Each candidate hosts a
/// `sessions` directory; the first existing one wins.
pub fn scan() -> Vec<MakiSession> {
    maki_sessions_dirs()
        .into_iter()
        .find(|dir| dir.is_dir())
        .map(|dir| scan_in(&dir))
        .unwrap_or_default()
}

/// Delete a session, mirroring maki's best-effort cleanup. Only the main log
/// file's removal is load-bearing; archive dirs, lock files, and the
/// `cwd_latest.json` mapping are removed opportunistically.
pub fn delete(id: &str) -> Result<(), String> {
    let Some(dir) = maki_sessions_dirs().into_iter().find(|dir| dir.is_dir()) else {
        return Err(format!("no maki sessions directory for session {id}"));
    };
    delete_in(&dir, id)
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

pub fn scan_in(sessions_dir: &Path) -> Vec<MakiSession> {
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return Vec::new();
    };
    let mut sessions = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
        .filter(|entry| is_session_file(&entry.file_name().to_string_lossy()))
        .filter_map(|entry| read_session(&entry.path()))
        .collect::<Vec<_>>();
    sessions.sort_unstable_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then(left.id.cmp(&right.id))
    });
    sessions
}

pub fn delete_in(sessions_dir: &Path, id: &str) -> Result<(), String> {
    let mut removed = false;
    for extension in SESSION_EXTENSIONS {
        let path = sessions_dir.join(format!("{id}.{extension}"));
        if !path.exists() {
            continue;
        }
        std::fs::remove_file(&path)
            .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        removed = true;
    }
    if !removed {
        return Err(format!("no session log found for session {id}"));
    }
    let _ = std::fs::remove_dir_all(sessions_dir.join(ARCHIVE_DIR).join(id));
    let _ = std::fs::remove_file(sessions_dir.join(LOCKS_DIR).join(id));
    remove_cwd_latest_entry(sessions_dir, id);
    Ok(())
}

pub fn relative_age(updated_at: u64, now: u64) -> String {
    let elapsed = now.saturating_sub(updated_at);
    match elapsed {
        0..=59 => format!("{elapsed}s"),
        60..=3599 => format!("{}m", elapsed / 60),
        3600..=86399 => format!("{}h", elapsed / 3600),
        _ => format!("{}d", elapsed / 86400),
    }
}

fn maki_sessions_dirs() -> Vec<PathBuf> {
    crate::platform::maki_data_dirs()
        .into_iter()
        .map(|base| base.join("sessions"))
        .collect()
}

fn is_session_file(name: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !NON_SESSION_STEMS.contains(&stem) && SESSION_EXTENSIONS.contains(&extension)
}

fn read_session(path: &Path) -> Option<MakiSession> {
    let mut first = true;
    let (mut id, mut model, mut cwd, mut created_at) =
        (String::new(), String::new(), String::new(), 0);
    let (mut title, mut updated_at) = (None, None);
    let file = std::fs::File::open(path).ok()?;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if first {
            first = false;
            let header: HeaderRecord = serde_json::from_str(&line).ok()?;
            if header.t != "header" {
                return None;
            }
            id = header.id?;
            model = header.model.unwrap_or_default();
            cwd = header.cwd.unwrap_or_default();
            created_at = header.created_at.unwrap_or(0);
            continue;
        }
        if let Ok(meta) = serde_json::from_str::<MetaRecord>(&line) {
            if meta.t == "meta" {
                title = meta.title;
                updated_at = meta.updated_at;
            }
        }
    }
    Some(MakiSession {
        id,
        title: normalize_title(&title.unwrap_or_default()),
        cwd,
        model,
        created_at,
        updated_at: updated_at.unwrap_or(created_at),
    })
}

fn normalize_title(title: &str) -> String {
    let collapsed = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "New session".to_owned()
    } else {
        collapsed
    }
}

fn remove_cwd_latest_entry(sessions_dir: &Path, id: &str) {
    let path = sessions_dir.join(CWD_LATEST_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(map) = value.as_object_mut() else {
        return;
    };
    if map.remove(id).is_none() {
        return;
    }
    if let Ok(serialized) = serde_json::to_string(&value) {
        let _ = std::fs::write(&path, serialized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempSessionsDir(PathBuf);

    impl TempSessionsDir {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "herdr-maki-{}-{}-{label}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&dir).expect("create sessions dir");
            Self(dir)
        }

        fn write_session(&self, name: &str, lines: &[&str]) {
            std::fs::write(self.0.join(name), lines.join("\n")).expect("write session");
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempSessionsDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const HEADER: &str =
        r#"{"t":"header","v":2,"id":"aaaabbbb","model":"glm-4","cwd":"/tmp/x","created_at":100}"#;
    const META: &str =
        r#"{"t":"meta","title":"fix  the\n thing","token_usage":{},"updated_at":300}"#;
    const META_STALE: &str = r#"{"t":"meta","title":"older","updated_at":200}"#;

    #[test]
    fn scan_reads_header_and_last_meta() {
        let dir = TempSessionsDir::new("scan");
        dir.write_session(
            "aaaabbbb.jsonl",
            &[HEADER, r#"{"t":"msg"}"#, META_STALE, META],
        );
        dir.write_session("cwd_latest.json", &[r#"{"aaaabbbb":"/tmp/x"}"#]);

        let sessions = scan_in(dir.path());

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.id, "aaaabbbb");
        assert_eq!(session.title, "fix the thing");
        assert_eq!(session.model, "glm-4");
        assert_eq!(session.cwd, "/tmp/x");
        assert_eq!(session.created_at, 100);
        assert_eq!(session.updated_at, 300);
    }

    #[test]
    fn scan_sorts_by_updated_at_desc_then_id() {
        let dir = TempSessionsDir::new("sort");
        dir.write_session(
            "ccc.jsonl",
            &[r#"{"t":"header","id":"ccc","created_at":1}"#, META],
        );
        dir.write_session(
            "aaa.jsonl",
            &[r#"{"t":"header","id":"aaa","created_at":1}"#, META],
        );
        dir.write_session(
            "bbb.jsonl",
            &[
                r#"{"t":"header","id":"bbb","created_at":1}"#,
                &META.replace("300", "400"),
            ],
        );

        let ids = scan_in(dir.path())
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["bbb", "aaa", "ccc"]);
    }

    #[test]
    fn scan_defaults_missing_title_and_updated_at() {
        let dir = TempSessionsDir::new("defaults");
        dir.write_session(
            "aaa.jsonl",
            &[r#"{"t":"header","id":"aaa","created_at":42}"#],
        );

        let sessions = scan_in(dir.path());

        assert_eq!(sessions[0].title, "New session");
        assert_eq!(sessions[0].updated_at, 42);
    }

    #[test]
    fn scan_skips_non_session_files_and_bad_headers() {
        let dir = TempSessionsDir::new("skip");
        dir.write_session("scan_cache.json", &[r#"{"t":"header","id":"x"}"#]);
        dir.write_session("not_a_session.txt", &[HEADER]);
        dir.write_session("broken.jsonl", &[r#"{"t":"msg"}"#]);
        dir.write_session("no_header_id.jsonl", &[r#"{"t":"header"}"#]);

        assert!(scan_in(dir.path()).is_empty());
    }

    #[test]
    fn delete_removes_log_and_best_effort_extras() {
        let dir = TempSessionsDir::new("delete");
        dir.write_session("aaa.jsonl", &[HEADER, META]);
        std::fs::create_dir_all(dir.path().join("archive/aaa/inner")).expect("archive dir");
        std::fs::write(dir.path().join("archive/aaa/inner/blob"), "x").expect("archive blob");
        std::fs::create_dir_all(dir.path().join("locks")).expect("locks dir");
        std::fs::write(dir.path().join("locks/aaa"), "").expect("lock file");
        std::fs::write(
            dir.path().join(CWD_LATEST_FILE),
            r#"{"aaa":"/tmp/x","bbb":"/tmp/y"}"#,
        )
        .expect("cwd latest");

        delete_in(dir.path(), "aaa").expect("delete");

        assert!(!dir.path().join("aaa.jsonl").exists());
        assert!(!dir.path().join("archive/aaa").exists());
        assert!(!dir.path().join("locks/aaa").exists());
        let cwd_latest = std::fs::read_to_string(dir.path().join(CWD_LATEST_FILE)).unwrap();
        assert_eq!(cwd_latest, r#"{"bbb":"/tmp/y"}"#);
    }

    #[test]
    fn delete_falls_back_to_legacy_log_extension() {
        let dir = TempSessionsDir::new("legacy");
        dir.write_session("aaa.json", &[HEADER, META]);

        delete_in(dir.path(), "aaa").expect("delete");

        assert!(!dir.path().join("aaa.json").exists());
    }

    #[test]
    fn delete_errors_when_log_cannot_be_removed() {
        let dir = TempSessionsDir::new("locked");
        dir.write_session("aaa.jsonl", &[HEADER, META]);
        // A directory at the log path makes remove_file fail.
        std::fs::remove_file(dir.path().join("aaa.jsonl")).expect("drop log file");
        std::fs::create_dir_all(dir.path().join("aaa.jsonl")).expect("dir at log path");

        let err = delete_in(dir.path(), "aaa").unwrap_err();

        assert!(err.contains("failed to remove"));
    }

    #[test]
    fn delete_errors_for_unknown_session() {
        let dir = TempSessionsDir::new("unknown");

        let err = delete_in(dir.path(), "missing").unwrap_err();

        assert!(err.contains("no session log"));
    }

    #[test]
    fn relative_age_buckets_are_human() {
        assert_eq!(relative_age(1_000, 1_000), "0s");
        assert_eq!(relative_age(0, 180), "3m");
        assert_eq!(relative_age(0, 7_200), "2h");
        assert_eq!(relative_age(0, 432_000), "5d");
    }
}
