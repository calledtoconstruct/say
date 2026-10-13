//! Words, pictures, and phrases loaded from the content folder.
//!
//! The folder is the board. Adding a word is a json file plus a picture.
//! Strings are leaked once so the rest of the app can keep `Copy` actions.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::board::{Category, Follow, Role, Word};

struct Pack {
    words: &'static [Word],
    categories: &'static [Category],
    follow: &'static [Follow],
}

static PACK: OnceLock<Pack> = OnceLock::new();

pub fn ensure() -> Result<(), String> {
    if PACK.get().is_some() {
        return Ok(());
    }
    let loaded = load()?;
    let _ = PACK.get_or_init(|| loaded);
    Ok(())
}

fn loaded() -> &'static Pack {
    PACK.get().unwrap_or_else(|| {
        PACK.get_or_init(|| match load() {
            Ok(pack) => pack,
            Err(err) => panic!("{err}"),
        })
    })
}

pub fn words() -> &'static [Word] {
    loaded().words
}

pub fn categories() -> &'static [Category] {
    loaded().categories
}

pub fn follow() -> &'static [Follow] {
    loaded().follow
}

pub fn content_dir() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("SAY_CONTENT") {
        let path = PathBuf::from(value);
        if path.join("words").is_dir() {
            return Ok(path);
        }
        return Err(format!(
            "SAY_CONTENT does not contain a words folder: {}",
            path.display()
        ));
    }
    let compiled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content");
    if compiled.join("words").is_dir() {
        return Ok(compiled);
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..6 {
            let Some(here) = dir else { break };
            let candidate = here.join("content");
            if candidate.join("words").is_dir() {
                return Ok(candidate);
            }
            dir = here.parent().map(Path::to_path_buf);
        }
    }
    let local = PathBuf::from("content");
    if local.join("words").is_dir() {
        return Ok(local);
    }
    Err("Could not find the Say content folder. Set SAY_CONTENT to the folder that contains words/.".into())
}

#[derive(Deserialize)]
struct WordFile {
    #[serde(default)]
    say: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    starter: f64,
    #[serde(default)]
    closes: bool,
    #[serde(default)]
    repeat: bool,
    #[serde(default)]
    also: Vec<String>,
    #[serde(default)]
    phrase: Vec<String>,
    #[serde(default)]
    next: BTreeMap<String, f64>,
    /// Missing means the first set, so an older file stays visible.
    #[serde(default = "starting_level")]
    level: u8,
}

fn starting_level() -> u8 {
    1
}

struct Draft {
    id: &'static str,
    say: &'static str,
    role: Role,
    vec: [f64; 7],
    starter: f64,
    closes: bool,
    repeat: bool,
    also: Vec<&'static str>,
    phrase: Vec<&'static str>,
    next: Vec<(&'static str, f64)>,
    icon: &'static str,
    categories: Vec<String>,
    level: u8,
}

fn load() -> Result<Pack, String> {
    let dir = content_dir()?
        .canonicalize()
        .map_err(|err| format!("could not open the Say content folder: {err}"))?;
    let words_dir = dir.join("words");
    if !words_dir.is_dir() {
        return Err(format!("missing words folder: {}", words_dir.display()));
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(&words_dir)
        .map_err(|err| format!("could not read {}: {err}", words_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    paths.sort();

    let mut names = HashMap::<String, &'static str>::new();
    for path in &paths {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("word file name is not text: {}", path.display()))?;
        intern(&mut names, stem.to_string());
    }

    let mut drafts = Vec::new();
    for path in &paths {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?;
        let file: WordFile = serde_json::from_str(&text)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap();
        let id = *names.get(stem).expect("stem interned");
        let role = if file.role.is_empty() {
            Role::Social
        } else {
            parse_role(&file.role, path)?
        };
        let mut also = Vec::new();
        let mut vec = [0.0; 7];
        vec[role_index(role)] = 1.0;
        for name in &file.also {
            let extra = parse_role(name, path)?;
            vec[role_index(extra)] = 1.0;
            also.push(leak(extra.name().to_string()));
        }
        let say = if file.say.is_empty() { id } else { leak(file.say) };
        let icon = match find_picture(&words_dir, id) {
            Some(picture) => leak(fs::canonicalize(&picture).unwrap_or(picture).to_string_lossy().into_owned()),
            None => "",
        };
        let phrase = file
            .phrase
            .iter()
            .filter_map(|key| names.get(key).copied())
            .collect();
        let mut next: Vec<(&'static str, f64)> = file
            .next
            .iter()
            .filter_map(|(key, weight)| names.get(key).copied().map(|id| (id, *weight)))
            .collect();
        next.sort_by(|a, b| a.0.cmp(b.0));
        drafts.push(Draft {
            id,
            say,
            role,
            vec,
            starter: file.starter,
            closes: file.closes,
            repeat: file.repeat,
            also,
            phrase,
            next,
            icon,
            categories: file.categories,
            level: file.level.clamp(1, 4),
        });
    }

    let categories = load_categories(&dir, &names, &drafts)?;
    let follow = load_follow(&dir, &names)?;
    let words = assemble_words(&drafts, &categories);
    Ok(Pack {
        words: leak_slice(words),
        categories: leak_slice(categories),
        follow: leak_slice(follow),
    })
}

fn load_categories(
    dir: &Path,
    names: &HashMap<String, &'static str>,
    drafts: &[Draft],
) -> Result<Vec<Category>, String> {
    let cat_dir = dir.join("categories");
    if !cat_dir.is_dir() {
        return Err(format!("missing categories folder: {}", cat_dir.display()));
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&cat_dir)
        .map_err(|err| format!("could not read {}: {err}", cat_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("txt"))
        .collect();
    files.sort();
    let mut by_id: BTreeMap<String, Category> = BTreeMap::new();
    for path in files {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("category file name is not text: {}", path.display()))?
            .to_string();
        if stem == "order" {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?;
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#'));
        let Some(label) = lines.next() else {
            return Err(format!("{} is empty", path.display()));
        };
        let mut home = true;
        let mut bar = false;
        let mut tone = None;
        let mut word_ids: Vec<&'static str> = Vec::new();
        let mut pending = Vec::new();
        if let Some(second) = lines.next() {
            match second {
                "home" => {}
                "bar" => {
                    home = false;
                    bar = true;
                }
                "setup" => home = false,
                other => pending.push(other.to_string()),
            }
        }
        pending.extend(lines.map(|line| line.to_string()));
        for line in pending {
            if let Some(name) = line.strip_prefix("tone ") {
                tone = Some(parse_role(name.trim(), &path)?);
                continue;
            }
            if let Some(id) = names.get(&line) {
                if !word_ids.contains(id) {
                    word_ids.push(*id);
                }
            }
        }
        let mut extra: Vec<&str> = drafts
            .iter()
            .filter(|draft| draft.categories.iter().any(|name| name == &stem))
            .map(|draft| draft.id)
            .filter(|id| !word_ids.contains(id))
            .collect();
        extra.sort();
        word_ids.extend(extra);
        let icon = match find_picture(&cat_dir, &stem) {
            Some(picture) => leak(fs::canonicalize(&picture).unwrap_or(picture).to_string_lossy().into_owned()),
            None => "",
        };
        by_id.insert(
            stem.clone(),
            Category {
                id: leak(stem),
                label: leak(label.to_string()),
                words: leak_slice(word_ids),
                icon,
                home,
                bar,
                tone,
            },
        );
    }

    let mut order = Vec::new();
    let order_path = dir.join("order.txt");
    if order_path.is_file() {
        let text = fs::read_to_string(&order_path)
            .map_err(|err| format!("could not read {}: {err}", order_path.display()))?;
        for line in text.lines() {
            let id = line.trim();
            if id.is_empty() || id.starts_with('#') {
                continue;
            }
            if by_id.contains_key(id) && !order.contains(&id.to_string()) {
                order.push(id.to_string());
            }
        }
    }
    let mut rest: Vec<String> = by_id.keys().filter(|id| !order.contains(id)).cloned().collect();
    rest.sort();
    order.extend(rest);
    Ok(order.into_iter().map(|id| by_id.remove(&id).expect("ordered")).collect())
}

fn load_follow(dir: &Path, names: &HashMap<String, &'static str>) -> Result<Vec<Follow>, String> {
    let path = dir.join("follow.txt");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (raw, always) = match line.strip_suffix('+') {
            Some(body) => (body.trim(), true),
            None => (line, false),
        };
        let Some(id) = names.get(raw).copied() else {
            continue;
        };
        if out.iter().any(|item: &Follow| item.id == id) {
            continue;
        }
        out.push(Follow { id, always });
    }
    Ok(out)
}

fn assemble_words(drafts: &[Draft], categories: &[Category]) -> Vec<Word> {
    let by_id: HashMap<&str, &Draft> = drafts.iter().map(|draft| (draft.id, draft)).collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for cat in categories {
        for id in cat.words {
            if !seen.insert(*id) {
                continue;
            }
            if let Some(draft) = by_id.get(id) {
                out.push(word_from(draft));
            }
        }
    }
    let mut rest: Vec<&Draft> = drafts.iter().filter(|draft| !seen.contains(draft.id)).collect();
    rest.sort_by(|a, b| a.id.cmp(b.id));
    for draft in rest {
        out.push(word_from(draft));
    }
    out
}

fn word_from(draft: &Draft) -> Word {
    Word {
        id: draft.id,
        say: draft.say,
        role: draft.role,
        vec: draft.vec,
        phrase: leak_slice(draft.phrase.clone()),
        starter: draft.starter,
        closes: draft.closes,
        repeat: draft.repeat,
        also: leak_slice(draft.also.clone()),
        next: leak_slice(draft.next.clone()),
        icon: draft.icon,
        level: draft.level,
    }
}

fn find_picture(dir: &Path, stem: &str) -> Option<PathBuf> {
    // A photograph dropped beside the json replaces the symbol. SVG is the
    // fallback so the shipped pictures still show when no photo is there.
    for ext in [
        "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "tif", "tiff", "svg",
    ] {
        let path = dir.join(format!("{stem}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn intern(names: &mut HashMap<String, &'static str>, id: String) -> &'static str {
    if let Some(found) = names.get(&id) {
        return *found;
    }
    let leaked = leak(id.clone());
    names.insert(id, leaked);
    leaked
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn leak_slice<T>(items: Vec<T>) -> &'static [T] {
    Box::leak(items.into_boxed_slice())
}

fn parse_role(name: &str, path: &Path) -> Result<Role, String> {
    Role::parse(name).ok_or_else(|| format!("{}: unknown role {name}", path.display()))
}

fn role_index(role: Role) -> usize {
    match role {
        Role::Person => 0,
        Role::Verb => 1,
        Role::Object => 2,
        Role::Feeling => 3,
        Role::Place => 4,
        Role::Polite => 5,
        Role::Social => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;

    #[test]
    fn shipped_pack_matches_the_board() {
        let words = board::words();
        let water = board::word_by_id("water").unwrap();
        assert_eq!(water.phrase, ["i", "want", "water"]);
        assert!(water.icon.ends_with("water.svg"), "{}", water.icon);
        assert!(board::word_by_id("help").unwrap().closes);
        assert!(board::word_by_id("play").unwrap().closes);
        assert!(board::word_by_id("eat").unwrap().closes);
        assert!(board::word_by_id("drink").unwrap().closes);
        let help = board::word_by_id("help").unwrap();
        assert_eq!(help.vec[1], 1.0);
        assert_eq!(help.vec[6], 1.0);
        let follow = follow();
        assert_eq!(follow.len(), 4);
        assert_eq!(follow[2].id, "more");
        assert!(follow[2].always);
        assert!(board::categories().iter().any(|cat| cat.bar && cat.id == "answers"));
        assert!((board::word_by_id("i").unwrap().starter - 8.0).abs() < 0.01);
        assert_eq!(words.iter().filter(|word| word.level == 1).count(), 37);
        assert!(board::word_by_id("pause").unwrap().repeat);
        assert_eq!(board::word_by_id("water").unwrap().level, 1);
        assert!(words.len() > 120, "{}", words.len());
        let dog = board::word_by_id("dog").unwrap();
        assert_eq!(dog.level, 2);
        assert!(dog.icon.ends_with("dog.svg"), "{}", dog.icon);
        let home: Vec<_> = board::categories()
            .iter()
            .filter(|cat| cat.home)
            .map(|cat| cat.id)
            .take(5)
            .collect();
        assert_eq!(home, ["people", "needs", "food", "feelings", "places"]);
    }

    #[test]
    fn a_photograph_replaces_the_symbol() {
        let dir = std::env::temp_dir().join(format!("say-pic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mom.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        fs::write(dir.join("mom.jpg"), b"photo").unwrap();
        let found = find_picture(&dir, "mom").unwrap();
        assert_eq!(found.file_name().unwrap(), "mom.jpg");
        fs::write(dir.join("park.gif"), b"gif").unwrap();
        assert_eq!(find_picture(&dir, "park").unwrap().extension().unwrap(), "gif");
        fs::write(dir.join("cup.webp"), b"w").unwrap();
        assert!(find_picture(&dir, "cup").is_some_and(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("webp")
        }));
        let _ = fs::remove_dir_all(&dir);
    }
}
