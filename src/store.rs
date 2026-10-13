//! Parent choices live in `$XDG_CONFIG_HOME/say/board.json`
//! (or `~/.config/say/board.json`). A bad file falls back to the big board
//! so the child still has something to say.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::board::{Board, Theme};
use crate::session::Session;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Saved {
    pub board: Board,
    pub hidden: BTreeSet<String>,
    pub muted: bool,
    /// Finished messages on the current stage. Missing in older files.
    #[serde(default)]
    pub turns: u32,
    /// How many words are on the board. Missing in older files, which stay at Starting.
    #[serde(default = "starting_skill")]
    pub skill: u8,
    /// Simple is the calm board. Future is the space-age board.
    #[serde(default)]
    pub theme: Theme,
    /// Headphones. Empty uses the default output.
    #[serde(default)]
    pub ear: String,
    /// Room speaker for Say it out loud. Empty uses the default output.
    #[serde(default)]
    pub room: String,
    /// espeak-ng voice id. Missing files keep Adam.
    #[serde(default = "adam")]
    pub voice: String,
    /// 2 is the speed the board shipped with.
    #[serde(default = "usual_speed")]
    pub speed: u8,
    /// Graph is the word graph. Llama is the local model test.
    #[serde(default)]
    pub engine: crate::board::Engine,
}

fn usual_speed() -> u8 {
    2
}

fn adam() -> String {
    "en-us".to_string()
}

fn starting_skill() -> u8 {
    1
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            board: Board::Big,
            hidden: BTreeSet::new(),
            muted: false,
            turns: 0,
            skill: 1,
            theme: Theme::Simple,
            ear: String::new(),
            room: String::new(),
            voice: "en-us".to_string(),
            speed: 2,
            engine: crate::board::Engine::Graph,
        }
    }
}

impl Saved {
    pub fn from_session(session: &Session) -> Self {
        Self {
            board: session.board,
            hidden: session.hidden.clone(),
            muted: session.muted,
            turns: session.turns,
            skill: session.skill.clamp(1, 4),
            theme: session.theme,
            ear: session.ear.clone(),
            room: session.room.clone(),
            voice: if crate::board::speech_voice(&session.voice).is_some() {
                session.voice.clone()
            } else {
                "en-us".to_string()
            },
            speed: session.speed.clamp(1, 4),
            engine: session.engine,
        }
    }

    pub fn apply(&self, session: &mut Session) {
        session.board = self.board;
        session.turns = self.turns;
        session.hidden = self
            .hidden
            .iter()
            .filter(|id| crate::board::word_by_id(id).is_some())
            .cloned()
            .collect();
        session.muted = self.muted;
        session.skill = self.skill.clamp(1, 4);
        session.theme = self.theme;
        session.ear = self.ear.clone();
        session.room = self.room.clone();
        session.voice = if crate::board::speech_voice(&self.voice).is_some() {
            self.voice.clone()
        } else {
            "en-us".to_string()
        };
        session.speed = self.speed.clamp(1, 4);
        session.engine = self.engine;
    }
}

pub fn graph_path() -> Option<PathBuf> {
    config_path().map(|path| path.with_file_name("predict.json"))
}

pub fn load_graph(path: &Path) -> BTreeMap<(String, String), f64> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(file) = serde_json::from_str::<GraphFile>(&text) else {
        return BTreeMap::new();
    };
    let mut graph = BTreeMap::new();
    for edge in file.edges {
        if edge.weight.is_finite() && edge.weight > 0.0 && !edge.from.is_empty() && !edge.to.is_empty()
        {
            graph.insert((edge.from, edge.to), edge.weight.min(8.0));
        }
    }
    graph
}

pub fn save_graph(path: &Path, graph: &BTreeMap<(String, String), f64>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = GraphFile {
        edges: graph
            .iter()
            .map(|((from, to), weight)| GraphEdge {
                from: from.clone(),
                to: to.clone(),
                weight: *weight,
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&file).unwrap_or_else(|_| "{}".to_string());
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text + "\n")?;
    fs::rename(&tmp, path)
}

#[derive(Serialize, Deserialize)]
struct GraphFile {
    edges: Vec<GraphEdge>,
}

#[derive(Serialize, Deserialize)]
struct GraphEdge {
    from: String,
    to: String,
    weight: f64,
}

pub fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("say/board.json"))
}

pub fn load(path: &Path) -> Saved {
    let Ok(text) = fs::read_to_string(path) else {
        return Saved::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(path: &Path, saved: &Saved) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(saved).unwrap_or_else(|_| "{}".to_string());
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text + "\n")?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_a_hidden_word() {
        let dir = std::env::temp_dir().join(format!("say-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("board.json");
        let mut saved = Saved::default();
        saved.board = Board::Sentences;
        saved.hidden.insert("cookie".to_string());
        saved.muted = true;
        saved.skill = 3;
        saved.theme = Theme::Future;
        saved.ear = "headphones".to_string();
        save(&path, &saved).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, saved);
        let junk = dir.join("bad.json");
        fs::write(&junk, "{").unwrap();
        assert_eq!(load(&junk), Saved::default());
        let _ = fs::remove_dir_all(&dir);
    }
}
