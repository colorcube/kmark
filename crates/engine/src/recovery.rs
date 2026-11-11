use std::fs;
use std::path::{Path, PathBuf};

use kmark_buffer::encoding::write_file_atomic;
use kmark_buffer::{LineEnding, TextEncoding};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RecoveryDraft {
    pub tab_id: u64,
    pub original_path: Option<PathBuf>,
    pub title: String,
    pub content: String,
    pub timestamp_unix_secs: u64,
    #[serde(default)]
    pub cursor_byte: usize,
}

#[derive(Clone, Debug)]
pub struct RecoveryStore {
    dir: PathBuf,
    enabled: bool,
    owner_pid: u32,
}

impl Default for RecoveryStore {
    fn default() -> Self {
        Self::xdg_default()
    }
}

impl RecoveryStore {
    #[must_use]
    pub fn xdg_default() -> Self {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .unwrap_or_else(std::env::temp_dir);
        Self {
            dir: base.join("kmark").join("recovery"),
            enabled: true,
            owner_pid: std::process::id(),
        }
    }

    #[must_use]
    pub fn with_directory(dir: impl Into<PathBuf>) -> Self {
        Self::with_directory_and_pid(dir, std::process::id())
    }

    #[must_use]
    pub fn with_directory_and_pid(dir: impl Into<PathBuf>, owner_pid: u32) -> Self {
        Self {
            dir: dir.into(),
            enabled: true,
            owner_pid,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            dir: PathBuf::new(),
            enabled: false,
            owner_pid: std::process::id(),
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub const fn owner_pid(&self) -> u32 {
        self.owner_pid
    }

    fn draft_path(&self, tab_id: u64) -> PathBuf {
        self.dir
            .join(format!("draft-{}-{tab_id}.json", self.owner_pid))
    }

    pub fn save_draft(&self, draft: &RecoveryDraft) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        restrict_private_permissions(&self.dir, true);

        let json = serde_json::to_string_pretty(draft).map_err(|e| e.to_string())?;
        let path = self.draft_path(draft.tab_id);
        write_file_atomic(&path, &json, TextEncoding::Utf8, LineEnding::Lf)
            .map_err(|e| e.to_string())?;
        restrict_private_permissions(&path, false);
        Ok(())
    }

    pub fn remove_draft(&self, tab_id: u64) {
        if !self.enabled {
            return;
        }
        let path = self.draft_path(tab_id);
        let _ = fs::remove_file(path);
    }

    /// Removes all recovery draft files owned by the current session PID on clean shutdown.
    pub fn clear_session_drafts(&self) {
        if !self.enabled {
            return;
        }
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let prefix = format!("draft-{}-", self.owner_pid);
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".json"))
            {
                let _ = fs::remove_file(path);
            }
        }
    }

    /// Loads recovery drafts left behind by non-running KMark processes, pruning corrupted files.
    #[must_use]
    pub fn load_orphaned_drafts(&self) -> Vec<(PathBuf, RecoveryDraft)> {
        if !self.enabled {
            return Vec::new();
        }
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Some(pid) = parse_draft_pid(&path)
                && (pid == self.owner_pid || is_pid_alive(pid))
            {
                continue;
            }
            match fs::read_to_string(&path)
                .ok()
                .and_then(|raw| serde_json::from_str::<RecoveryDraft>(&raw).ok())
            {
                Some(draft) => out.push((path, draft)),
                None => {
                    // Remove unreadable/truncated orphan so recovery directory does not grow forever.
                    let _ = fs::remove_file(&path);
                }
            }
        }
        out.sort_by_key(|(_, d)| (d.timestamp_unix_secs, d.tab_id));
        out
    }

    /// Alias kept for callers inspecting all orphaned drafts.
    #[must_use]
    pub fn load_existing_drafts(&self) -> Vec<(PathBuf, RecoveryDraft)> {
        self.load_orphaned_drafts()
    }
}

fn parse_draft_pid(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    let rest = stem.strip_prefix("draft-")?;
    let (pid_str, _) = rest.split_once('-')?;
    pid_str.parse::<u32>().ok()
}

#[cfg(target_os = "linux")]
fn is_pid_alive(pid: u32) -> bool {
    pid != 0 && Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn is_pid_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn restrict_private_permissions(path: &Path, is_dir: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if is_dir { 0o700 } else { 0o600 };
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn restrict_private_permissions(_path: &Path, _is_dir: bool) {}
