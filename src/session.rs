//! User mode is the child's board. Setup mode is the parent's.
//! A short look never opens setup. The parent holds the Parent control,
//! or uses Ctrl+P from the keyboard.

use std::collections::{BTreeMap, BTreeSet};

use crate::board::{self, Board, DwellKind};
use crate::predict::{self, Graph};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    User,
    Setup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Mute,
    Fullscreen,
    Parent,
    Back,
    Category(&'static str),
    Word(&'static str),
    SayAll,
    Delete,
    Clear,
    ChooseBoard(Board),
    /// How many words are on the board. Does not reset the turn count.
    ChooseSkill(u8),
    ToggleWord(&'static str),
    /// Open the full category board while Predicted words is the stage.
    OtherWords,
    /// Show a longer list of likely next words.
    MoreWords,
    /// Play the whole sentence in the headphones.
    Hear,
    /// How the board looks. Does not reset the turn count.
    ChooseTheme(crate::board::Theme),
    /// Headphones output. The number is an index into the speaker list.
    ChooseEar(u8),
    /// Room speaker. The number is an index into the speaker list.
    ChooseRoom(u8),
    /// Which espeak-ng voice speaks. The number is an index into the voice list.
    ChooseVoice(u8),
    /// Speech speed. 1 is slower, 2 is the shipped speed, 4 is fastest.
    ChooseSpeed(u8),
    /// Word graph or the local Llama test.
    ChooseEngine(board::Engine),
    Done,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub speak: Option<String>,
    /// True when this line should play on the room speaker. False plays in the headphones.
    pub loud: bool,
    pub stop_voice: bool,
    pub save: bool,
}

impl Effect {
    fn quiet() -> Self {
        Self {
            speak: None,
            loud: false,
            stop_voice: false,
            save: false,
        }
    }

    fn ear(text: String) -> Self {
        Self {
            speak: Some(text),
            loud: false,
            stop_voice: false,
            save: false,
        }
    }

    fn room(text: String) -> Self {
        Self {
            speak: Some(text),
            loud: true,
            stop_voice: false,
            save: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub mode: Mode,
    pub board: Board,
    pub page: Option<&'static str>,
    /// Words in the line being said now.
    pub sentence: Vec<&'static str>,
    /// End index of each added chunk, so Delete removes a phrase, not one letter.
    pub chunks: Vec<usize>,
    /// True once this line has been offered as a finished message.
    pub delivered: bool,
    /// Finished lines still on screen, newest last. Capped at three.
    pub history: Vec<String>,
    /// Messages finished on this stage. The parent changes stage. The board does not.
    pub turns: u32,
    pub spoken: Vec<String>,
    pub heard: String,
    pub hidden: BTreeSet<String>,
    pub muted: bool,
    /// True while Predicted words is showing categories instead of guesses.
    pub picking: bool,
    /// Predicted words is showing a longer guess list.
    pub predict_more: bool,
    /// Links learned from finished lines on this computer.
    pub learned: Graph,
    pub graph_dirty: bool,
    /// 1 shows the first words. 4 shows the whole folder.
    pub skill: u8,
    pub theme: crate::board::Theme,
    /// PulseAudio sink name. Empty uses the computer's default output.
    pub ear: String,
    pub room: String,
    /// espeak-ng voice id. Empty or unknown falls back to Adam.
    pub voice: String,
    /// 1 slower, 2 the speed the board shipped with, 3 faster, 4 fastest.
    pub speed: u8,
    pub engine: board::Engine,
    /// Sentence the Llama list belongs to.
    pub llama_key: Vec<String>,
    pub llama_ids: Vec<String>,
    pub llama_status: String,
    /// Filled when setup opens. Not saved.
    pub outputs: Vec<(String, String)>,
}

impl Session {
    pub fn fresh() -> Self {
        Self {
            mode: Mode::User,
            board: Board::Big,
            page: None,
            sentence: Vec::new(),
            chunks: Vec::new(),
            delivered: false,
            history: Vec::new(),
            turns: 0,
            spoken: Vec::new(),
            heard: String::new(),
            hidden: BTreeSet::new(),
            muted: false,
            picking: false,
            predict_more: false,
            learned: Graph::new(),
            graph_dirty: false,
            skill: 1,
            theme: crate::board::Theme::Simple,
            ear: String::new(),
            room: String::new(),
            voice: "en-us".to_string(),
            speed: 2,
            engine: board::Engine::Graph,
            llama_key: Vec::new(),
            llama_ids: Vec::new(),
            llama_status: String::new(),
            outputs: Vec::new(),
        }
    }

    pub fn sentence_ids(&self) -> Vec<&str> {
        self.sentence.iter().copied().collect()
    }

    pub fn sentence_says(&self) -> Vec<&str> {
        self.sentence
            .iter()
            .filter_map(|id| board::word_by_id(id).map(board::display_piece))
            .collect()
    }

    pub fn view(&self, eyes: bool) -> View<'_> {
        let predict_home =
            self.board == Board::Predict && self.page.is_none() && !self.picking;
        let building = self.mode == Mode::User
            && !predict_home
            && (self.board == Board::Sentences || !self.sentence.is_empty());
        let next_ids = if !building {
            Vec::new()
        } else {
            predict::mix(
                &self.sentence_ids(),
                6,
                &self.hidden,
                &self.learned,
                self.skill,
                llama_slice(self),
            )
            .into_iter()
            .map(|word| word.id)
            .collect()
        };
        View {
            mode: self.mode,
            board: self.board,
            page: self.page,
            sentence: self
                .sentence_says()
                .into_iter()
                .map(str::to_string)
                .collect(),
            next_ids,
            ids: self.sentence.clone(),
            picking: self.picking,
            predict_more: self.predict_more,
            learned: &self.learned,
            heard: &self.heard,
            earlier: self.history.last().map(String::as_str),
            delivered: self.delivered,
            turns: self.turns,
            note: {
                let mut note = board::progress_note(self.board, self.turns, self.skill);
                if self.mode == Mode::Setup && !self.llama_status.is_empty() {
                    note.push(' ');
                    note.push_str(&self.llama_status);
                }
                note
            },
            skill: self.skill,
            theme: self.theme,
            ear: &self.ear,
            room: &self.room,
            outputs: &self.outputs,
            voice: &self.voice,
            speed: self.speed,
            engine: self.engine,
            llama_ids: &self.llama_ids,
            llama_hit: self.engine == board::Engine::Llama
                && self.llama_key
                    == self
                        .sentence
                        .iter()
                        .map(|id| (*id).to_string())
                        .collect::<Vec<_>>(),
            llama_status: &self.llama_status,
            muted: self.muted,
            eyes,
            hidden: &self.hidden,
        }
    }

    pub fn enter_setup(&mut self) {
        self.mode = Mode::Setup;
        self.page = None;
    }

    pub fn activate(&mut self, action: Action) -> Effect {
        match action {
            Action::Mute => {
                self.muted = !self.muted;
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: self.muted,
                    save: true,
                }
            }
            Action::Fullscreen | Action::Parent => Effect::quiet(),
            Action::Back => {
                if self.page.is_some() {
                    self.page = None;
                } else {
                    self.picking = false;
                }
                Effect::quiet()
            }
            Action::OtherWords => {
                self.picking = true;
                self.page = None;
                Effect::quiet()
            }
            Action::MoreWords => {
                self.predict_more = !self.predict_more;
                self.picking = false;
                self.page = None;
                Effect::quiet()
            }
            Action::Category(id) => {
                if board::category(id).is_none() {
                    return Effect::quiet();
                }
                self.page = Some(id);
                Effect::quiet()
            }
            Action::Word(id) => self.choose(id),
            Action::SayAll => self.say_out_loud(),
            Action::Hear => self.hear_line(),
            Action::ChooseTheme(theme) => {
                self.theme = theme;
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::ChooseEar(index) => {
                if let Some((id, _)) = self.outputs.get(index as usize) {
                    self.ear = id.clone();
                    let mut effect = Effect::ear("Headphones".to_string());
                    effect.save = true;
                    return effect;
                }
                Effect::quiet()
            }
            Action::ChooseSpeed(speed) => {
                self.speed = speed.clamp(1, 4);
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::ChooseEngine(engine) => {
                if self.engine != engine {
                    self.engine = engine;
                    self.llama_key.clear();
                    self.llama_ids.clear();
                    self.llama_status = if engine == board::Engine::Llama {
                        "Llama is loading the model.".to_string()
                    } else {
                        String::new()
                    };
                }
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::ChooseVoice(index) => {
                if let Some(choice) = board::speech_voices().get(index as usize) {
                    self.voice = choice.id.to_string();
                    let mut effect = Effect::ear(choice.label.to_string());
                    effect.save = true;
                    return effect;
                }
                Effect::quiet()
            }
            Action::ChooseRoom(index) => {
                if let Some((id, _)) = self.outputs.get(index as usize) {
                    self.room = id.clone();
                    let mut effect = Effect::room("Speakers".to_string());
                    effect.save = true;
                    return effect;
                }
                Effect::quiet()
            }
            Action::Delete => self.delete_chunk(),
            Action::Clear => {
                self.sentence.clear();
                self.chunks.clear();
                self.delivered = false;
                self.heard.clear();
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: true,
                    save: false,
                }
            }
            Action::ChooseSkill(level) => {
                let level = level.clamp(1, 4);
                if self.skill != level {
                    self.skill = level;
                    if self.page.is_some_and(|id| !category_has_skill(id, level)) {
                        self.page = None;
                    }
                }
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::ChooseBoard(board) => {
                if self.board != board {
                    self.turns = 0;
                    self.board = board;
                    self.sentence.clear();
                    self.chunks.clear();
                    self.delivered = false;
                    self.heard.clear();
                    self.history.clear();
                    self.page = None;
                    self.picking = false;
                    self.predict_more = false;
                }
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::ToggleWord(id) => {
                if board::word_by_id(id).is_none() {
                    return Effect::quiet();
                }
                if !self.hidden.remove(id) {
                    self.hidden.insert(id.to_string());
                }
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
            Action::Done => {
                self.mode = Mode::User;
                self.page = None;
                self.picking = false;
                Effect {
                    speak: None,
                    loud: false,
                    stop_voice: false,
                    save: true,
                }
            }
        }
    }

    fn choose(&mut self, id: &str) -> Effect {
        let Some(word) = board::word_by_id(id) else {
            return Effect::quiet();
        };
        if self.hidden.contains(word.id) || !board::in_skill(word, self.skill) {
            return Effect::quiet();
        }
        if self.board == Board::Predict {
            self.page = None;
            self.picking = false;
            self.predict_more = false;
        }
        let before = self.sentence.len();
        let expand = self.sentence.is_empty()
            && (self.board == Board::Big
                || (self.board == Board::Medium && word.role == board::Role::Object));
        if expand {
            self.push_ids(&board::first_phrase(word.id));
        } else {
            self.push_ids(&[word.id]);
        }
        let added = self.sentence[before..]
            .iter()
            .filter_map(|id| board::word_by_id(id).map(board::speech_piece))
            .collect::<Vec<_>>()
            .join(" ");
        if added.is_empty() {
            return Effect::quiet();
        }
        self.delivered = false;
        self.heard = self.sentence_says().join(" ");
        self.spoken.push(added.clone());
        Effect::ear(added)
    }

    fn push_ids(&mut self, ids: &[&'static str]) {
        for id in ids {
            let Some(word) = board::word_by_id(id) else {
                continue;
            };
            if self.hidden.contains(word.id) || !board::in_skill(word, self.skill) {
                continue;
            }
            if self.sentence.contains(&word.id) && !board::allows_repeat(word) {
                continue;
            }
            self.sentence.push(word.id);
        }
        self.chunks.push(self.sentence.len());
    }

    fn sentence_speech(&self) -> String {
        self.sentence
            .iter()
            .filter_map(|id| board::word_by_id(id).map(board::speech_piece))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn hear_line(&mut self) -> Effect {
        if self.sentence.is_empty() {
            return Effect::quiet();
        }
        let shown = self.sentence_says().join(" ");
        let spoken_line = self.sentence_speech();
        self.heard = shown;
        self.spoken.push(spoken_line.clone());
        Effect::ear(spoken_line)
    }

    fn say_out_loud(&mut self) -> Effect {
        if self.sentence.is_empty() {
            return Effect::quiet();
        }
        let shown = self.sentence_says().join(" ");
        let spoken_line = self.sentence_speech();
        self.heard = shown.clone();
        self.spoken.push(spoken_line.clone());
        let ids: Vec<&str> = self.sentence.to_vec();
        if predict::learn(&mut self.learned, &ids) {
            self.graph_dirty = true;
        }
        self.delivered = true;
        self.turns = self.turns.saturating_add(1);
        if self.history.last().map(String::as_str) != Some(shown.as_str()) {
            self.history.push(shown);
            if self.history.len() > 3 {
                self.history.remove(0);
            }
        }
        Effect::room(spoken_line)
    }

    fn delete_chunk(&mut self) -> Effect {
        let Some(_) = self.chunks.pop() else {
            return Effect::quiet();
        };
        let keep = self.chunks.last().copied().unwrap_or(0);
        self.sentence.truncate(keep);
        self.delivered = false;
        if self.sentence.is_empty() {
            self.heard.clear();
        } else {
            self.heard = self.sentence_says().join(" ");
        }
        Effect {
            speak: None,
            loud: false,
            stop_voice: true,
            save: false,
        }
    }
}

/// What the window needs to draw. Borrowed from the session for one layout pass.
pub struct View<'a> {
    pub mode: Mode,
    pub board: Board,
    pub page: Option<&'static str>,
    pub sentence: Vec<String>,
    pub next_ids: Vec<&'static str>,
    pub ids: Vec<&'static str>,
    pub picking: bool,
    pub predict_more: bool,
    pub learned: &'a BTreeMap<(String, String), f64>,
    pub heard: &'a str,
    pub earlier: Option<&'a str>,
    pub delivered: bool,
    pub turns: u32,
    pub note: String,
    pub muted: bool,
    pub eyes: bool,
    pub hidden: &'a BTreeSet<String>,
    pub skill: u8,
    pub theme: crate::board::Theme,
    pub ear: &'a str,
    pub room: &'a str,
    pub outputs: &'a [(String, String)],
    pub voice: &'a str,
    pub speed: u8,
    pub engine: board::Engine,
    pub llama_ids: &'a [String],
    pub llama_hit: bool,
    pub llama_status: &'a str,
}

fn llama_slice(session: &Session) -> Option<&[String]> {
    if session.engine == board::Engine::Llama
        && session.llama_key
            == session
                .sentence
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
    {
        Some(session.llama_ids.as_slice())
    } else {
        None
    }
}

fn category_has_skill(id: &str, skill: u8) -> bool {
    board::category(id).is_some_and(|cat| {
        cat.words.iter().any(|word_id| {
            board::word_by_id(word_id).is_some_and(|word| board::in_skill(word, skill))
        })
    })
}

/// Look-to-press timer. The key changing starts the hold over.
#[derive(Clone, Debug)]
pub struct Dwell {
    key: Option<Action>,
    start_ms: u64,
    fired: bool,
}

impl Dwell {
    pub fn new() -> Self {
        Self {
            key: None,
            start_ms: 0,
            fired: false,
        }
    }

    pub fn reset(&mut self) {
        self.key = None;
        self.fired = false;
    }

    /// `true` once, when the look has stayed on `key` for `need_ms`.
    pub fn tick(&mut self, key: Option<Action>, now_ms: u64, need_ms: u64) -> bool {
        if key != self.key {
            self.key = key;
            self.start_ms = now_ms;
            self.fired = false;
        }
        if key.is_none() || self.fired {
            return false;
        }
        if now_ms.saturating_sub(self.start_ms) >= need_ms {
            self.fired = true;
            return true;
        }
        false
    }

    pub fn progress(&self, now_ms: u64, need_ms: u64) -> f64 {
        if self.key.is_none() || need_ms == 0 {
            return 0.0;
        }
        if self.fired {
            return 1.0;
        }
        (now_ms.saturating_sub(self.start_ms) as f64 / need_ms as f64).clamp(0.0, 1.0)
    }
}

impl Default for Dwell {
    fn default() -> Self {
        Self::new()
    }
}

pub fn dwell_kind(mode: Mode, action: Action) -> DwellKind {
    match action {
        Action::Parent => DwellKind::Parent,
        Action::Word("yes")
        | Action::Word("no")
        | Action::Word("and")
        | Action::Word("or")
        | Action::Word("pause")
            if mode == Mode::User =>
        {
            DwellKind::Steady
        }
        Action::Word(_) if mode == Mode::User => DwellKind::Word,
        Action::ChooseBoard(_)
        | Action::ChooseSkill(_)
        | Action::ChooseTheme(_)
        | Action::ChooseEar(_)
        | Action::ChooseRoom(_)
        | Action::ChooseVoice(_)
        | Action::ChooseSpeed(_)
        | Action::ChooseEngine(_)
        | Action::ToggleWord(_) => DwellKind::Choice,
        _ => DwellKind::Menu,
    }
}

pub fn hold_ms(mode: Mode, board: Board, action: Action) -> u64 {
    board::dwell_ms(mode == Mode::Setup, board, dwell_kind(mode, action))
}
