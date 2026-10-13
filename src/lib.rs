//! Picture-talker rules shared by the window and by `say --selfcheck`.
//! The window is a separate binary so these tests do not need GTK.

pub mod board;
pub mod gaze;
pub mod layout;
pub mod pack;
pub mod predict;
pub mod session;
pub mod store;

use board::Board;
use layout::Place;
use session::{Action, Mode, Session};

/// Failures from the board checks, plus the parent gate.
pub struct Report {
    pub ok: bool,
    pub failures: Vec<String>,
}

impl Report {
    pub fn text(&self) -> String {
        if self.ok {
            return "SELFCHECK_OK".to_string();
        }
        let mut lines = vec!["SELFCHECK_FAIL".to_string()];
        lines.extend(self.failures.iter().cloned());
        lines.join("\n")
    }
}

pub fn self_check() -> Report {
    let mut failures = Vec::new();
    let fail = |failures: &mut Vec<String>, name: &str, detail: &str| {
        failures.push(format!("{name}: {detail}"));
    };

    let openers = board::recommend(&[], 4, &Default::default(), 1);
    let opener_ids: Vec<_> = openers.iter().map(|word| word.id).collect();
    if !opener_ids.contains(&"i") || !opener_ids.contains(&"you") {
        fail(
            &mut failures,
            "Case 1",
            &format!("openers were {}", opener_ids.join(",")),
        );
    }

    let after_i = board::recommend(&["i"], 4, &Default::default(), 1);
    if after_i
        .iter()
        .any(|word| !matches!(word.role, board::Role::Verb | board::Role::Feeling))
    {
        let ids: Vec<_> = after_i.iter().map(|word| word.id).collect();
        fail(
            &mut failures,
            "Case 2",
            &format!("after I came {}", ids.join(",")),
        );
    }
    if after_i.first().map(|word| word.id) != Some("want") {
        fail(
            &mut failures,
            "Case 2",
            &format!(
                "first after I was {}",
                after_i.first().map(|word| word.id).unwrap_or("none")
            ),
        );
    }

    let after_want = board::recommend(&["i", "want"], 5, &Default::default(), 1);
    if !matches!(
        after_want.first().map(|word| word.role),
        Some(board::Role::Object | board::Role::Social)
    ) {
        let ids: Vec<_> = after_want.iter().map(|word| word.id).collect();
        fail(
            &mut failures,
            "Case 3",
            &format!("after want came {}", ids.join(",")),
        );
    }
    if !after_want.iter().any(|word| word.id == "water") {
        fail(&mut failures, "Case 3", "water was not suggested");
    }

    let mut session = Session::fresh();
    session.board = Board::Big;
    let scene = layout::layout(&session.view(false), 800.0, 700.0);
    let big = scene.cells.iter().find(|cell| cell.place == Place::Board);
    match big {
        Some(cell) if cell.rect.w >= 140.0 && cell.rect.h >= 110.0 => {}
        Some(cell) => fail(
            &mut failures,
            "Case 4",
            &format!("big cell was {:.0}x{:.0}", cell.rect.w, cell.rect.h),
        ),
        None => fail(&mut failures, "Case 4", "no big cell"),
    }

    session.board = Board::Sentences;
    session.mode = Mode::User;
    for id in ["i", "want", "water"] {
        session.activate(Action::Word(id));
    }
    let says = session.sentence_says();
    if says.join(" ") != "I want water" {
        fail(
            &mut failures,
            "Case 5",
            &format!("sentence was {}", says.join(" ")),
        );
    }
    let next = board::recommend(&session.sentence_ids(), 6, &session.hidden, session.skill);
    if next.first().map(|word| word.id) != Some("please") {
        let ids: Vec<_> = next.iter().map(|word| word.id).collect();
        fail(
            &mut failures,
            "Case 5",
            &format!("next was {}", ids.join(",")),
        );
    }
    if session.spoken != ["I".to_string(), "want".to_string(), "water".to_string()] {
        fail(
            &mut failures,
            "Case 5",
            &format!("each word should speak alone: {}", session.spoken.join(" | ")),
        );
    }
    session.activate(Action::Hear);
    if session.spoken.last().map(String::as_str) != Some("I want water") || session.turns != 0 {
        fail(
            &mut failures,
            "Case 6",
            &format!(
                "hear it said {} turns {}",
                session.spoken.last().map(String::as_str).unwrap_or(""),
                session.turns
            ),
        );
    }
    session.activate(Action::SayAll);
    if session.spoken.last().map(String::as_str) != Some("I want water")
        || session.sentence_says().join(" ") != "I want water"
        || session.turns != 1
    {
        fail(
            &mut failures,
            "Case 6",
            &format!(
                "out loud was {} sentence {} turns {}",
                session.spoken.last().map(String::as_str).unwrap_or(""),
                session.sentence_says().join(" "),
                session.turns
            ),
        );
    }
    let spoken_before = session.spoken.len();
    session.activate(Action::Delete);
    if session.sentence_says().join(" ") != "I want" || session.spoken.len() != spoken_before {
        fail(
            &mut failures,
            "Case 7",
            &format!(
                "after delete {} spoken {}",
                session.sentence_says().join(" "),
                session.spoken.len()
            ),
        );
    }
    session.activate(Action::Clear);
    if !session.sentence_ids().is_empty() {
        fail(
            &mut failures,
            "Case 7",
            &format!("clear left {}", session.sentence_says().join(" ")),
        );
    }

    let scene = layout::layout(&session.view(false), 1000.0, 800.0);
    let small = scene.cells.iter().find(|cell| cell.place == Place::Board);
    match small {
        Some(cell) if cell.rect.h > 120.0 => {
            fail(
                &mut failures,
                "Case 8",
                &format!("sentence cell was tall {:.0}", cell.rect.h),
            );
        }
        None => fail(&mut failures, "Case 8", "no sentence cell"),
        _ => {}
    }

    let mut dwell = session::Dwell::new();
    let parent = Some(Action::Parent);
    if dwell.tick(parent, 0, 2600) {
        fail(&mut failures, "Case 9", "parent opened at 0 ms");
    }
    if dwell.tick(parent, 2599, 2600) {
        fail(
            &mut failures,
            "Case 9",
            "parent opened before the hold finished",
        );
    }
    if !dwell.tick(parent, 2600, 2600) {
        fail(&mut failures, "Case 9", "parent hold did not open setup");
    }
    if dwell.tick(parent, 3000, 2600) {
        fail(&mut failures, "Case 9", "parent hold fired twice");
    }

    if gaze::decode_point(&sample(0, 0, 0.25, 0.5)).is_none() {
        fail(&mut failures, "Case 10", "a seen gaze point was dropped");
    }
    let mut first = Session::fresh();
    first.activate(Action::Word("water"));
    if first.sentence_says().join(" ") != "I want water" {
        fail(
            &mut failures,
            "Case 11",
            &format!("first message was {}", first.sentence_says().join(" ")),
        );
    }
    if first.turns != 0 || first.board != Board::Big {
        fail(
            &mut failures,
            "Case 11",
            &format!("a new word counted a turn {} or left the stage", first.turns),
        );
    }
    if first.spoken.last().map(String::as_str) != Some("I want water") {
        fail(
            &mut failures,
            "Case 11",
            "the new words were not what was heard",
        );
    }
    let then = first.view(false);
    if then.next_ids.first().copied() != Some("please") {
        fail(
            &mut failures,
            "Case 11",
            &format!("next was {}", then.next_ids.join(",")),
        );
    }
    first.activate(Action::Word("please"));
    if first.sentence_says().join(" ") != "I want water please"
        || first.spoken.last().map(String::as_str) != Some("please")
        || first.turns != 0
    {
        fail(
            &mut failures,
            "Case 11",
            &format!(
                "please became {} spoke {} turns {}",
                first.sentence_says().join(" "),
                first.spoken.last().map(String::as_str).unwrap_or(""),
                first.turns
            ),
        );
    }
    first.activate(Action::Word("more"));
    if first.sentence_says().join(" ") != "I want water please more" {
        fail(
            &mut failures,
            "Case 11",
            &format!("more reset the line to {}", first.sentence_says().join(" ")),
        );
    }
    first.activate(Action::SayAll);
    if first.turns != 1
        || first.sentence_says().join(" ") != "I want water please more"
        || first.history.last().map(String::as_str) != Some("I want water please more")
    {
        fail(
            &mut failures,
            "Case 11",
            &format!(
                "out loud left {} turns {} history {}",
                first.sentence_says().join(" "),
                first.turns,
                first.history.last().map(String::as_str).unwrap_or("")
            ),
        );
    }

    if gaze::decode_point(&sample(4, 4, 0.25, 0.5)).is_some() {
        fail(
            &mut failures,
            "Case 10",
            "closed eyes still produced a point",
        );
    }
    let (x, y) = gaze::apply_warp(Some(gaze::IDENTITY), 0.2, 0.4);
    if (x - 0.2).abs() > 1e-9 || (y - 0.4).abs() > 1e-9 {
        fail(&mut failures, "Case 10", "identity warp moved the point");
    }

    let mut predicted = Session::fresh();
    predicted.board = Board::Predict;
    predicted.activate(Action::Word("i"));
    if predicted.heard != "I" || predicted.turns != 0 {
        fail(
            &mut failures,
            "Case 13",
            &format!("predict spoke {} turns {}", predicted.heard, predicted.turns),
        );
    }
    let after_i = predict::predict(&["i"], 4, &Default::default(), &Default::default(), 1);
    if after_i.first().map(|word| word.id) != Some("want") {
        fail(
            &mut failures,
            "Case 13",
            &format!(
                "after i the board offered {}",
                after_i.first().map(|word| word.id).unwrap_or("")
            ),
        );
    }
    let mut graph = predict::Graph::new();
    graph.insert(("want".to_string(), "outside".to_string()), 8.0);
    let after_want = predict::predict(&["want"], 4, &Default::default(), &graph, 1);
    if after_want.first().map(|word| word.id) != Some("outside") {
        fail(
            &mut failures,
            "Case 13",
            &format!(
                "a learned link offered {}",
                after_want.first().map(|word| word.id).unwrap_or("")
            ),
        );
    }

    let dog = board::word_by_id("dog");
    if dog.map(|word| word.level) != Some(2) {
        fail(&mut failures, "Case 14", "dog is not an everyday word");
    }
    let mut skilled = Session::fresh();
    skilled.turns = 4;
    skilled.activate(Action::ChooseSkill(2));
    if skilled.skill != 2 || skilled.turns != 4 {
        fail(
            &mut failures,
            "Case 14",
            &format!("skill {} turns {}", skilled.skill, skilled.turns),
        );
    }
    let everyday = layout::layout(&skilled.view(false), 900.0, 700.0);
    if !everyday
        .cells
        .iter()
        .any(|cell| cell.action == Action::Category("animals"))
    {
        fail(&mut failures, "Case 14", "everyday home hid animals");
    }
    skilled.activate(Action::Word("dog"));
    if skilled.heard != "I want dog" {
        fail(
            &mut failures,
            "Case 14",
            &format!("dog said {}", skilled.heard),
        );
    }
    skilled.skill = 1;
    skilled.sentence.clear();
    skilled.chunks.clear();
    skilled.heard.clear();
    skilled.page = None;
    skilled.activate(Action::Word("dog"));
    if !skilled.heard.is_empty() {
        fail(
            &mut failures,
            "Case 14",
            &format!("starting still said {}", skilled.heard),
        );
    }
    let starting = layout::layout(&skilled.view(false), 900.0, 700.0);
    if starting
        .cells
        .iter()
        .any(|cell| cell.action == Action::Category("animals"))
    {
        fail(&mut failures, "Case 14", "starting home showed animals");
    }
    let mut listed = Session::fresh();
    listed.board = Board::Sentences;
    listed.activate(Action::Word("and"));
    if !listed.sentence.is_empty() {
        fail(&mut failures, "Case 15", "starting accepted and");
    }
    listed.skill = 2;
    for id in ["banana", "and", "bread", "and", "milk"] {
        listed.activate(Action::Word(id));
    }
    if listed.sentence_says().join(" ") != "banana and bread and milk" {
        fail(
            &mut failures,
            "Case 15",
            &format!("list was {}", listed.sentence_says().join(" ")),
        );
    }
    let with_join = layout::layout(&listed.view(false), 1000.0, 800.0);
    if !with_join.cells.iter().any(|cell| cell.action == Action::Word("and"))
        || !with_join.cells.iter().any(|cell| cell.action == Action::Word("or"))
    {
        fail(&mut failures, "Case 15", "everyday bar hid and or or");
    }
    listed.skill = 1;
    let without_join = layout::layout(&listed.view(false), 1000.0, 800.0);
    if without_join
        .cells
        .iter()
        .any(|cell| cell.action == Action::Word("and") || cell.action == Action::Word("or"))
    {
        fail(&mut failures, "Case 15", "starting bar showed and or or");
    }

    let mut paused = Session::fresh();
    paused.board = Board::Sentences;
    paused.activate(Action::Word("i"));
    paused.activate(Action::Word("pause"));
    paused.activate(Action::Word("want"));
    if paused.sentence_says().join(" ") != "I <--> want" {
        fail(
            &mut failures,
            "Case 16",
            &format!("pause line was {}", paused.sentence_says().join(" ")),
        );
    }
    if paused.spoken.get(paused.spoken.len().wrapping_sub(2)).map(String::as_str) != Some(",") {
        fail(
            &mut failures,
            "Case 16",
            &format!("pause spoke {}", paused.spoken.join(" | ")),
        );
    }
    paused.activate(Action::Hear);
    if paused.spoken.last().map(String::as_str) != Some("I , want") {
        fail(
            &mut failures,
            "Case 16",
            &format!("full line spoke {}", paused.spoken.last().map(String::as_str).unwrap_or("")),
        );
    }

    skilled.page = Some("food");
    let foods = layout::layout(&skilled.view(false), 900.0, 700.0);
    if foods
        .cells
        .iter()
        .any(|cell| cell.action == Action::Word("pizza"))
    {
        fail(&mut failures, "Case 14", "starting food showed pizza");
    }

    Report {
        ok: failures.is_empty(),
        failures,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn self_check_matches_the_board() {
        let report = super::self_check();
        assert!(report.ok, "{}", report.text());
    }
}

fn sample(left: u32, right: u32, x: f64, y: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 56];
    let mask = gaze::BIT_GAZE_2D | gaze::BIT_VALIDITY_L | gaze::BIT_VALIDITY_R;
    bytes[0..4].copy_from_slice(&mask.to_le_bytes());
    bytes[8..12].copy_from_slice(&left.to_le_bytes());
    bytes[12..16].copy_from_slice(&right.to_le_bytes());
    bytes[40..48].copy_from_slice(&x.to_le_bytes());
    bytes[48..56].copy_from_slice(&y.to_le_bytes());
    bytes
}
