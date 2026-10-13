//! Say, a picture talker. User mode is the child's board. Setup mode is
//! the parent's. Gaze comes from the tobiifreed daemon.

mod icons;
mod link;
mod llama;
mod paint;
mod speak;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

use gtk4::gdk::{Key, ModifierType};
use gtk4::glib::{self, ControlFlow};
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea, EventControllerKey, GestureClick};

use say::gaze::{Follow, WarpUpdate};
use say::layout;
use say::session::{hold_ms, Action, Dwell, Session};
use say::store::{self, Saved};

use link::GazeMsg;

struct Pointer {
    action: Action,
    at_ms: u64,
    fired: bool,
}

struct Ui {
    window: ApplicationWindow,
    session: Session,
    follow: Follow,
    speaker: speak::Speaker,
    llama: llama::Host,
    eyes: bool,
    pointer: Option<Pointer>,
    dwell: Dwell,
    started: Instant,
    rx: mpsc::Receiver<GazeMsg>,
    progress: f64,
    armed: Option<Action>,
    gaze: Option<(f64, f64)>,
    config: Option<std::path::PathBuf>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Say\n\n\
             A picture talker. The child uses the board. A parent holds Parent\n\
             or presses Ctrl+P to change it. A tap on Setup opens it too.\n\
             The skill chooses how many words show. Simple and Future are looks.\n\
             Hear it plays in the headphones. Out loud plays on the other speaker.\n\
             Predicted words guesses the next picture on this computer.\n\n\
             say              open the child's board\n\
             say --setup      open parent setup\n\
             say --selfcheck  check the word graph and the parent hold\n\n\
             Pictures and spoken text live in the content folder beside\n\
             this program. Add a word by adding a file there.\n\
             Ctrl+Q quits. Esc leaves setup, or leaves fullscreen."
        );
        return;
    }
    if args.iter().any(|arg| arg == "--selfcheck") {
        if let Err(err) = say::pack::ensure() {
            println!("SELFCHECK_FAIL\n{err}");
            std::process::exit(1);
        }
        let report = say::self_check();
        println!("{}", report.text());
        std::process::exit(if report.ok { 0 } else { 1 });
    }

    let app = Application::builder()
        .application_id("net.calledtoconstruct.Say")
        .build();
    let start_setup = args.iter().any(|arg| arg == "--setup");
    app.connect_activate(move |app| activate(app, start_setup));
    // GTK would otherwise reject --setup and --selfcheck as its own options.
    app.run_with_args(&["say"]);
}

fn activate(app: &Application, start_setup: bool) {
    if let Err(err) = say::pack::ensure() {
        eprintln!("Say: {err}");
        std::process::exit(1);
    }
    let mut session = Session::fresh();
    let config = store::config_path();
    if let Some(path) = &config {
        store::load(path).apply(&mut session);
    }
    if let Some(path) = store::graph_path() {
        session.learned = store::load_graph(&path);
    }
    load_speakers(&mut session);
    if start_setup {
        session.enter_setup();
    }

    let (tx, rx) = mpsc::channel();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    std::thread::spawn(move || link::run(tx, &thread_stop));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Say")
        .default_width(1280)
        .default_height(800)
        .build();
    let area = DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_focusable(true);
    window.set_child(Some(&area));

    let ui = Rc::new(RefCell::new(Ui {
        window: window.clone(),
        session,
        follow: Follow::new(),
        speaker: speak::Speaker::new(),
        llama: llama::Host::new(),
        eyes: false,
        pointer: None,
        dwell: Dwell::new(),
        started: Instant::now(),
        rx,
        progress: 0.0,
        armed: None,
        gaze: None,
        config,
    }));

    let drawn = ui.clone();
    area.set_draw_func(move |_area, cr, width, height| {
        let ui = drawn.borrow();
        let scene = layout::layout(&ui.session.view(ui.eyes), width as f64, height as f64);
        paint::paint(
            cr,
            &scene,
            ui.progress,
            ui.armed,
            ui.gaze,
            width as f64,
            height as f64,
        );
    });

    let gesture = GestureClick::new();
    let pressed = ui.clone();
    let area_for_press = area.clone();
    gesture.connect_pressed(move |_gesture, _n, x, y| {
        let mut ui = pressed.borrow_mut();
        let (width, height) = size_of(&area_for_press);
        let scene = layout::layout(&ui.session.view(ui.eyes), width, height);
        let Some(cell) = layout::hit(&scene.cells, x, y) else {
            ui.pointer = None;
            return;
        };
        ui.pointer = Some(Pointer {
            action: cell.action,
            at_ms: elapsed_ms(ui.started),
            fired: false,
        });
        ui.dwell.reset();
    });
    let released = ui.clone();
    let area_for_release = area.clone();
    gesture.connect_released(move |_gesture, _n, x, y| {
        let window = released.borrow().window.clone();
        let op = (|| {
            let mut ui = released.borrow_mut();
            let pointer = ui.pointer.take()?;
            if pointer.fired {
                return None;
            }
            let (width, height) = size_of(&area_for_release);
            let scene = layout::layout(&ui.session.view(ui.eyes), width, height);
            let cell = layout::hit(&scene.cells, x, y)?;
            if cell.action == pointer.action {
                fire(&mut ui, cell.action)
            } else {
                None
            }
        })();
        if let Some(op) = op {
            apply_window(&window, op);
        }
    });
    area.add_controller(gesture);

    let keyed = ui.clone();
    let keys = EventControllerKey::new();
    keys.connect_key_pressed(move |_controller, key, _code, modifiers| {
        let window = keyed.borrow().window.clone();
        let handled = (|| {
            let mut ui = keyed.borrow_mut();
            let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
            if ctrl && key == Key::q {
                ui.speaker.stop();
                ui.llama.stop();
                return Some(Some(WindowOp::Quit));
            }
            if ctrl && key == Key::p {
                load_speakers(&mut ui.session);
                ui.session.enter_setup();
                ui.dwell.reset();
                return Some(None);
            }
            if key == Key::Escape {
                if ui.session.mode == say::session::Mode::Setup {
                    if ui.session.page.is_some() {
                        return Some(fire(&mut ui, Action::Back));
                    }
                    return Some(fire(&mut ui, Action::Done));
                }
                if (ui.session.page.is_some() || ui.session.picking)
                    && ui.session.board != say::board::Board::Sentences
                {
                    return Some(fire(&mut ui, Action::Back));
                }
                if ui.window.is_fullscreen() {
                    return Some(Some(WindowOp::Fullscreen(false)));
                }
                return Some(None);
            }
            None
        })();
        match handled {
            Some(op) => {
                if let Some(op) = op {
                    apply_window(&window, op);
                }
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    });
    area.add_controller(keys);

    let ticking = ui.clone();
    let area_for_tick = area.clone();
    area.add_tick_callback(move |area, _clock| {
        let window = ticking.borrow().window.clone();
        let ops = {
            let mut ui = ticking.borrow_mut();
            let (width, height) = size_of(&area_for_tick);
            step(&mut ui, width, height)
        };
        for op in ops {
            apply_window(&window, op);
        }
        area.queue_draw();
        ControlFlow::Continue
    });

    window.connect_close_request(move |_| {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        glib::Propagation::Proceed
    });

    window.fullscreen();
    window.present();
    area.grab_focus();
}

fn step(ui: &mut Ui, width: f64, height: f64) -> Vec<WindowOp> {
    let mut ops = Vec::new();
    let now = elapsed_ms(ui.started);
    while let Ok(message) = ui.rx.try_recv() {
        match message {
            GazeMsg::Up(up) => ui.eyes = up,
            GazeMsg::Warp(update) => {
                let next = match update {
                    WarpUpdate::Set(values) => Some(values),
                    WarpUpdate::Clear => None,
                    WarpUpdate::Keep => ui.follow.warp,
                };
                ui.follow.set_warp(next);
            }
            GazeMsg::Point(x, y) => ui.follow.push(x, y, now),
        }
    }
    ui.speaker.poll();
    ui.llama.poll(&mut ui.session);

    let parent_ready = ui.pointer.as_ref().is_some_and(|pointer| {
        pointer.action == Action::Parent
            && !pointer.fired
            && now.saturating_sub(pointer.at_ms)
                >= hold_ms(ui.session.mode, ui.session.board, Action::Parent)
    });
    if parent_ready {
        if let Some(pointer) = ui.pointer.as_mut() {
            pointer.fired = true;
        }
        if let Some(op) = fire(ui, Action::Parent) {
            ops.push(op);
        }
    }

    let scene = layout::layout(&ui.session.view(ui.eyes), width, height);
    let pointer_down = ui.pointer.is_some();
    let gaze = if pointer_down {
        None
    } else {
        ui.follow.pixel(now, width, height)
    };
    let hover = gaze.and_then(|(x, y)| layout::hit(&scene.cells, x, y).map(|cell| cell.action));
    let need = hover
        .map(|action| hold_ms(ui.session.mode, ui.session.board, action))
        .unwrap_or(1);
    if pointer_down {
        ui.dwell.reset();
    } else if ui.dwell.tick(hover, now, need) {
        if let Some(action) = hover {
            if let Some(op) = fire(ui, action) {
                ops.push(op);
            }
        }
    }

    ui.gaze = gaze;
    if let Some(pointer) = ui.pointer.as_ref() {
        if pointer.action == Action::Parent && !pointer.fired {
            let need = hold_ms(ui.session.mode, ui.session.board, Action::Parent).max(1);
            ui.armed = Some(Action::Parent);
            ui.progress = (now.saturating_sub(pointer.at_ms) as f64 / need as f64).clamp(0.0, 1.0);
            return ops;
        }
    }
    ui.armed = if pointer_down { None } else { hover };
    ui.progress = if pointer_down {
        0.0
    } else {
        ui.dwell.progress(now, need.max(1))
    };
    ops
}

enum WindowOp {
    Fullscreen(bool),
    Quit,
}

/// Session updates stay here. Window calls that can redraw return in
/// [`WindowOp`] so the caller can run them after releasing the UI borrow.
fn load_speakers(session: &mut Session) {
    session.outputs = speak::output_devices();
    speak::prefer_routes(&mut session.ear, &mut session.room, &session.outputs);
}

fn fire(ui: &mut Ui, action: Action) -> Option<WindowOp> {
    match action {
        Action::Fullscreen => Some(WindowOp::Fullscreen(!ui.window.is_fullscreen())),
        Action::Parent => {
            load_speakers(&mut ui.session);
            ui.session.enter_setup();
            ui.dwell.reset();
            None
        }
        other => {
            let effect = ui.session.activate(other);
            if effect.stop_voice {
                ui.speaker.stop();
            }
            if let Some(text) = effect.speak {
                let sink = if effect.loud {
                    ui.session.room.as_str()
                } else {
                    ui.session.ear.as_str()
                };
                ui.speaker.say(
                    &text,
                    say::board::scaled_wpm(ui.session.board, ui.session.speed),
                    ui.session.muted,
                    sink,
                    &ui.session.voice,
                );
            }
            if effect.save {
                persist(ui);
            }
            if ui.session.graph_dirty {
                if let Some(path) = store::graph_path() {
                    let _ = store::save_graph(&path, &ui.session.learned);
                }
                ui.session.graph_dirty = false;
            }
            if matches!(other, Action::Done | Action::Back) {
                ui.dwell.reset();
            }
            None
        }
    }
}

fn apply_window(window: &ApplicationWindow, op: WindowOp) {
    match op {
        WindowOp::Fullscreen(true) => window.fullscreen(),
        WindowOp::Fullscreen(false) => window.unfullscreen(),
        WindowOp::Quit => window.close(),
    }
}

fn persist(ui: &Ui) {
    if let Some(path) = &ui.config {
        let _ = store::save(path, &Saved::from_session(&ui.session));
    }
}

fn size_of(area: &DrawingArea) -> (f64, f64) {
    let alloc = area.allocation();
    (alloc.width() as f64, alloc.height() as f64)
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}
