//! Calm device chrome. Category color follows the Fitzgerald key.

use gtk4::cairo::{Context, FontSlant, FontWeight};

use say::board::{self, Role, Theme};
use say::layout::{Cell, Place, Rect, Scene};
use say::session::Action;

use crate::icons;

const INK: (f64, f64, f64) = (0.12, 0.14, 0.16);
const PAPER: (f64, f64, f64) = (0.953, 0.945, 0.929);
const CARD: (f64, f64, f64) = (1.0, 1.0, 1.0);
const LINE: (f64, f64, f64) = (0.82, 0.80, 0.76);
const TEAL: (f64, f64, f64) = (0.06, 0.36, 0.36);
const PALE: (f64, f64, f64) = (0.86, 0.93, 0.92);
const YES: (f64, f64, f64) = (0.78, 0.90, 0.80);
const NO: (f64, f64, f64) = (0.96, 0.82, 0.82);
const MUTED: (f64, f64, f64) = (0.38, 0.40, 0.42);
const PEOPLE: (f64, f64, f64) = (0.965, 0.886, 0.478);
const VERBS: (f64, f64, f64) = (0.659, 0.816, 0.553);
const THINGS: (f64, f64, f64) = (0.957, 0.698, 0.455);
const FEEL: (f64, f64, f64) = (0.957, 0.761, 0.843);
const PLACES: (f64, f64, f64) = (0.718, 0.831, 0.941);
const SOCIAL: (f64, f64, f64) = (0.831, 0.769, 0.941);

pub fn paint(
    cr: &Context,
    scene: &Scene,
    progress: f64,
    armed: Option<Action>,
    gaze: Option<(f64, f64)>,
    _width: f64,
    _height: f64,
) {
    if scene.theme == Theme::Future {
        cr.set_source_rgb(0.03, 0.05, 0.09);
        cr.paint().ok();
        for index in 0..56 {
            let x = ((index * 97) % 2000) as f64 / 2000.0 * _width;
            let y = ((index * 53) % 1100) as f64 / 1100.0 * _height;
            let radius = if index % 5 == 0 { 1.6 } else { 1.0 };
            cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(0.85, 0.92, 1.0, if index % 3 == 0 { 0.7 } else { 0.35 });
            cr.fill().ok();
        }
    } else {
        rgb(cr, PAPER);
        cr.paint().ok();
    }

    if let Some((rect, title, body)) = &scene.banner {
        card(cr, *rect, 16.0);
        center_text(cr, title, rect.x + 16.0, rect.y + 26.0, 28.0, INK, false);
        cr.select_font_face("Noto Sans", FontSlant::Normal, FontWeight::Normal);
        cr.set_font_size(15.0);
        rgb(cr, MUTED);
        cr.move_to(rect.x + 16.0, rect.y + 52.0);
        cr.show_text("Stage and words. A short look does not open this.").ok();
        cr.move_to(rect.x + 16.0, rect.y + 76.0);
        cr.show_text(body).ok();
    }
    if let Some((rect, text)) = &scene.heard {
        card(cr, *rect, 14.0);
        if let Some(earlier) = &scene.earlier {
            cr.select_font_face("Noto Sans", FontSlant::Normal, FontWeight::Normal);
            cr.set_font_size(15.0);
            rgb(cr, MUTED);
            cr.move_to(rect.x + 16.0, rect.y + 22.0);
            cr.show_text(earlier).ok();
        }
        let cy = if scene.earlier.is_some() {
            rect.y + rect.h - 24.0
        } else {
            rect.y + rect.h / 2.0
        };
        center_text(cr, text, rect.x + rect.w / 2.0, cy, 26.0, INK, true);
    }
    if let Some(rect) = scene.next_bar {
        card(cr, rect, 14.0);
        cr.select_font_face("Noto Sans", FontSlant::Normal, FontWeight::Bold);
        cr.set_font_size(13.0);
        rgb(cr, MUTED);
        let caption = if scene.next_caption.is_empty() {
            "Next"
        } else {
            scene.next_caption.as_str()
        };
        cr.move_to(rect.x + 12.0, rect.y + rect.h / 2.0 + 5.0);
        cr.show_text(caption).ok();
    }

    for cell in &scene.cells {
        let armed_here = armed == Some(cell.action);
        draw_cell(
            cr,
            cell,
            if armed_here { progress } else { 0.0 },
            scene.theme,
        );
    }

    if scene.eyes {
        if let Some(parent) = scene
            .cells
            .iter()
            .find(|cell| cell.action == Action::Parent)
        {
            let chip = Rect {
                x: 156.0,
                y: 30.0,
                w: 72.0,
                h: 36.0,
            };
            if chip.x + chip.w < parent.rect.x - 8.0 {
                card(cr, chip, 18.0);
                center_text(
                    cr,
                    "Eyes",
                    chip.x + chip.w / 2.0,
                    chip.y + chip.h / 2.0,
                    16.0,
                    INK,
                    true,
                );
            }
        }
    }

    if let Some((x, y)) = gaze {
        ring(cr, x, y, progress);
    }
}

fn draw_cell(cr: &Context, cell: &Cell, progress: f64, theme: Theme) {
    let rect = cell.rect;
    let fill = fill_for(cell);
    let radius = if theme == Theme::Future { 18.0 } else { 14.0 };
    let lift = if theme == Theme::Future {
        -8.0 * progress
    } else {
        0.0
    };
    cr.save().ok();
    if theme == Theme::Future {
        cr.save().ok();
        round_rect(cr, rect.x, rect.y + 8.0, rect.w, rect.h, radius);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
        cr.fill().ok();
        cr.restore().ok();
    }
    cr.translate(0.0, lift);
    round_rect(cr, rect.x, rect.y, rect.w, rect.h, radius);
    if theme == Theme::Future {
        cr.set_source_rgba(fill.0, fill.1, fill.2, 0.82);
    } else {
        rgb(cr, fill);
    }
    cr.fill().ok();
    if theme == Theme::Future {
        cr.save().ok();
        round_rect(cr, rect.x, rect.y, rect.w, rect.h * 0.46, radius);
        cr.clip();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.28);
        cr.paint().ok();
        cr.restore().ok();
        if progress > 0.02 {
            round_rect(cr, rect.x - 2.0, rect.y - 2.0, rect.w + 4.0, rect.h + 4.0, radius + 2.0);
            cr.set_line_width(2.0 + progress * 4.0);
            cr.set_source_rgba(0.45, 0.85, 1.0, 0.25 + progress * 0.65);
            cr.stroke().ok();
        }
    }
    round_rect(cr, rect.x + 0.5, rect.y + 0.5, rect.w - 1.0, rect.h - 1.0, radius);
    cr.set_line_width(1.0);
    rgb(cr, if cell.selected { TEAL } else { LINE });
    if cell.selected {
        cr.set_line_width(2.5);
    }
    cr.stroke().ok();
    if progress > 0.02 {
        cr.save().ok();
        round_rect(cr, rect.x, rect.y, rect.w, rect.h, radius);
        cr.clip();
        let bar = (rect.h - 10.0) * progress;
        cr.rectangle(rect.x, rect.y + rect.h - bar, rect.w, bar);
        cr.set_source_rgba(TEAL.0, TEAL.1, TEAL.2, 0.28);
        cr.fill().ok();
        cr.restore().ok();
    }
    let faded = cell.place == Place::Setup
        && !cell.selected
        && matches!(cell.action, Action::ToggleWord(_));
    if faded {
        cr.set_source_rgba(0.4, 0.32, 0.24, 0.45);
    } else {
        rgb(cr, INK);
    }
    if cell.action == Action::Mute {
        icons::speaker(
            cr,
            cell.title == "Sound",
            rect.x + (rect.w - 34.0) / 2.0,
            rect.y + (rect.h - 34.0) / 2.0,
            34.0,
        );
    } else if let Some(icon) = cell.icon {
        let size = if cell.place == Place::Board {
            cell.icon_px.max((rect.h * 0.34).min(220.0))
        } else {
            cell.icon_px
        };
        let block = size + 12.0 + cell.label_px;
        let top = rect.y + ((rect.h - block) / 2.0).max(6.0);
        icons::draw_file(cr, icon, rect.x + (rect.w - size) / 2.0, top, size);
        center_text(
            cr,
            &cell.title,
            rect.x + rect.w / 2.0,
            top + size + 12.0 + cell.label_px / 2.0,
            cell.label_px,
            if faded { MUTED } else { INK },
            true,
        );
    } else if matches!(cell.action, Action::ChooseBoard(_)) {
        center_text(
            cr,
            &cell.title,
            rect.x + 18.0,
            rect.y + 26.0,
            cell.label_px,
            INK,
            false,
        );
        let blurb = match cell.action {
            Action::ChooseBoard(board) => board.blurb(),
            _ => "",
        };
        cr.set_font_size(14.0);
        rgb(cr, MUTED);
        cr.move_to(rect.x + 18.0, rect.y + 48.0);
        cr.show_text(blurb).ok();
    } else {
        center_text(
            cr,
            &cell.title,
            rect.x + rect.w / 2.0,
            rect.y + rect.h / 2.0,
            cell.label_px,
            INK,
            true,
        );
    }
    cr.restore().ok();
}

fn fill_for(cell: &Cell) -> (f64, f64, f64) {
    match cell.action {
        Action::Word("yes") => YES,
        Action::Word("no") => NO,
        Action::Word("and") => (0.85, 0.90, 0.96),
        Action::Word("or") => (0.96, 0.91, 0.80),
        Action::Word("pause") => (0.90, 0.90, 0.92),
        Action::SayAll
        | Action::Hear
        | Action::Delete
        | Action::Clear
        | Action::Done
        | Action::OtherWords
        | Action::MoreWords => PALE,
        Action::ChooseBoard(_)
        | Action::ChooseSkill(_)
        | Action::ChooseTheme(_)
        | Action::ChooseEar(_)
        | Action::ChooseRoom(_)
        | Action::ChooseVoice(_)
        | Action::ChooseSpeed(_)
        | Action::ChooseEngine(_)
            if cell.selected =>
        {
            PALE
        }
        Action::ToggleWord(_) if !cell.selected => (0.93, 0.92, 0.90),
        Action::Word(id) | Action::Category(id) | Action::ToggleWord(id) => {
            tone(id).unwrap_or(CARD)
        }
        _ => CARD,
    }
}

fn tone(id: &str) -> Option<(f64, f64, f64)> {
    if let Some(word) = board::word_by_id(id) {
        return Some(role_color(word.role));
    }
    board::category(id).and_then(|cat| cat.tone).map(role_color)
}

fn role_color(role: Role) -> (f64, f64, f64) {
    match role {
        Role::Person => PEOPLE,
        Role::Verb => VERBS,
        Role::Object => THINGS,
        Role::Feeling => FEEL,
        Role::Place => PLACES,
        Role::Polite | Role::Social => SOCIAL,
    }
}

fn ring(cr: &Context, x: f64, y: f64, progress: f64) {
    cr.save().ok();
    cr.set_line_width(2.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.arc(x, y, 16.0, 0.0, std::f64::consts::TAU);
    cr.stroke().ok();
    cr.set_line_width(3.0);
    rgb(cr, TEAL);
    let start = -std::f64::consts::FRAC_PI_2;
    cr.arc(
        x,
        y,
        16.0,
        start,
        start + std::f64::consts::TAU * progress.clamp(0.0, 1.0),
    );
    cr.stroke().ok();
    cr.restore().ok();
}

fn card(cr: &Context, rect: Rect, radius: f64) {
    round_rect(cr, rect.x, rect.y, rect.w, rect.h, radius);
    rgb(cr, CARD);
    cr.fill().ok();
    round_rect(cr, rect.x + 0.5, rect.y + 0.5, rect.w - 1.0, rect.h - 1.0, radius);
    cr.set_line_width(1.0);
    rgb(cr, LINE);
    cr.stroke().ok();
}

fn round_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
    cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
    cr.arc(x + r, y + r, r, pi, pi * 1.5);
    cr.close_path();
}

fn center_text(
    cr: &Context,
    text: &str,
    cx: f64,
    cy: f64,
    size: f64,
    color: (f64, f64, f64),
    centered: bool,
) {
    cr.select_font_face("Noto Sans", FontSlant::Normal, FontWeight::Bold);
    cr.set_font_size(size);
    rgb(cr, color);
    let Ok(extents) = cr.text_extents(text) else {
        return;
    };
    let x = if centered {
        cx - extents.width() / 2.0 - extents.x_bearing()
    } else {
        cx
    };
    let y = cy - extents.height() / 2.0 - extents.y_bearing();
    cr.move_to(x, y);
    cr.show_text(text).ok();
}

fn rgb(cr: &Context, color: (f64, f64, f64)) {
    cr.set_source_rgb(color.0, color.1, color.2);
}
