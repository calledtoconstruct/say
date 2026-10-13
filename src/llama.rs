//! Local llama.cpp test for next-word guesses.
//!
//! The word graph stays in charge until a parent picks Llama. The model
//! runs on this computer through llama-server. Nothing is sent off the machine.
//! Until the model answers, the board keeps showing the word graph.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use say::board::{self, Engine};
use say::session::Session;

const PORT: u16 = 8099;

pub enum Event {
    Up,
    Down(String),
    Guess { key: Vec<String>, ids: Vec<String> },
}

pub struct Host {
    child: Option<Child>,
    rx: Receiver<Event>,
    tx: Sender<Event>,
    busy: bool,
    asked: Vec<String>,
    ready: bool,
    failed: bool,
}

impl Host {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            child: None,
            rx,
            tx,
            busy: false,
            asked: Vec::new(),
            ready: false,
            failed: false,
        }
    }

    pub fn stop(&mut self) {
        self.ready = false;
        self.busy = false;
        self.asked.clear();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn poll(&mut self, session: &mut Session) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Up => {
                    self.ready = true;
                    session.llama_status = "Llama is ready.".to_string();
                }
                Event::Down(reason) => {
                    self.ready = false;
                    self.busy = false;
                    self.failed = true;
                    session.llama_status = reason;
                    if let Some(mut child) = self.child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
                Event::Guess { key, ids } => {
                    self.busy = false;
                    let current = sentence_key(session);
                    if key == current {
                        session.llama_key = key;
                        session.llama_ids = ids;
                    }
                }
            }
        }
        if session.engine != Engine::Llama {
            self.failed = false;
            if self.child.is_some() {
                self.stop();
            }
            session.llama_status.clear();
            return;
        }
        if self.failed {
            return;
        }
        if self.child.is_none() {
            match spawn_server() {
                Ok(child) => {
                    self.child = Some(child);
                    self.ready = false;
                    session.llama_status = "Llama is loading the model.".to_string();
                    watch_health(self.tx.clone());
                }
                Err(reason) => {
                    session.llama_status = reason;
                    return;
                }
            }
        }
        if !self.ready || self.busy {
            return;
        }
        let key = sentence_key(session);
        if key == session.llama_key || key == self.asked {
            return;
        }
        self.busy = true;
        self.asked = key.clone();
        let allowed = allowed_words(session);
        let shown = session.sentence_says().join(" ");
        let tx = self.tx.clone();
        thread::spawn(move || {
            let ids = match ask(&shown, &allowed) {
                Ok(ids) => ids,
                Err(reason) => {
                    let _ = tx.send(Event::Down(reason));
                    return;
                }
            };
            let _ = tx.send(Event::Guess { key, ids });
        });
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop();
    }
}

fn sentence_key(session: &Session) -> Vec<String> {
    session.sentence.iter().map(|id| (*id).to_string()).collect()
}

fn allowed_words(session: &Session) -> Vec<String> {
    board::words()
        .iter()
        .filter(|word| board::in_skill(word, session.skill))
        .filter(|word| !session.hidden.contains(word.id))
        .filter(|word| board::allows_repeat(word) || !session.sentence.contains(&word.id))
        .map(|word| word.id.to_string())
        .collect()
}

fn model_path() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("SAY_LLAMA_MODEL") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "SAY_LLAMA_MODEL is not a file: {}",
            path.display()
        ));
    }
    for candidate in [
        "/home/joseph/code/toolkitcreator/cache/artifacts/gguf-qwen25-3b/qwen2.5-3b-instruct-q4_k_m.gguf",
        "/home/joseph/code/toolkitcreator/cache/artifacts/gguf-llama32-3b/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("No local model was found. Set SAY_LLAMA_MODEL to a .gguf file.".into())
}

fn server_bin() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("SAY_LLAMA_SERVER") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
    }
    let home = PathBuf::from("/home/joseph/.local/share/say/llama-b11583/llama-server");
    if home.is_file() {
        return Ok(home);
    }
    Err("llama-server was not found. The word graph is still available.".into())
}

fn spawn_server() -> Result<Child, String> {
    let bin = server_bin()?;
    let model = model_path()?;
    let lib = bin
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().clamp(2, 8))
        .unwrap_or(4);
    unsafe {
        Command::new(&bin)
            .arg("-m")
            .arg(&model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(PORT.to_string())
            .arg("-c")
            .arg("4096")
            .arg("-t")
            .arg(threads.to_string())
            .arg("--no-webui")
            .env("LD_LIBRARY_PATH", lib)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .pre_exec(|| {
                // If Say exits, the model process exits with it.
                set_death_signal();
                Ok(())
            })
            .spawn()
            .map_err(|err| format!("Could not start llama-server: {err}"))
    }
}

fn set_death_signal() {
    unsafe extern "C" {
        fn prctl(option: i32, arg2: u64) -> i32;
    }
    const PR_SET_PDEATHSIG: i32 = 1;
    unsafe {
        prctl(PR_SET_PDEATHSIG, 9);
    }
}

fn watch_health(tx: Sender<Event>) {
    thread::spawn(move || {
        for _ in 0..180 {
            if health_ok() {
                let _ = tx.send(Event::Up);
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        let _ = tx.send(Event::Down(
            "Llama did not finish loading. The word graph is still there.".into(),
        ));
    });
}

fn health_ok() -> bool {
    let url = format!("http://127.0.0.1:{PORT}/health");
    Command::new("curl")
        .args(["-fsS", "--max-time", "1", &url])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn ask(sentence: &str, allowed: &[String]) -> Result<Vec<String>, String> {
    let sentence = if sentence.trim().is_empty() {
        "(the line is empty, so pick a word to start)".to_string()
    } else {
        sentence.to_string()
    };
    let take = allowed.len().clamp(1, 12);
    let user = format!("Sentence so far: {sentence}\nPut the best next word ids first.");
    let body = serde_json::json!({
        "model": "say",
        "temperature": 0.1,
        "max_tokens": 160,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "next_words",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "next": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": take,
                            "items": {"type": "string", "enum": allowed}
                        }
                    },
                    "required": ["next"],
                    "additionalProperties": false
                }
            }
        },
        "messages": [
            {"role": "system", "content": "You help a child finish a spoken line by choosing the next pictures. Prefer words that continue the meaning of the sentence."},
            {"role": "user", "content": user}
        ]
    });
    let path = std::env::temp_dir().join(format!("say-llama-{}.json", std::process::id()));
    std::fs::write(&path, body.to_string()).map_err(|err| format!("Could not write the Llama request: {err}"))?;
    let url = format!("http://127.0.0.1:{PORT}/v1/chat/completions");
    let output = Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "45",
            "-H",
            "content-type: application/json",
            "-d",
            &format!("@{}", path.display()),
            &url,
        ])
        .output()
        .map_err(|err| format!("Could not ask Llama: {err}"))?;
    let _ = std::fs::remove_file(&path);
    if !output.status.success() {
        return Err("Llama did not answer. The word graph is still there.".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let content = reply_text(&text).unwrap_or_default();
    Ok(parse_ids(&content, allowed))
}

fn reply_text(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .map(str::to_string)
}

fn parse_ids(text: &str, allowed: &[String]) -> Vec<String> {
    let hay = format!(
        " {} ",
        text.to_lowercase().replace(
            ['"', '[', ']', '{', '}', ',', '\n', '\t', '.', ';', ':'],
            " ",
        )
    );
    let mut hits = Vec::new();
    for id in allowed {
        let needle = format!(" {} ", id.to_lowercase());
        if let Some(pos) = hay.find(&needle) {
            hits.push((pos, id.clone()));
        }
    }
    hits.sort_by_key(|hit| hit.0);
    let mut out = Vec::new();
    for (_, id) in hits {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_ids;

    #[test]
    fn ids_follow_the_model_order() {
        let allowed = vec![
            "bread".into(),
            "milk".into(),
            "and".into(),
            "thank you".into(),
        ];
        let text = "thank you\nbread and milk";
        assert_eq!(
            parse_ids(text, &allowed),
            vec!["thank you".to_string(), "bread".to_string(), "and".to_string(), "milk".to_string()]
        );
    }

    #[test]
    fn json_ids_follow_the_model_order() {
        let allowed = vec![
            "bread".into(),
            "milk".into(),
            "and".into(),
            "thank you".into(),
        ];
        let text = "{\"next\": [\"thank you\", \"bread\", \"and\", \"milk\"]}";
        assert_eq!(
            parse_ids(text, &allowed),
            vec!["thank you".to_string(), "bread".to_string(), "and".to_string(), "milk".to_string()]
        );
    }
}
