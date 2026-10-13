//! Reads the tobiifreed gaze stream.
//!
//! The browser board uses `ws://127.0.0.1:7081`. This tries that first,
//! because the daemon sends the screen warp there. If the port is closed
//! it uses `$XDG_RUNTIME_DIR/tobiifreed/gaze.sock` and the warp file.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use say::gaze::{self, WarpUpdate};

pub enum GazeMsg {
    Up(bool),
    Warp(WarpUpdate),
    Point(f64, f64),
}

pub fn run(tx: Sender<GazeMsg>, stop: &std::sync::atomic::AtomicBool) {
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if let Ok((stream, pending)) = connect_ws() {
            let _ = tx.send(GazeMsg::Up(true));
            read_ws(stream, pending, &tx, stop);
            let _ = tx.send(GazeMsg::Up(false));
        } else if let Some(stream) = connect_unix() {
            let _ = tx.send(GazeMsg::Up(true));
            read_unix(stream, &tx, stop);
            let _ = tx.send(GazeMsg::Up(false));
        }
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn socket_path() -> Option<std::path::PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    if runtime.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(runtime).join("tobiifreed/gaze.sock"))
}

fn connect_unix() -> Option<UnixStream> {
    let path = socket_path()?;
    let stream = UnixStream::connect(path).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(400)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    let mut stream = stream;
    stream.write_all(&gaze::subscribe_bytes()).ok()?;
    Some(stream)
}

fn connect_ws() -> std::io::Result<(TcpStream, Vec<u8>)> {
    let addr: SocketAddr = "127.0.0.1:7081".parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(400))?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_millis(400)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let request = format!(
        "GET / HTTP/1.1\r\n\
         Host: 127.0.0.1:7081\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut buf = Vec::new();
    let header_end = read_header(&mut stream, &mut buf)?;
    let header = String::from_utf8_lossy(&buf[..header_end]);
    if !header.starts_with("HTTP/1.1 101") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "no websocket",
        ));
    }
    buf.drain(..header_end);
    write_client_frame(&mut stream, 0x2, &gaze::subscribe_bytes(), 1)?;
    Ok((stream, buf))
}

fn read_header(stream: &mut TcpStream, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    let mut tmp = [0u8; 512];
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(end) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            return Ok(end + 4);
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "websocket header",
    ))
}

fn read_ws(
    mut stream: TcpStream,
    mut buf: Vec<u8>,
    tx: &Sender<GazeMsg>,
    stop: &std::sync::atomic::AtomicBool,
) {
    let mut tmp = [0u8; 8192];
    let mut mask_n = 2u32;
    let mut watch = WarpWatch::default();
    let mut polled = Instant::now() - Duration::from_secs(3);
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if polled.elapsed() >= Duration::from_secs(2) {
            watch.poll(tx);
            polled = Instant::now();
        }
        if !dispatch_frames(&mut stream, &mut buf, tx, &mut mask_n) {
            break;
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
}

fn dispatch_frames(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    tx: &Sender<GazeMsg>,
    mask_n: &mut u32,
) -> bool {
    let frames = match take_frames(buf) {
        Ok(frames) => frames,
        Err(()) => return false,
    };
    for frame in frames {
        match frame.opcode {
            0x2 => emit_daemon(tx, &frame.payload),
            0x8 => return false,
            0x9 => {
                if write_client_frame(stream, 0xA, &frame.payload, *mask_n).is_err() {
                    return false;
                }
                *mask_n = mask_n.wrapping_add(1);
            }
            _ => {}
        }
    }
    true
}

fn read_unix(mut stream: UnixStream, tx: &Sender<GazeMsg>, stop: &std::sync::atomic::AtomicBool) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let mut watch = WarpWatch::default();
    let mut polled = Instant::now() - Duration::from_secs(3);
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if polled.elapsed() >= Duration::from_secs(2) {
            watch.poll(tx);
            polled = Instant::now();
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
        loop {
            match gaze::pop_message(&mut buf) {
                Ok(Some(message)) => emit_one(tx, message.kind, &message.payload),
                Ok(None) => break,
                Err(()) => return,
            }
        }
    }
}

fn emit_daemon(tx: &Sender<GazeMsg>, payload: &[u8]) {
    let mut buf = payload.to_vec();
    loop {
        match gaze::pop_message(&mut buf) {
            Ok(Some(message)) => emit_one(tx, message.kind, &message.payload),
            Ok(None) => break,
            Err(()) => break,
        }
    }
}

fn emit_one(tx: &Sender<GazeMsg>, kind: u8, payload: &[u8]) {
    match kind {
        gaze::SRV_GAZE => {
            if let Some((x, y)) = gaze::decode_point(payload) {
                let _ = tx.send(GazeMsg::Point(x, y));
            }
        }
        gaze::SRV_SCREEN_WARP => {
            let _ = tx.send(GazeMsg::Warp(gaze::decode_warp(payload)));
        }
        _ => {}
    }
}

struct Frame {
    opcode: u8,
    payload: Vec<u8>,
}

fn take_frames(buf: &mut Vec<u8>) -> Result<Vec<Frame>, ()> {
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let remaining = buf.len() - pos;
        if remaining < 2 {
            break;
        }
        let b0 = buf[pos];
        let b1 = buf[pos + 1];
        let opcode = b0 & 0x0f;
        let masked = b1 & 0x80 != 0;
        let mut payload_len = (b1 & 0x7f) as usize;
        let mut header_len = 2;
        if payload_len == 126 {
            if remaining < 4 {
                break;
            }
            payload_len = u16::from_be_bytes(buf[pos + 2..pos + 4].try_into().unwrap()) as usize;
            header_len = 4;
        } else if payload_len == 127 {
            if remaining < 10 {
                break;
            }
            let len = u64::from_be_bytes(buf[pos + 2..pos + 10].try_into().unwrap());
            if len > 1_048_576 {
                return Err(());
            }
            payload_len = len as usize;
            header_len = 10;
        }
        let mask_len = if masked { 4 } else { 0 };
        let total = header_len + mask_len + payload_len;
        if remaining < total {
            break;
        }
        let mut payload = buf[pos + header_len + mask_len..pos + total].to_vec();
        if masked {
            let mask = &buf[pos + header_len..pos + header_len + 4];
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        frames.push(Frame { opcode, payload });
        pos += total;
    }
    if pos > 0 {
        buf.drain(..pos);
    }
    Ok(frames)
}

fn write_client_frame(
    stream: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    n: u32,
) -> std::io::Result<()> {
    let mask = n.wrapping_mul(0x9E37_79B1).to_le_bytes();
    let mut header = Vec::with_capacity(14);
    header.push(0x80 | opcode);
    if payload.len() < 126 {
        header.push(0x80 | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        header.push(0x80 | 126);
        header.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        header.push(0x80 | 127);
        header.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    header.extend_from_slice(&mask);
    let mut body = payload.to_vec();
    for (index, byte) in body.iter_mut().enumerate() {
        *byte ^= mask[index % 4];
    }
    stream.write_all(&header)?;
    stream.write_all(&body)?;
    Ok(())
}

#[derive(Default)]
struct WarpWatch {
    stamp: Option<(SystemTime, u64)>,
    saw_file: bool,
}

impl WarpWatch {
    fn poll(&mut self, tx: &Sender<GazeMsg>) {
        let Some(path) = gaze::warp_file() else {
            return;
        };
        let meta = match std::fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => {
                if self.saw_file {
                    self.saw_file = false;
                    self.stamp = None;
                    let _ = tx.send(GazeMsg::Warp(WarpUpdate::Clear));
                }
                return;
            }
        };
        let stamp = (meta.modified().unwrap_or(UNIX_EPOCH), meta.len());
        if self.stamp == Some(stamp) {
            return;
        }
        self.stamp = Some(stamp);
        self.saw_file = true;
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        if let Some(values) = gaze::parse_warp_json(&text) {
            let _ = tx.send(GazeMsg::Warp(WarpUpdate::Set(values)));
        }
    }
}
