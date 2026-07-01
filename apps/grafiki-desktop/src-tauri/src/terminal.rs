//! Hosted terminal: Grafiki spawns a real shell/agent in a PTY, renders it in the
//! UI (xterm.js), and TEES the output into Grafiki's capture pipeline. Because we
//! own the PTY we see every byte and know the folder — so a session run inside
//! Grafiki (e.g. `claude`) is captured automatically, no daemon or transcript
//! discovery. Captured output → `capture_events` → the usual extraction/review.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use grafiki_core::{
    ingest_capture_event, start_capture_session, stop_capture_session, IngestCaptureEventOptions,
    StartCaptureOptions, StopCaptureOptions,
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tauri::State;

/// A live hosted terminal session.
struct TerminalSession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Grafiki capture session id (`None` when the folder isn't a Grafiki project).
    capture_id: Option<String>,
    project_root: String,
    /// Output accumulated for capture (flushed on threshold + on close).
    buffer: Arc<Mutex<Vec<u8>>>,
}

/// Tauri managed state: all live terminal sessions by id.
#[derive(Default)]
pub struct TerminalRegistry(Mutex<HashMap<String, TerminalSession>>);

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Open a hosted terminal: spawn `command` in `cwd` inside a PTY, stream its bytes
/// to `on_output`, and capture the session into `cwd`'s Grafiki project (if any).
#[tauri::command]
pub fn terminal_open(
    registry: State<TerminalRegistry>,
    id: String,
    cwd: String,
    command: String,
    rows: u16,
    cols: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<String, String> {
    let pair = native_pty_system()
        .openpty(pty_size(rows, cols))
        .map_err(|error| error.to_string())?;

    // Empty command → the user's default login shell (they can then run `claude`
    // etc.); a specific command (e.g. "claude") launches that agent directly.
    let mut cmd = if command.trim().is_empty() {
        CommandBuilder::new_default_prog()
    } else {
        CommandBuilder::new(&command)
    };
    cmd.cwd(&cwd);
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|error| error.to_string())?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    // Release the slave so the reader sees EOF when the child exits.
    drop(pair.slave);
    let master = pair.master;

    // Best-effort capture session for this folder; skipped if it isn't a Grafiki
    // project (the terminal still works — capture is additive, never blocking).
    let capture_id = start_capture_session(StartCaptureOptions {
        project_name: None,
        start_dir: PathBuf::from(&cwd),
        grafiki_home: None,
        scope: String::new(),
        source_app: Some("grafiki-terminal".to_owned()),
        consent_profile: None,
        redaction_profile: None,
    })
    .ok()
    .map(|report| report.capture.id);

    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));

    // Reader thread: stream output to the UI and tee it into the capture buffer.
    {
        let buffer = buffer.clone();
        let capture_id = capture_id.clone();
        let project_root = cwd.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break, // child exited or the pty closed
                    Ok(n) => {
                        let bytes = &chunk[..n];
                        if on_output.send(bytes.to_vec()).is_err() {
                            break; // UI went away
                        }
                        if capture_id.is_some() {
                            let flush = {
                                let mut buffered = buffer.lock().unwrap();
                                buffered.extend_from_slice(bytes);
                                if buffered.len() > 64 * 1024 {
                                    Some(std::mem::take(&mut *buffered))
                                } else {
                                    None
                                }
                            };
                            if let Some(raw) = flush {
                                flush_capture(&project_root, &capture_id, raw);
                            }
                        }
                    }
                }
            }
        });
    }

    registry.0.lock().unwrap().insert(
        id.clone(),
        TerminalSession {
            writer,
            master,
            child,
            capture_id,
            project_root: cwd,
            buffer,
        },
    );
    Ok(id)
}

/// Send keystrokes (or paste) to the hosted shell.
#[tauri::command]
pub fn terminal_write(
    registry: State<TerminalRegistry>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = registry.0.lock().unwrap();
    if let Some(session) = sessions.get_mut(&id) {
        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|error| error.to_string())?;
        session.writer.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Resize the PTY when the terminal pane resizes.
#[tauri::command]
pub fn terminal_resize(
    registry: State<TerminalRegistry>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let sessions = registry.0.lock().unwrap();
    if let Some(session) = sessions.get(&id) {
        session
            .master
            .resize(pty_size(rows, cols))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Close the terminal: kill the child, flush the remaining output to capture, and
/// stop the capture session.
#[tauri::command]
pub fn terminal_close(registry: State<TerminalRegistry>, id: String) -> Result<(), String> {
    let session = registry.0.lock().unwrap().remove(&id);
    if let Some(mut session) = session {
        let _ = session.child.kill();
        let raw = std::mem::take(&mut *session.buffer.lock().unwrap());
        if let Some(capture_id) = session.capture_id.clone() {
            flush_capture(&session.project_root, &session.capture_id, raw);
            let _ = stop_capture_session(StopCaptureOptions {
                project_name: None,
                start_dir: PathBuf::from(&session.project_root),
                grafiki_home: None,
                capture_id,
            });
        }
    }
    Ok(())
}

/// Persist a chunk of terminal output as a capture event (ANSI-stripped). Silent
/// on error — capture must never disrupt the live terminal.
fn flush_capture(project_root: &str, capture_id: &Option<String>, raw: Vec<u8>) {
    let text = strip_ansi(&raw);
    if text.trim().is_empty() {
        return;
    }
    let _ = ingest_capture_event(IngestCaptureEventOptions {
        project_name: None,
        start_dir: PathBuf::from(project_root),
        grafiki_home: None,
        capture_id: capture_id.clone(),
        scope: String::new(),
        source_type: "terminal".to_owned(),
        source: Some("grafiki-terminal".to_owned()),
        title: Some("Hosted terminal session".to_owned()),
        text: Some(text),
        payload: None,
        metadata: None,
        privacy_level: None,
        redacted: false,
        captured_at: None,
    });
}

/// Strip ANSI/VT escape sequences and carriage returns from raw terminal output,
/// leaving readable text for extraction. Deterministic, no dependency.
fn strip_ansi(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b => {
                i += 1;
                match bytes.get(i) {
                    Some(b'[') => {
                        // CSI: parameters/intermediates until a final byte 0x40..=0x7e.
                        i += 1;
                        while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                            i += 1;
                        }
                        i += 1;
                    }
                    Some(b']') => {
                        // OSC: until BEL (0x07) or ST (ESC \).
                        i += 1;
                        while i < bytes.len() && bytes[i] != 0x07 {
                            if bytes[i] == 0x1b
                                && bytes.get(i + 1) == Some(&b'\\')
                            {
                                i += 1;
                                break;
                            }
                            i += 1;
                        }
                        i += 1;
                    }
                    _ => i += 1, // other 2-byte escape
                }
            }
            b'\r' => i += 1, // drop CR, keep LF
            _ => {
                let start = i;
                while i < bytes.len() && bytes[i] != 0x1b && bytes[i] != b'\r' {
                    i += 1;
                }
                out.push_str(&String::from_utf8_lossy(&bytes[start..i]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn strips_color_and_cursor_sequences_but_keeps_text() {
        // "\x1b[31mError:\x1b[0m disk full\r\n" + a cursor move.
        let raw = b"\x1b[31mError:\x1b[0m disk full\r\n\x1b[2Kok\n";
        let out = strip_ansi(raw);
        assert!(out.contains("Error: disk full"));
        assert!(out.contains("ok"));
        assert!(!out.contains('\x1b'));
        assert!(!out.contains('\r'));
    }
}
