//! Tobii daemon framing. This crate does not link the daemon.
//!
//! A message is `[u8 type][u32 little-endian length][payload]`.
//! Gaze samples are the 392-byte layout. The screen warp is nine `f64`s,
//! sent on the WebSocket as type `0x04`, or read from
//! `~/.config/tobii/screen_warp.json` for a Unix-socket client.

pub const HEADER_SIZE: usize = 5;
pub const CMD_SUBSCRIBE: u8 = 0x01;
pub const SRV_GAZE: u8 = 0x01;
pub const SRV_SCREEN_WARP: u8 = 0x04;
pub const STREAM_GAZE: u32 = 0x500;

pub const BIT_VALIDITY_L: u32 = 1 << 2;
pub const BIT_VALIDITY_R: u32 = 1 << 3;
pub const BIT_GAZE_2D: u32 = 1 << 6;

/// Seen eye. The tracker uses 4 when an eye is not detected.
pub const EYE_SEEN: u32 = 0;

pub const IDENTITY: [f64; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub const FRESH_MS: u64 = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub kind: u8,
    pub payload: Vec<u8>,
}

/// The subscribe command the browser board sends: stream `0x500`.
pub fn subscribe_bytes() -> [u8; 9] {
    let mut out = [0u8; 9];
    out[0] = CMD_SUBSCRIBE;
    out[1..5].copy_from_slice(&4u32.to_le_bytes());
    out[5..9].copy_from_slice(&STREAM_GAZE.to_le_bytes());
    out
}

/// Pull one frame off `buf`. `Err` means the length is not believable.
pub fn pop_message(buf: &mut Vec<u8>) -> Result<Option<Message>, ()> {
    if buf.len() < HEADER_SIZE {
        return Ok(None);
    }
    let length = u32::from_le_bytes(buf[1..5].try_into().unwrap()) as usize;
    if length > 1024 * 1024 {
        buf.clear();
        return Err(());
    }
    let end = HEADER_SIZE + length;
    if buf.len() < end {
        return Ok(None);
    }
    let kind = buf[0];
    let payload = buf[HEADER_SIZE..end].to_vec();
    buf.drain(..end);
    Ok(Some(Message { kind, payload }))
}

/// Binocular gaze in display-plane units, or nothing when the sample
/// should not move the look. Matches the browser board: both eyes unseen
/// drops the sample, and a negative coordinate is ignored.
pub fn decode_point(payload: &[u8]) -> Option<(f64, f64)> {
    if payload.len() < 56 {
        return None;
    }
    let mask = u32::from_le_bytes(payload[0..4].try_into().ok()?);
    if mask & BIT_GAZE_2D == 0 {
        return None;
    }
    if mask & (BIT_VALIDITY_L | BIT_VALIDITY_R) != 0 {
        let left = if mask & BIT_VALIDITY_L != 0 {
            u32::from_le_bytes(payload[8..12].try_into().ok()?)
        } else {
            4
        };
        let right = if mask & BIT_VALIDITY_R != 0 {
            u32::from_le_bytes(payload[12..16].try_into().ok()?)
        } else {
            4
        };
        if left != EYE_SEEN && right != EYE_SEEN {
            return None;
        }
    }
    let x = f64::from_le_bytes(payload[40..48].try_into().ok()?);
    let y = f64::from_le_bytes(payload[48..56].try_into().ok()?);
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return None;
    }
    Some((x, y))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarpUpdate {
    Set([f64; 9]),
    Clear,
    Keep,
}

/// Type `0x04` payload. Shorter than nine numbers clears the correction.
/// A non-finite number leaves the current correction alone.
pub fn decode_warp(payload: &[u8]) -> WarpUpdate {
    if payload.len() < 72 {
        return WarpUpdate::Clear;
    }
    let mut out = [0.0; 9];
    for (index, slot) in out.iter_mut().enumerate() {
        let start = index * 8;
        let value = f64::from_le_bytes(payload[start..start + 8].try_into().unwrap());
        if !value.is_finite() {
            return WarpUpdate::Keep;
        }
        *slot = value;
    }
    WarpUpdate::Set(out)
}

pub fn apply_warp(warp: Option<[f64; 9]>, x: f64, y: f64) -> (f64, f64) {
    let Some(h) = warp else {
        return (x, y);
    };
    let w = h[6] * x + h[7] * y + h[8];
    if w.abs() < 1e-9 {
        return (x, y);
    }
    (
        (h[0] * x + h[1] * y + h[2]) / w,
        (h[3] * x + h[4] * y + h[5]) / w,
    )
}

/// `~/.config/tobii/screen_warp.json`, the file the daemon reads.
pub fn warp_file() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(home).join(".config/tobii/screen_warp.json"))
}

pub fn parse_warp_json(text: &str) -> Option<[f64; 9]> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let rows = value.get("homography")?.as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut out = [0.0; 9];
    for (row_index, row) in rows.iter().enumerate() {
        let row = row.as_array()?;
        if row.len() != 3 {
            return None;
        }
        for (col_index, cell) in row.iter().enumerate() {
            let number = cell.as_f64()?;
            if !number.is_finite() {
                return None;
            }
            out[row_index * 3 + col_index] = number;
        }
    }
    Some(out)
}

/// Smoothed look. The blend is the same 0.45 the browser board uses.
#[derive(Clone, Debug)]
pub struct Follow {
    pub warp: Option<[f64; 9]>,
    smooth: Option<(f64, f64)>,
    at_ms: Option<u64>,
}

impl Follow {
    pub fn new() -> Self {
        Self {
            warp: None,
            smooth: None,
            at_ms: None,
        }
    }

    pub fn set_warp(&mut self, warp: Option<[f64; 9]>) {
        if self.warp != warp {
            self.warp = warp;
            self.smooth = None;
            self.at_ms = None;
        }
    }

    pub fn push(&mut self, x: f64, y: f64, now_ms: u64) {
        let (x, y) = apply_warp(self.warp, x, y);
        self.smooth = Some(match self.smooth {
            None => (x, y),
            Some((sx, sy)) => (sx + (x - sx) * 0.45, sy + (y - sy) * 0.45),
        });
        self.at_ms = Some(now_ms);
    }

    /// Pixel point inside a window of `width` by `height`, or nothing
    /// when the last sample is older than [`FRESH_MS`].
    pub fn pixel(&self, now_ms: u64, width: f64, height: f64) -> Option<(f64, f64)> {
        let at = self.at_ms?;
        if now_ms.saturating_sub(at) > FRESH_MS {
            return None;
        }
        let (x, y) = self.smooth?;
        Some((x * width, y * height))
    }
}

impl Default for Follow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_matches_the_browser_board() {
        assert_eq!(subscribe_bytes(), [1, 4, 0, 0, 0, 0, 5, 0, 0]);
    }

    #[test]
    fn frames_reassemble() {
        let mut buf = subscribe_bytes().to_vec();
        buf.extend_from_slice(&[9]);
        let first = pop_message(&mut buf).unwrap().unwrap();
        assert_eq!(first.kind, CMD_SUBSCRIBE);
        assert_eq!(first.payload, STREAM_GAZE.to_le_bytes());
        assert!(pop_message(&mut buf).unwrap().is_none());
        assert_eq!(buf, vec![9]);
    }

    #[test]
    fn warp_json_is_row_major() {
        let text = r#"{"homography":[[1,0,0],[0,0.91,0],[0,0,1]]}"#;
        let warp = parse_warp_json(text).unwrap();
        assert!((warp[4] - 0.91).abs() < 1e-9);
        let (x, y) = apply_warp(Some(warp), 0.5, 0.5);
        assert!((x - 0.5).abs() < 1e-9);
        assert!((y - 0.455).abs() < 1e-9);
    }
}
