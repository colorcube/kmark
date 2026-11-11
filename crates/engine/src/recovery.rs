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
}

#[derive(Clone, Debug)]
pub struct RecoveryStore {
    dir: PathBuf,
    enabled: bool,
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
        }
    }

    #[must_use]
    pub fn with_directory(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            enabled: true,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            dir: PathBuf::new(),
            enabled: false,
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn draft_path(&self, tab_id: u64) -> PathBuf {
        let pid = std::process::id();
        self.dir.join(format!("draft-{pid}-{tab_id}.json"))
    }

    pub fn save_draft(&self, draft: &RecoveryDraft) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(draft).map_err(|e| e.to_string())?;
        let path = self.draft_path(draft.tab_id);
        write_file_atomic(&path, &json, TextEncoding::Utf8, LineEnding::Lf)
            .map_err(|e| e.to_string())
    }

    pub fn remove_draft(&self, tab_id: u64) {
        if !self.enabled {
            return;
        }
        let path = self.draft_path(tab_id);
        let _ = fs::remove_file(path);
    }

    #[must_use]
    pub fn load_existing_drafts(&self) -> Vec<(PathBuf, RecoveryDraft)> {
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
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(draft) = serde_json::from_str::<RecoveryDraft>(&raw)
            {
                out.push((path, draft));
            }
        }
        out
    }
}
