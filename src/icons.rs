//! Pictures come from the content folder. The mute speaker stays drawn here.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use gtk4::cairo::{Context, ImageSurface};

struct Cached {
    mtime: Option<SystemTime>,
    surface: Option<ImageSurface>,
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Cached>> = RefCell::new(HashMap::new());
}

pub fn draw_file(cr: &Context, path: &str, x: f64, y: f64, size: f64) {
    if path.is_empty() || size < 4.0 {
        return;
    }
    let Some(surface) = surface_for(path) else {
        return;
    };
    let width = surface.width() as f64;
    let height = surface.height() as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    cr.save().ok();
    cr.translate(x, y);
    cr.scale(size / width, size / height);
    cr.set_source_surface(&surface, 0.0, 0.0).ok();
    cr.paint().ok();
    cr.restore().ok();
}

fn surface_for(path: &str) -> Option<ImageSurface> {
    let mtime = std::fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let cached = CACHE.with(|cache| {
        cache.borrow().get(path).and_then(|item| {
            if item.mtime == mtime {
                item.surface.clone()
            } else {
                None
            }
        })
    });
    if cached.is_some() {
        return cached;
    }
    let known = CACHE.with(|cache| {
        cache
            .borrow()
            .get(path)
            .is_some_and(|item| item.mtime == mtime)
    });
    if known {
        return None;
    }
    let surface = raster(Path::new(path));
    CACHE.with(|cache| {
        cache.borrow_mut().insert(
            path.to_string(),
            Cached {
                mtime,
                surface: surface.clone(),
            },
        );
    });
    surface
}

fn raster(path: &Path) -> Option<ImageSurface> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = if ext == "svg" {
        let out = Command::new("rsvg-convert")
            .args(["-w", "256", "-h", "256"])
            .arg(path)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        out.stdout
    } else if ext == "png" {
        return ImageSurface::create_from_png(&mut std::fs::File::open(path).ok()?).ok();
    } else {
        let out = Command::new("magick")
            .arg(path)
            .args(["-resize", "256x256", "png:-"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        out.stdout
    };
    ImageSurface::create_from_png(&mut bytes.as_slice()).ok()
}

pub fn speaker(cr: &Context, muted: bool, x: f64, y: f64, size: f64) {
    cr.save().ok();
    cr.translate(x, y);
    cr.scale(size / 24.0, size / 24.0);
    cr.set_source_rgb(0.16, 0.20, 0.24);
    cr.set_line_width(2.0);
    cr.move_to(3.0, 9.0);
    cr.line_to(7.0, 9.0);
    cr.line_to(12.0, 5.0);
    cr.line_to(12.0, 19.0);
    cr.line_to(7.0, 15.0);
    cr.line_to(3.0, 15.0);
    cr.close_path();
    cr.fill().ok();
    if muted {
        cr.move_to(16.0, 9.0);
        cr.line_to(21.0, 15.0);
        cr.move_to(21.0, 9.0);
        cr.line_to(16.0, 15.0);
    } else {
        cr.move_to(16.0, 8.5);
        cr.curve_to(19.0, 10.0, 19.0, 14.0, 16.0, 15.5);
    }
    cr.stroke().ok();
    cr.restore().ok();
}
