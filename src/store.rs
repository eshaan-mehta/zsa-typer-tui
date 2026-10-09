//! Settings, the confirmed layout, manual edits, and a per-revision layout cache, on disk.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::layout::{Edit, Layout};
use crate::stats::{Stats, Targets};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Learn your layout a few letters at a time.
    Progressive,
    /// Practice the letters you're slowest or least accurate on.
    WeakKeys,
    /// Random common words.
    Words,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Progressive, Mode::WeakKeys, Mode::Words];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Progressive => "progressive",
            Mode::WeakKeys => "weak keys",
            Mode::Words => "words",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub hints: bool,
    /// End the test on the first wrong key.
    pub instant_death: bool,
    pub word_count: usize,
    /// Per-letter speed needed to unlock the next letter in progressive mode.
    pub target_wpm: u32,
    /// Per-letter accuracy (percent) needed alongside the speed.
    pub target_accuracy: u32,
}

pub const TARGET_WPM_RANGE: std::ops::RangeInclusive<u32> = 10..=200;
pub const TARGET_ACCURACY_RANGE: std::ops::RangeInclusive<u32> = 50..=100;
pub const WORD_COUNT_RANGE: std::ops::RangeInclusive<u32> = 5..=500;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Progressive,
            hints: true,
            instant_death: false,
            word_count: 25,
            target_wpm: 30,
            target_accuracy: 95,
        }
    }
}

impl Settings {
    pub fn targets(&self) -> Targets {
        let wpm = self.target_wpm.clamp(*TARGET_WPM_RANGE.start(), *TARGET_WPM_RANGE.end());
        let acc = self.target_accuracy.clamp(*TARGET_ACCURACY_RANGE.start(), *TARGET_ACCURACY_RANGE.end());
        Targets { wpm: wpm as f64, accuracy: acc as f64 / 100.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutRef {
    pub layout_id: String,
    pub revision_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub settings: Settings,
    /// The layout revision the user last confirmed.
    pub confirmed: Option<LayoutRef>,
    /// The user's corrections, applied on top of whichever revision is confirmed.
    pub edits: Vec<Edit>,
}

pub struct Store {
    dir: PathBuf,
    pub saved: Saved,
}

impl Store {
    pub fn open() -> Self {
        let dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("zsa-typer-tui");
        let saved = read_json(&dir.join("state.json")).unwrap_or_default();
        Store { dir, saved }
    }

    pub fn save(&self) -> Result<()> {
        write_json(&self.dir.join("state.json"), &self.saved)
    }

    fn layout_path(&self, layout_id: &str, revision_id: &str) -> PathBuf {
        self.dir.join("layouts").join(format!("{layout_id}-{revision_id}.json"))
    }

    /// A previously fetched layout revision (without edits).
    pub fn cached_layout(&self, layout_id: &str, revision_id: &str) -> Option<Layout> {
        let layout: Layout = read_json(&self.layout_path(layout_id, revision_id))?;
        layout.validate().ok()?;
        Some(layout)
    }

    pub fn cache_layout(&self, layout: &Layout) -> Result<()> {
        write_json(&self.layout_path(&layout.layout_id, &layout.revision_id), layout)
    }

    pub fn load_stats(&self) -> Stats {
        read_json(&self.dir.join("stats.json")).unwrap_or_default()
    }

    pub fn save_stats(&self, stats: &Stats) -> Result<()> {
        write_json(&self.dir.join("stats.json"), stats)
    }

    /// The confirmed layout with the user's edits applied.
    pub fn confirmed_layout(&self) -> Option<Layout> {
        let r = self.saved.confirmed.as_ref()?;
        Some(self.cached_layout(&r.layout_id, &r.revision_id)?.with_edits(&self.saved.edits))
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().context("bad path")?;
    fs::create_dir_all(parent).with_context(|| format!("couldn't create {}", parent.display()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    fs::rename(&tmp, path).with_context(|| format!("couldn't write {}", path.display()))?;
    Ok(())
}
