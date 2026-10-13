//! Words, categories, and the next-word graph from the Say board.
//!
//! A word is a small vector of the job it does in a sentence. Edges say
//! which words often follow. Suggestions add those two scores.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Board {
    Big,
    Medium,
    Sentences,
    Predict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Graph,
    Llama,
}

impl Engine {
    pub fn label(self) -> &'static str {
        match self {
            Engine::Graph => "Word graph",
            Engine::Llama => "Llama",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Simple,
    Future,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Theme::Simple => "Simple",
            Theme::Future => "Future",
        }
    }
}

/// A voice espeak-ng can speak. The id is the `-v` argument.
pub struct SpeechVoice {
    pub id: &'static str,
    pub label: &'static str,
}

pub fn speech_voices() -> &'static [SpeechVoice] {
    &[
        SpeechVoice { id: "en-us", label: "Adam" },
        SpeechVoice { id: "en-us+Michael", label: "Michael" },
        SpeechVoice { id: "en-us+Jacky", label: "Jack" },
        SpeechVoice { id: "en-gb", label: "Britain" },
        SpeechVoice { id: "en-gb-scotland", label: "Scotland" },
        SpeechVoice { id: "en-us-nyc", label: "New York" },
        SpeechVoice { id: "en-029", label: "Caribbean" },
        SpeechVoice { id: "en-us+Alicia", label: "Alicia" },
        SpeechVoice { id: "en-us+Annie", label: "Annie" },
        SpeechVoice { id: "en-us+Linda", label: "Linda" },
        SpeechVoice { id: "en-us+Auntie", label: "Auntie" },
        SpeechVoice { id: "en-us+grandma", label: "Grandma" },
    ]
}

pub fn speech_voice(id: &str) -> Option<&'static SpeechVoice> {
    speech_voices().iter().find(|voice| voice.id == id)
}

impl Board {
    pub fn label(self) -> &'static str {
        match self {
            Board::Big => "First messages",
            Board::Medium => "Growing phrases",
            Board::Sentences => "Conversation",
            Board::Predict => "Predicted words",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Board::Big => "One picture can still say a request. The line stays until you clear it.",
            Board::Medium => "Build a thought. One picture can still make a request.",
            Board::Sentences => "Keep talking. The last line stays, and the next follows.",
            Board::Predict => "The board brings up words that fit this sentence.",
        }
    }

    /// espeak-ng words per minute. Big is a little slower, matching the page.
    pub fn speech_wpm(self) -> i32 {
        match self {
            Board::Big => 150,
            Board::Medium | Board::Sentences | Board::Predict => 166,
        }
    }
}

impl<'de> Deserialize<'de> for Board {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match text.as_str() {
            "big" => Ok(Board::Big),
            "medium" => Ok(Board::Medium),
            "sentences" | "advanced" => Ok(Board::Sentences),
            "predict" => Ok(Board::Predict),
            other => Err(serde::de::Error::custom(format!("unknown board {other}"))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Person,
    Verb,
    Object,
    Feeling,
    Place,
    Polite,
    Social,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Person => "person",
            Role::Verb => "verb",
            Role::Object => "object",
            Role::Feeling => "feeling",
            Role::Place => "place",
            Role::Polite => "polite",
            Role::Social => "social",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "person" => Role::Person,
            "verb" => Role::Verb,
            "object" => Role::Object,
            "feeling" => Role::Feeling,
            "place" => Role::Place,
            "polite" => Role::Polite,
            "social" => Role::Social,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Word {
    pub id: &'static str,
    pub say: &'static str,
    pub role: Role,
    pub vec: [f64; 7],
    /// Whole line First messages speaks. Empty means the picture speaks itself.
    pub phrase: &'static [&'static str],
    pub starter: f64,
    /// A person or verb with this set can finish a thought on its own.
    pub closes: bool,
    /// This word may be used again in the same line, such as and or or.
    pub repeat: bool,
    pub also: &'static [&'static str],
    pub next: &'static [(&'static str, f64)],
    /// Absolute path of the picture, or empty when the file is missing.
    pub icon: &'static str,
    /// 1 is the first set. A parent skill hides every word above it.
    pub level: u8,
}

pub fn skill_label(skill: u8) -> &'static str {
    match skill.clamp(1, 4) {
        1 => "Starting",
        2 => "Everyday",
        3 => "Wider",
        _ => "Full",
    }
}

pub fn in_skill(word: &Word, skill: u8) -> bool {
    word.level <= skill.clamp(1, 4)
}

pub struct Category {
    pub id: &'static str,
    pub label: &'static str,
    pub words: &'static [&'static str],
    pub icon: &'static str,
    pub home: bool,
    pub bar: bool,
    pub tone: Option<Role>,
}

pub struct Follow {
    pub id: &'static str,
    /// Offer this word even when it is already in the sentence.
    pub always: bool,
}

pub fn words() -> &'static [Word] {
    crate::pack::words()
}

pub fn word_by_id(id: &str) -> Option<&'static Word> {
    words().iter().find(|word| word.id == id)
}

pub fn categories() -> &'static [Category] {
    crate::pack::categories()
}

pub fn category(id: &str) -> Option<&'static Category> {
    categories().iter().find(|cat| cat.id == id)
}

/// Words a parent can show or hide, in category order.
pub fn setup_words(skill: u8) -> Vec<&'static Word> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for cat in categories() {
        for id in cat.words {
            if seen.insert(*id) {
                if let Some(word) = word_by_id(id) {
                    if in_skill(word, skill) {
                        out.push(word);
                    }
                }
            }
        }
    }
    out
}

pub fn allows_repeat(word: &Word) -> bool {
    word.role == Role::Polite || word.repeat
}

/// What espeak should say. Pause is a comma so the line rests instead of saying the word.
pub fn speech_piece(word: &Word) -> &'static str {
    if word.id == "pause" { "," } else { word.say }
}

/// What the sentence line shows. The Pause button stays labeled Pause.
pub fn display_piece(word: &Word) -> &'static str {
    if word.id == "pause" { "<-->" } else { word.say }
}

pub fn speed_label(speed: u8) -> &'static str {
    match speed.clamp(1, 4) {
        1 => "Slower",
        2 => "Usual",
        3 => "Faster",
        _ => "Fastest",
    }
}

pub fn scaled_wpm(board: Board, speed: u8) -> i32 {
    let scale = match speed.clamp(1, 4) {
        1 => 0.75,
        2 => 1.0,
        3 => 1.25,
        _ => 1.5,
    };
    (board.speech_wpm() as f64 * scale).round() as i32
}

/// A lift for words the authored links never mention, when they fit the last word.
/// Linked words keep the scores the checks already rely on.
pub fn tail_bonus(last: Option<&Word>, word: &Word, authored: f64, learned: f64) -> f64 {
    let Some(prev) = last else {
        return 0.0;
    };
    if authored > 0.0 || learned > 0.0 || prev.id == word.id {
        return 0.0;
    }
    let mut bonus = 0.0;
    if shares_category(prev.id, word.id) {
        bonus += 2.6;
    }
    if favored_group(prev.id, word.id) {
        bonus += 2.4;
    }
    bonus
}

fn shares_category(left: &str, right: &str) -> bool {
    categories().iter().any(|cat| {
        cat.words.contains(&left) && cat.words.contains(&right)
    })
}

fn favored_group(verb: &str, word_id: &str) -> bool {
    let groups: &[&str] = match verb {
        "want" | "need" | "like" | "get" => &["food", "things", "clothes"],
        "eat" | "drink" => &["food"],
        "go" => &["places"],
        "see" | "look" => &["things", "animals", "people", "places", "weather"],
        "play" => &["things"],
        _ => &[],
    };
    categories()
        .iter()
        .any(|cat| groups.contains(&cat.id) && cat.words.contains(&word_id))
}

/// Extra pull for and/or after a thing, so "bread and milk" is easy to reach.
pub fn join_bonus(last: Option<&Word>, word: &Word) -> f64 {
    if word.repeat && last.is_some_and(|prev| prev.role == Role::Object) {
        3.2
    } else {
        0.0
    }
}

/// The bottom row. Yes, no, and, and or ship here. A parent can add more in a bar category.
pub fn bar_words() -> Vec<&'static Word> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for cat in categories() {
        if !cat.bar {
            continue;
        }
        for id in cat.words {
            if seen.insert(*id) {
                if let Some(word) = word_by_id(id) {
                    out.push(word);
                }
            }
        }
    }
    out
}


pub(crate) fn expect_weights(role: Option<Role>) -> [f64; 7] {
    match role {
        None => [2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.6],
        Some(Role::Person) => [0.0, 2.0, 0.0, 1.2, 0.0, 0.0, 0.0],
        Some(Role::Verb) => [0.0, 0.0, 2.0, 1.1, 1.2, 0.0, 0.4],
        Some(Role::Object) => [0.0, 0.0, 0.0, 0.0, 0.0, 1.6, 0.4],
        Some(Role::Feeling) => [0.0, 0.6, 0.0, 0.0, 0.0, 0.8, 0.0],
        Some(Role::Place) => [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        Some(Role::Polite) => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4],
        Some(Role::Social) => [1.0, 1.0, 0.8, 0.0, 0.0, 0.0, 0.0],
    }
}

/// A First-messages press. The button stays one picture. The spoken line is the
/// whole request a beginner is trying to make.
pub fn first_phrase(id: &str) -> Vec<&'static str> {
    match word_by_id(id) {
        Some(word) if !word.phrase.is_empty() => word.phrase.to_vec(),
        Some(word) => vec![word.id],
        None => Vec::new(),
    }
}

/// A line can stand on its own: it ends on a thing, a place, a feeling, or a closer.
/// "I" and "I want" are still being built.
pub fn line_complete(ids: &[&str]) -> bool {
    let Some(last) = ids.last().copied().and_then(word_by_id) else {
        return false;
    };
    match last.role {
        Role::Person | Role::Verb => last.closes,
        _ => true,
    }
}

/// What a person often says after a finished line. Please, thanks, more, and
/// all done stay in that order so the eyes can learn the places.
pub fn follow_ups<'a>(
    ids: &[&'a str],
    limit: usize,
    hidden: &BTreeSet<String>,
    skill: u8,
) -> Vec<&'static Word> {
    let mut out: Vec<&Word> = Vec::new();
    for item in crate::pack::follow() {
        if hidden.contains(item.id) {
            continue;
        }
        if !item.always && ids.contains(&item.id) {
            continue;
        }
        if let Some(word) = word_by_id(item.id) {
            if !in_skill(word, skill) {
                continue;
            }
            out.push(word);
        }
    }
    for word in recommend(ids, limit + 4, hidden, skill) {
        if out.iter().any(|have| have.id == word.id) {
            continue;
        }
        if ids.contains(&word.id) && !allows_repeat(word) {
            continue;
        }
        out.push(word);
        if out.len() >= limit {
            break;
        }
    }
    out.truncate(limit);
    out
}

/// Parent-facing count. The board never changes stage by itself.
pub fn progress_note(board: Board, turns: u32, skill: u8) -> String {
    let noun = if turns == 1 { "message" } else { "messages" };
    let mut text = format!("{turns} {noun} on this stage. {} words.", skill_label(skill));
    let ready = match board {
        Board::Big if turns >= 12 => Some("Ready for Growing phrases when you choose it."),
        Board::Medium if turns >= 24 => Some("Ready for Conversation when you choose it."),
        _ => None,
    };
    if let Some(ready) = ready {
        text.push(' ');
        text.push_str(ready);
    }
    text
}

/// Authored weight from one word to the next. Zero when the link was not written.
pub fn link_weight(from: &Word, to_id: &str) -> f64 {
    from.next
        .iter()
        .find(|(id, _)| *id == to_id)
        .map(|(_, weight)| *weight)
        .unwrap_or(0.0)
}

/// A softer link from the word before last, so the guess sees more than one step.
pub fn earlier_link(ids: &[&str], to_id: &str) -> f64 {
    if ids.len() < 2 {
        return 0.0;
    }
    word_by_id(ids[ids.len() - 2])
        .map(|word| link_weight(word, to_id))
        .unwrap_or(0.0)
}

/// Words that share a category with something already in the line get a small lift.
pub fn topic_bonus(ids: &[&str], to_id: &str) -> f64 {
    if ids.is_empty() {
        return 0.0;
    }
    let shares = categories().iter().any(|cat| {
        cat.words.contains(&to_id)
            && ids
                .iter()
                .any(|id| *id != to_id && cat.words.contains(id))
    });
    if shares { 0.45 } else { 0.0 }
}

/// Next words for a line of ids. Hidden words stay off the suggestion row.
pub fn recommend<'a>(
    ids: &[&'a str],
    limit: usize,
    hidden: &BTreeSet<String>,
    skill: u8,
) -> Vec<&'static Word> {
    let last = ids.last().and_then(|id| word_by_id(id));
    let expect = expect_weights(last.map(|word| word.role));
    let edge_weights: HashMap<&str, f64> = last
        .map(|word| word.next.iter().copied().collect())
        .unwrap_or_default();
    let used: BTreeSet<&str> = ids.iter().copied().collect();
    let mut scored: Vec<(&Word, f64)> = Vec::new();
    for word in words() {
        if hidden.contains(word.id) || !in_skill(word, skill) {
            continue;
        }
        if used.contains(word.id) && !allows_repeat(word) {
            continue;
        }
        let edge = edge_weights.get(word.id).copied().unwrap_or(0.0);
        let mut dot = 0.0;
        for (index, weight) in expect.iter().enumerate() {
            dot += weight * word.vec[index];
        }
        let score = edge * 2.0
            + earlier_link(ids, word.id) * 0.8
            + topic_bonus(ids, word.id)
            + join_bonus(last, word)
            + dot
            + if last.is_some() { 0.0 } else { word.starter };
        if score <= 0.0 {
            continue;
        }
        scored.push((word, score));
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(b.0.id)));
    scored
        .into_iter()
        .take(limit)
        .map(|(word, _)| word)
        .collect()
}

/// How long a look must stay before it counts, in milliseconds.
/// Menu, categories, and sentence tools wait longer than a picture.
/// The parent control waits twice the menu time.
pub fn dwell_ms(mode_is_setup: bool, board: Board, kind: DwellKind) -> u64 {
    if mode_is_setup && matches!(kind, DwellKind::Word | DwellKind::Choice) {
        return 1300;
    }
    match kind {
        DwellKind::Parent => 2600,
        DwellKind::Steady => 800,
        DwellKind::Word => match board {
            Board::Big => 700,
            Board::Medium | Board::Predict => 900,
            Board::Sentences => 1100,
        },
        DwellKind::Menu | DwellKind::Choice => 1300,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwellKind {
    Word,
    Menu,
    Steady,
    Parent,
    Choice,
}
