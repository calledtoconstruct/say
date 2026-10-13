//! Speaks through espeak-ng. Headphones and the room speaker are two outputs.
//! The line on screen does not depend on either one.

use std::process::{Child, Command, Stdio};

pub struct Speaker {
    voice: Option<Child>,
    play: Option<Child>,
}

impl Speaker {
    pub fn new() -> Self {
        Self {
            voice: None,
            play: None,
        }
    }

    /// `sink` is a PulseAudio sink name. Empty plays on the default output.
    /// `voice` is an espeak-ng voice id from [`crate::board::speech_voices`].
    pub fn say(&mut self, text: &str, wpm: i32, muted: bool, sink: &str, voice_id: &str) {
        self.stop();
        if muted || text.is_empty() {
            return;
        }
        let text: String = text.chars().take(200).collect();
        if sink.is_empty() {
            self.voice = espeak(&text, wpm, voice_id, false).ok();
            return;
        }
        let Ok(mut voice) = espeak(&text, wpm, voice_id, true) else {
            return;
        };
        let Some(wav) = voice.stdout.take() else {
            let _ = voice.kill();
            return;
        };
        let Ok(play) = Command::new("paplay")
            .arg(format!("--device={sink}"))
            .stdin(Stdio::from(wav))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            let _ = voice.kill();
            self.voice = espeak(&text, wpm, voice_id, false).ok();
            return;
        };
        // Dropping the espeak child would kill it before paplay reads the wave.
        let _ = voice.stdin.take();
        self.voice = Some(voice);
        self.play = Some(play);
    }

    pub fn stop(&mut self) {
        for child in [self.play.take(), self.voice.take()].into_iter().flatten() {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn poll(&mut self) {
        let play_done = self
            .play
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        if play_done {
            self.play = None;
        }
        let voice_done = self
            .voice
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        if voice_done && self.play.is_none() {
            self.voice = None;
        }
    }
}

fn espeak(text: &str, wpm: i32, voice_id: &str, wave: bool) -> Result<Child, std::io::Error> {
    let mut command = Command::new("espeak-ng");
    command.arg("-s").arg(wpm.to_string());
    if let Some(voice) = say::board::speech_voice(voice_id) {
        command.arg("-v").arg(voice.id);
    }
    if wave {
        command.arg("--stdout");
    }
    command
        .arg(text)
        .stdin(Stdio::null())
        .stdout(if wave { Stdio::piped() } else { Stdio::null() })
        .stderr(Stdio::null())
        .spawn()
}

impl Default for Speaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Connected speakers as (PulseAudio sink name, label a person can read).
pub fn output_devices() -> Vec<(String, String)> {
    let Ok(output) = Command::new("pactl")
        .args(["-f", "json", "list", "sinks"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(list) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for sink in list {
        let Some(name) = sink.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let description = sink
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        devices.push((name.to_string(), friendly_speaker(name, description)));
    }
    devices
}

/// Headphones go to a headset when one is connected. Out loud goes to the other speaker.
pub fn prefer_routes(ear: &mut String, room: &mut String, outputs: &[(String, String)]) {
    let known = |id: &str| outputs.iter().any(|(name, _)| name == id);
    if !ear.is_empty() && !known(ear) {
        ear.clear();
    }
    if !room.is_empty() && !known(room) {
        room.clear();
    }
    let headset = outputs.iter().find(|(id, label)| is_headset(id, label));
    let speaker = outputs.iter().find(|(id, label)| !is_headset(id, label));
    if ear.is_empty() {
        if let Some((id, _)) = headset.or(outputs.first()) {
            *ear = id.clone();
        }
    }
    if room.is_empty() {
        let other = speaker
            .filter(|(id, _)| id != ear)
            .or_else(|| outputs.iter().find(|(id, _)| id != ear))
            .or(outputs.first());
        if let Some((id, _)) = other {
            *room = id.clone();
        }
    }
}

fn is_headset(id: &str, label: &str) -> bool {
    let id = id.to_ascii_lowercase();
    let label = label.to_ascii_lowercase();
    id.contains("bluez")
        || label.contains("headphone")
        || label.contains("headset")
        || label.contains("buds")
        || label.contains("airpod")
}

fn friendly_speaker(name: &str, description: &str) -> String {
    let description = description.trim();
    if name.contains("bluez") {
        return if description.is_empty() {
            "Headphones".to_string()
        } else {
            description.to_string()
        };
    }
    if description.contains("Built-in") || name.contains("analog-stereo") {
        return "Built-in speakers".to_string();
    }
    if description.is_empty() {
        name.to_string()
    } else {
        description.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buds_are_headphones_and_the_jack_is_the_room() {
        let outputs = vec![
            (
                "alsa_output.pci-0000_00_1f.3.analog-stereo".to_string(),
                "Built-in speakers".to_string(),
            ),
            (
                "bluez_output.B4_23_A2_00_7B_B7.1".to_string(),
                "Joseph's Pixel Buds Pro 2".to_string(),
            ),
        ];
        let mut ear = String::new();
        let mut room = String::new();
        prefer_routes(&mut ear, &mut room, &outputs);
        assert_eq!(ear, "bluez_output.B4_23_A2_00_7B_B7.1");
        assert_eq!(room, "alsa_output.pci-0000_00_1f.3.analog-stereo");
        assert_eq!(
            friendly_speaker("bluez_output.X.1", "Joseph's Pixel Buds Pro 2"),
            "Joseph's Pixel Buds Pro 2"
        );
        assert_eq!(
            friendly_speaker("alsa_output.analog-stereo", "Built-in Audio Analog Stereo"),
            "Built-in speakers"
        );
    }
}
