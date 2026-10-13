//! Places every picture and control. Geometry stays here so a test can
//! check cell size without opening a window.

use crate::board::{self, Board};
use crate::predict;
use crate::session::{Action, Mode, View};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Chrome,
    Board,
    YesNo,
    Next,
    Setup,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub rect: Rect,
    pub action: Action,
    pub title: String,
    pub icon: Option<&'static str>,
    pub place: Place,
    pub selected: bool,
    pub label_px: f64,
    pub icon_px: f64,
}

#[derive(Clone, Debug)]
pub struct SentenceBar {
    pub rect: Rect,
    pub words: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub cells: Vec<Cell>,
    pub heard: Option<(Rect, String)>,
    pub earlier: Option<String>,
    pub next_caption: String,
    pub sentence: Option<SentenceBar>,
    pub next_bar: Option<Rect>,
    pub banner: Option<(Rect, String, String)>,
    pub eyes: bool,
    pub theme: crate::board::Theme,
}

const TOP: f64 = 86.0;
const PAD: f64 = 10.0;
const GAP: f64 = 10.0;

pub fn layout(view: &View<'_>, width: f64, height: f64) -> Scene {
    let mut scene = Scene {
        cells: Vec::new(),
        heard: None,
        earlier: None,
        next_caption: String::new(),
        sentence: None,
        next_bar: None,
        banner: None,
        eyes: view.eyes,
        theme: view.theme,
    };
    if width < 40.0 || height < 40.0 {
        return scene;
    }
    chrome(&mut scene, view, width);
    match view.mode {
        Mode::User => user_board(&mut scene, view, width, height),
        Mode::Setup => setup_board(&mut scene, view, width, height),
    }
    scene
}

fn chrome(scene: &mut Scene, view: &View<'_>, width: f64) {
    let y = 18.0;
    let h = 64.0;
    scene.cells.push(button(
        Rect {
            x: PAD,
            y,
            w: 64.0,
            h,
        },
        Action::Mute,
        if view.muted { "Sound" } else { "Mute" },
        None,
        Place::Chrome,
        false,
        14.0,
        0.0,
    ));
    scene.cells.push(button(
        Rect {
            x: 82.0,
            y,
            w: 64.0,
            h,
        },
        Action::Fullscreen,
        "Full",
        None,
        Place::Chrome,
        false,
        16.0,
        0.0,
    ));

    let mut right = width - PAD;
    let show_back = view.mode == Mode::User
        && view.board != Board::Sentences
        && (view.page.is_some() || view.picking);
    if show_back {
        let w = 148.0;
        right -= w;
        scene.cells.push(button(
            Rect { x: right, y, w, h },
            Action::Back,
            "Back",
            None,
            Place::Chrome,
            false,
            18.0,
            0.0,
        ));
        right -= GAP;
    }
    if view.mode == Mode::User {
        let w = 132.0;
        right -= w;
        scene.cells.push(button(
            Rect { x: right, y, w, h },
            Action::Parent,
            "Setup",
            None,
            Place::Chrome,
            false,
            18.0,
            0.0,
        ));
    }
}

fn user_board(scene: &mut Scene, view: &View<'_>, width: f64, height: f64) {
    let mut y = TOP;
    let show_message = view.board == Board::Sentences
        || view.board == Board::Predict
        || !view.sentence.is_empty();
    if show_message {
        let h = if view.earlier.is_some() { 78.0 } else { 58.0 };
        let rect = Rect {
            x: PAD,
            y,
            w: width - PAD * 2.0,
            h,
        };
        let line = if view.sentence.is_empty() {
            "The line stays here while you talk.".to_string()
        } else {
            view.sentence.join(" ")
        };
        scene.heard = Some((rect, line));
        scene.earlier = view.earlier.map(str::to_string);
        y += h + GAP;
    }
    if !view.next_ids.is_empty() {
        let next_rect = Rect {
            x: PAD,
            y,
            w: width - PAD * 2.0,
            h: 72.0,
        };
        scene.next_caption = "Next".to_string();
        place_next(scene, view, next_rect);
        y += 72.0 + GAP;
    }

    let tools_h = 56.0;
    let bar: Vec<_> = board::bar_words()
        .into_iter()
        .filter(|word| shown(view, word))
        .collect();
    let yes_h = if view.board == Board::Sentences || bar.is_empty() {
        0.0
    } else {
        (height * 0.16).clamp(84.0, 160.0)
    };
    let between = if tools_h > 0.0 && yes_h > 0.0 { GAP } else { 0.0 };
    let stack = tools_h + yes_h + between;
    let bottom = height - PAD - stack - if stack > 0.0 { GAP } else { 0.0 };
    let board_rect = Rect {
        x: PAD,
        y,
        w: (width - PAD * 2.0).max(1.0),
        h: (bottom - y).max(1.0),
    };

    let (min_w, min_h, label, icon) = match view.board {
        Board::Big => {
            let h = if width < 800.0 { 120.0 } else { 148.0 };
            (168.0, h, 22.0, 54.0)
        }
        Board::Medium => (120.0, 108.0, 16.0, 48.0),
        Board::Sentences => (96.0, 84.0, 14.0, 36.0),
        Board::Predict => (150.0, 120.0, 18.0, 48.0),
    };

    if view.board == Board::Predict && view.page.is_none() && !view.picking {
        let on_bar: std::collections::BTreeSet<&str> = board::bar_words()
            .into_iter()
            .map(|word| word.id)
            .collect();
        let take = if view.predict_more { 24 } else { 8 };
        let cell_w = if view.predict_more { 110.0 } else { min_w };
        let cell_h = if view.predict_more { 84.0 } else { min_h };
        let llama = if view.engine == board::Engine::Llama && view.llama_hit {
            Some(view.llama_ids)
        } else {
            None
        };
        let guessed: Vec<_> = predict::mix(&view.ids, 40, view.hidden, view.learned, view.skill, llama)
            .into_iter()
            .filter(|word| !on_bar.contains(word.id))
            .take(take)
            .collect();
        let mut rects = fit_grid(
            board_rect,
            guessed.len() + 2,
            cell_w,
            cell_h,
            GAP,
            true,
            !view.predict_more,
        );
        let other = rects.pop();
        let more = rects.pop();
        for (word, rect) in guessed.into_iter().zip(rects) {
            scene.cells.push(button(
                rect,
                Action::Word(word.id),
                word.say,
                icon_of(word.icon),
                Place::Board,
                false,
                if view.predict_more { 14.0 } else { label },
                if view.predict_more { 32.0 } else { icon },
            ));
        }
        if let Some(rect) = more {
            scene.cells.push(button(
                rect,
                Action::MoreWords,
                if view.predict_more { "Fewer words" } else { "More words" },
                None,
                Place::Board,
                false,
                if view.predict_more { 14.0 } else { label },
                0.0,
            ));
        }
        if let Some(rect) = other {
            scene.cells.push(button(
                rect,
                Action::OtherWords,
                "Other words",
                None,
                Place::Board,
                false,
                if view.predict_more { 14.0 } else { label },
                0.0,
            ));
        }
    } else if view.board == Board::Sentences {
        let words: Vec<_> = board::words()
            .iter()
            .filter(|word| shown(view, word))
            .collect();
        let rects = fit_grid(board_rect, words.len(), min_w, min_h, GAP, true, false);
        for (word, rect) in words.into_iter().zip(rects) {
            scene.cells.push(button(
                rect,
                Action::Word(word.id),
                word.say,
                icon_of(word.icon),
                Place::Board,
                false,
                label,
                icon,
            ));
        }
    } else if view.page.is_none() {
        let cats: Vec<_> = board::categories()
            .iter()
            .filter(|cat| cat.home)
            .filter(|cat| {
                cat.words.iter().any(|id| {
                    board::word_by_id(id).is_some_and(|word| shown(view, word))
                })
            })
            .collect();
        let rects = fit_grid(board_rect, cats.len(), min_w, min_h, GAP, true, true);
        for (cat, rect) in cats.into_iter().zip(rects) {
            scene.cells.push(button(
                rect,
                Action::Category(cat.id),
                cat.label,
                icon_of(cat.icon),
                Place::Board,
                false,
                label,
                icon,
            ));
        }
    } else if let Some(cat) = view.page.and_then(board::category) {
        let words: Vec<_> = cat
            .words
            .iter()
            .filter_map(|id| board::word_by_id(id))
            .filter(|word| shown(view, word))
            .collect();
        let rects = fit_grid(board_rect, words.len(), min_w, min_h, GAP, true, true);
        for (word, rect) in words.into_iter().zip(rects) {
            scene.cells.push(button(
                rect,
                Action::Word(word.id),
                word.say,
                icon_of(word.icon),
                Place::Board,
                false,
                label,
                icon,
            ));
        }
    }

    let y = height - PAD - 56.0
        - if view.board == Board::Sentences || yes_h <= 0.0 {
            0.0
        } else {
            yes_h + GAP
        };
    let w = (width - PAD * 2.0 - GAP * 3.0) / 4.0;
    for (index, (action, title)) in [
        (Action::Hear, "Hear it"),
        (Action::SayAll, "Out loud"),
        (Action::Delete, "Back up"),
        (Action::Clear, "Clear"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect {
            x: PAD + index as f64 * (w + GAP),
            y,
            w,
            h: 56.0,
        };
        scene.cells.push(button(
            rect,
            action,
            title,
            None,
            Place::Chrome,
            false,
            16.0,
            0.0,
        ));
    }

    if yes_h > 0.0 {
        let count = bar.len() as f64;
        let gaps = GAP * (count - 1.0).max(0.0);
        let cell_w = ((width - PAD * 2.0 - gaps) / count).max(1.0);
        for (index, word) in bar.iter().enumerate() {
            scene.cells.push(button(
                Rect {
                    x: PAD + index as f64 * (cell_w + GAP),
                    y: height - PAD - yes_h,
                    w: cell_w,
                    h: yes_h,
                },
                Action::Word(word.id),
                word.say,
                icon_of(word.icon),
                Place::YesNo,
                false,
                28.0,
                36.0,
            ));
        }
    }
}

fn place_next(scene: &mut Scene, view: &View<'_>, rect: Rect) {
    scene.next_bar = Some(rect);
    let label_w = 52.0;
    let count = view.next_ids.len().max(1);
    let avail = (rect.w - label_w - GAP).max(1.0);
    let cell_w =
        ((avail - GAP * (count.saturating_sub(1) as f64)) / count as f64).clamp(64.0, 140.0);
    for (index, id) in view.next_ids.iter().enumerate() {
        let Some(word) = board::word_by_id(id) else {
            continue;
        };
        let x = rect.x + label_w + GAP + index as f64 * (cell_w + GAP);
        if x + cell_w > rect.x + rect.w + 1.0 {
            break;
        }
        scene.cells.push(button(
            Rect {
                x,
                y: rect.y + 4.0,
                w: cell_w,
                h: rect.h - 8.0,
            },
            Action::Word(word.id),
            word.say,
            icon_of(word.icon),
            Place::Next,
            false,
            14.0,
            28.0,
        ));
    }
}

fn setup_board(scene: &mut Scene, view: &View<'_>, width: f64, height: f64) {
    let banner = Rect {
        x: PAD,
        y: TOP,
        w: width - PAD * 2.0,
        h: 96.0,
    };
    scene.banner = Some((
        banner,
        "Setup".to_string(),
        view.note.clone(),
    ));
    let mut y = TOP + 96.0 + GAP;
    for board in [Board::Big, Board::Medium, Board::Sentences, Board::Predict] {
        let rect = Rect {
            x: PAD,
            y,
            w: width - PAD * 2.0,
            h: 72.0,
        };
        scene.cells.push(button(
            rect,
            Action::ChooseBoard(board),
            board.label(),
            None,
            Place::Setup,
            view.board == board,
            20.0,
            0.0,
        ));
        y += 72.0 + 8.0;
    }
    let skill_w = (width - PAD * 2.0 - GAP * 3.0) / 4.0;
    for (index, level) in [1u8, 2, 3, 4].into_iter().enumerate() {
        scene.cells.push(button(
            Rect {
                x: PAD + index as f64 * (skill_w + GAP),
                y,
                w: skill_w,
                h: 52.0,
            },
            Action::ChooseSkill(level),
            board::skill_label(level),
            None,
            Place::Setup,
            view.skill == level,
            16.0,
            0.0,
        ));
    }
    y += 52.0 + 8.0;
    let theme_w = (width - PAD * 2.0 - GAP) / 2.0;
    for (index, theme) in [crate::board::Theme::Simple, crate::board::Theme::Future]
        .into_iter()
        .enumerate()
    {
        scene.cells.push(button(
            Rect {
                x: PAD + index as f64 * (theme_w + GAP),
                y,
                w: theme_w,
                h: 48.0,
            },
            Action::ChooseTheme(theme),
            theme.label(),
            None,
            Place::Setup,
            view.theme == theme,
            16.0,
            0.0,
        ));
    }
    y += 48.0 + 8.0;
    let speed_w = (width - PAD * 2.0 - GAP * 3.0) / 4.0;
    for (index, speed) in [1u8, 2, 3, 4].into_iter().enumerate() {
        scene.cells.push(button(
            Rect {
                x: PAD + index as f64 * (speed_w + GAP),
                y,
                w: speed_w,
                h: 44.0,
            },
            Action::ChooseSpeed(speed),
            board::speed_label(speed),
            None,
            Place::Setup,
            view.speed == speed,
            15.0,
            0.0,
        ));
    }
    y += 44.0 + 8.0;
    let engine_w = (width - PAD * 2.0 - GAP) / 2.0;
    for (index, engine) in [board::Engine::Graph, board::Engine::Llama]
        .into_iter()
        .enumerate()
    {
        scene.cells.push(button(
            Rect {
                x: PAD + index as f64 * (engine_w + GAP),
                y,
                w: engine_w,
                h: 44.0,
            },
            Action::ChooseEngine(engine),
            engine.label(),
            None,
            Place::Setup,
            view.engine == engine,
            16.0,
            0.0,
        ));
    }
    y += 44.0 + 8.0;
    let done_h = 76.0;
    let done = Rect {
        x: PAD,
        y: height - PAD - done_h,
        w: width - PAD * 2.0,
        h: done_h,
    };
    let audio_h = 52.0;
    let audio_top = done.y - GAP - audio_h * 2.0 - 8.0;
    let voice_h = 40.0;
    let voice_rows = (board::speech_voices().len() as f64 / 6.0).ceil().max(1.0);
    let voice_block = voice_rows * voice_h + (voice_rows - 1.0).max(0.0) * 8.0;
    let voice_top = audio_top - GAP - voice_block;
    let grid_bottom = voice_top;
    place_voices(scene, view, PAD, voice_top, width - PAD * 2.0, voice_h);
    place_speakers(scene, view, PAD, audio_top, width - PAD * 2.0, audio_h);
    let grid = Rect {
        x: PAD,
        y: y + 4.0,
        w: (width - PAD * 2.0).max(1.0),
        h: (grid_bottom - GAP - (y + 4.0)).max(1.0),
    };
    let words = board::setup_words(view.skill);
    if words.len() > 48 {
        if let Some(cat) = view.page.and_then(board::category) {
            let back = Rect {
                x: grid.x,
                y: grid.y,
                w: grid.w,
                h: 48.0,
            };
            scene.cells.push(button(
                back,
                Action::Back,
                "All groups",
                None,
                Place::Setup,
                false,
                18.0,
                0.0,
            ));
            let grid = Rect {
                x: grid.x,
                y: grid.y + 56.0,
                w: grid.w,
                h: (grid.h - 56.0).max(1.0),
            };
            let page_words: Vec<_> = cat
                .words
                .iter()
                .filter_map(|id| board::word_by_id(id))
                .filter(|word| board::in_skill(word, view.skill))
                .collect();
            place_toggles(scene, view, &page_words, grid);
        } else {
            let cats: Vec<_> = board::categories()
                .iter()
                .filter(|cat| {
                    cat.words.iter().any(|id| {
                        board::word_by_id(id)
                            .is_some_and(|word| board::in_skill(word, view.skill))
                    })
                })
                .collect();
            let rects = fit_grid(grid, cats.len(), 140.0, 86.0, 8.0, true, true);
            for (cat, rect) in cats.into_iter().zip(rects) {
                scene.cells.push(button(
                    rect,
                    Action::Category(cat.id),
                    cat.label,
                    icon_of(cat.icon),
                    Place::Setup,
                    false,
                    18.0,
                    36.0,
                ));
            }
        }
    } else {
        place_toggles(scene, view, &words, grid);
    }
    scene.cells.push(button(
        done,
        Action::Done,
        "Done. Open the board",
        None,
        Place::Setup,
        false,
        28.0,
        0.0,
    ));
}

fn place_voices(scene: &mut Scene, view: &View<'_>, x: f64, y: f64, width: f64, row_h: f64) {
    let voices = board::speech_voices();
    let columns = 6usize;
    let count = columns.min(voices.len()).max(1) as f64;
    let gaps = GAP * (count - 1.0);
    let cell_w = ((width - gaps) / count).max(1.0);
    for (index, voice) in voices.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        scene.cells.push(button(
            Rect {
                x: x + column as f64 * (cell_w + GAP),
                y: y + row as f64 * (row_h + 8.0),
                w: cell_w,
                h: row_h,
            },
            Action::ChooseVoice(index as u8),
            voice.label,
            None,
            Place::Setup,
            view.voice == voice.id,
            14.0,
            0.0,
        ));
    }
}

fn place_speakers(scene: &mut Scene, view: &View<'_>, x: f64, y: f64, width: f64, row_h: f64) {
    if view.outputs.is_empty() {
        scene.cells.push(button(
            Rect { x, y, w: width, h: row_h },
            Action::ChooseEar(0),
            "No speakers found",
            None,
            Place::Setup,
            false,
            16.0,
            0.0,
        ));
        return;
    }
    let count = view.outputs.len() as f64;
    let gaps = GAP * (count - 1.0).max(0.0);
    let cell_w = ((width - gaps) / count).max(1.0);
    for (index, (id, label)) in view.outputs.iter().enumerate() {
        let column = x + index as f64 * (cell_w + GAP);
        scene.cells.push(button(
            Rect { x: column, y, w: cell_w, h: row_h },
            Action::ChooseEar(index as u8),
            &format!("Hear: {label}"),
            None,
            Place::Setup,
            view.ear == id,
            15.0,
            0.0,
        ));
        scene.cells.push(button(
            Rect {
                x: column,
                y: y + row_h + 8.0,
                w: cell_w,
                h: row_h,
            },
            Action::ChooseRoom(index as u8),
            &format!("Out loud: {label}"),
            None,
            Place::Setup,
            view.room == id,
            15.0,
            0.0,
        ));
    }
}

fn shown(view: &View<'_>, word: &board::Word) -> bool {
    board::in_skill(word, view.skill) && !view.hidden.contains(word.id)
}

fn place_toggles(scene: &mut Scene, view: &View<'_>, words: &[&board::Word], grid: Rect) {
    let rects = fit_grid(grid, words.len(), 120.0, 86.0, 8.0, true, false);
    for (word, rect) in words.iter().zip(rects) {
        let hidden = view.hidden.contains(word.id);
        let title = if hidden {
            format!("{} off", word.say)
        } else {
            word.say.to_string()
        };
        scene.cells.push(Cell {
            rect,
            action: Action::ToggleWord(word.id),
            title,
            icon: icon_of(word.icon),
            place: Place::Setup,
            selected: !hidden,
            label_px: 14.0,
            icon_px: 32.0,
        });
    }
}

fn icon_of(path: &'static str) -> Option<&'static str> {
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn button(
    rect: Rect,
    action: Action,
    title: &str,
    icon: Option<&'static str>,
    place: Place,
    selected: bool,
    label_px: f64,
    icon_px: f64,
) -> Cell {
    Cell {
        rect,
        action,
        title: title.to_string(),
        icon,
        place,
        selected,
        label_px,
        icon_px,
    }
}

/// Pack `count` cells into `area`. Heights stay at `min_h` when they fit.
/// `shrink` lets a crowded page get smaller so every word stays on screen.
/// `grow` lets a few large pictures fill the board, which is easier to look at.
fn fit_grid(
    area: Rect,
    count: usize,
    min_w: f64,
    min_h: f64,
    gap: f64,
    shrink: bool,
    grow: bool,
) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let mut cell_w_floor = min_w;
    let mut cell_h = min_h;
    let mut cols = columns(area.w, cell_w_floor, gap).min(count).max(1);
    let mut rows = count.div_ceil(cols);
    if shrink {
        while rows as f64 * cell_h + (rows.saturating_sub(1) as f64) * gap > area.h && cell_h > 64.0
        {
            cell_h -= 4.0;
        }
        while rows as f64 * cell_h + (rows.saturating_sub(1) as f64) * gap > area.h
            && cell_w_floor > 88.0
        {
            cell_w_floor -= 8.0;
            cols = columns(area.w, cell_w_floor, gap).min(count).max(1);
            rows = count.div_ceil(cols);
        }
    }
    if grow {
        let slack = area.h - (rows.saturating_sub(1) as f64) * gap;
        let grown = slack / rows as f64;
        if grown > cell_h {
            cell_h = grown;
        }
    }
    let width = ((area.w - gap * (cols.saturating_sub(1) as f64)) / cols as f64).max(1.0);
    let mut rects = Vec::with_capacity(count);
    for index in 0..count {
        let col = index % cols;
        let row = index / cols;
        rects.push(Rect {
            x: area.x + col as f64 * (width + gap),
            y: area.y + row as f64 * (cell_h + gap),
            w: width,
            h: cell_h,
        });
    }
    rects
}

fn columns(width: f64, min_w: f64, gap: f64) -> usize {
    ((width + gap) / (min_w + gap)).floor().max(1.0) as usize
}

pub fn hit<'a>(cells: &'a [Cell], x: f64, y: f64) -> Option<&'a Cell> {
    cells.iter().rev().find(|cell| cell.rect.contains(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    #[test]
    fn setup_shows_both_speakers_by_name() {
        let mut session = Session::fresh();
        session.mode = Mode::Setup;
        session.outputs = vec![
            ("alsa".to_string(), "Built-in speakers".to_string()),
            ("bluez".to_string(), "Pixel Buds".to_string()),
        ];
        session.ear = "bluez".to_string();
        session.room = "alsa".to_string();
        let scene = layout(&session.view(false), 1200.0, 1000.0);
        let hear_buds = scene.cells.iter().any(|cell| {
            cell.action == Action::ChooseEar(1) && cell.title == "Hear: Pixel Buds" && cell.selected
        });
        let loud_speakers = scene.cells.iter().any(|cell| {
            cell.action == Action::ChooseRoom(0)
                && cell.title == "Out loud: Built-in speakers"
                && cell.selected
        });
        assert!(hear_buds);
        assert!(loud_speakers);
    }

    #[test]
    fn big_home_cells_stay_large() {
        let mut session = Session::fresh();
        session.board = Board::Big;
        let scene = layout(&session.view(false), 800.0, 700.0);
        let cell = scene
            .cells
            .iter()
            .find(|cell| cell.place == Place::Board)
            .unwrap();
        assert!(cell.rect.w >= 140.0, "{}", cell.rect.w);
        assert!(cell.rect.h >= 110.0, "{}", cell.rect.h);
        assert!(scene.cells.iter().any(|cell| cell.action == Action::Parent));
        assert!(scene.cells.iter().all(|cell| cell.action != Action::Back));
    }

    #[test]
    fn sentence_cells_stay_short() {
        let mut session = Session::fresh();
        session.board = Board::Sentences;
        let scene = layout(&session.view(false), 1000.0, 800.0);
        let cell = scene
            .cells
            .iter()
            .find(|cell| cell.place == Place::Board)
            .unwrap();
        assert!(cell.rect.h <= 120.0);
        assert!(cell.rect.h >= 70.0);
    }

    #[test]
    fn hidden_word_leaves_the_category() {
        let mut session = Session::fresh();
        session.board = Board::Big;
        session.page = Some("food");
        session.hidden.insert("cookie".to_string());
        let scene = layout(&session.view(false), 900.0, 700.0);
        assert!(scene
            .cells
            .iter()
            .all(|cell| cell.action != Action::Word("cookie")));
        assert!(scene
            .cells
            .iter()
            .any(|cell| cell.action == Action::Word("apple")));
    }
}
