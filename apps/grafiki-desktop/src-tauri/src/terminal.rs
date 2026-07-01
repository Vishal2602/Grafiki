//! Hosted terminal: Grafiki spawns a real shell/agent in a PTY, renders it in the
//! UI (xterm.js), and TEES the output into Grafiki's capture pipeline. Because we
//! own the PTY we see every byte and know the folder — so a session run inside
//! Grafiki (e.g. `claude`) is captured automatically, no daemon or transcript
//! discovery. Captured output → `capture_events` → the usual extraction/review.
//!
//! Sessions are DETACHED, not owned by the UI: the PTY keeps running (and keeps
//! being captured) when the pane unmounts — switching tabs must never kill the
//! agent. The UI attaches/detaches an output channel; on reattach the scrollback
//! buffer is replayed so the terminal picks up where it left off. Only an explicit
//! `terminal_close` (or child exit) ends a session.

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

/// Cap on the replayable scrollback kept per session (raw bytes incl. ANSI).
const SCROLLBACK_MAX: usize = 512 * 1024;
/// Capture is flushed to `capture_events` whenever this much output accumulates.
const CAPTURE_FLUSH_THRESHOLD: usize = 64 * 1024;

/// State shared between the reader thread and the Tauri commands. One mutex
/// guards scrollback + the attached channel so replay-then-attach is atomic
/// (no byte can slip between the replayed snapshot and the live stream).
struct TermShared {
    scrollback: Vec<u8>,
    channel: Option<Channel<Vec<u8>>>,
    capture: Vec<u8>,
    exited: bool,
}

impl TermShared {
    /// Append output, trimming the front of the scrollback (to a newline
    /// boundary, so a replay doesn't start mid escape sequence) once over cap.
    fn push_scrollback(&mut self, bytes: &[u8]) {
        self.scrollback.extend_from_slice(bytes);
        if self.scrollback.len() > SCROLLBACK_MAX {
            let overflow = self.scrollback.len() - SCROLLBACK_MAX;
            let cut = self.scrollback[overflow..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|pos| overflow + pos + 1)
                .unwrap_or(overflow);
            self.scrollback.drain(..cut);
        }
    }
}

/// A live hosted terminal session.
struct TerminalSession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Grafiki capture session id (`None` when the folder isn't a Grafiki project).
    capture_id: Option<String>,
    project_root: String,
    shared: Arc<Mutex<TermShared>>,
}

/// Tauri managed state: all live terminal sessions by id.
#[derive(Default)]
pub struct TerminalRegistry(Mutex<HashMap<String, TerminalSession>>);

/// What `terminal_attach` tells the UI about a session it asked for.
#[derive(serde::Serialize)]
pub struct AttachReply {
    pub found: bool,
    pub exited: bool,
    pub cwd: String,
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Replay the scrollback through `channel` and install it as the live output
/// sink, atomically with respect to the reader thread.
fn attach_channel(shared: &Arc<Mutex<TermShared>>, channel: Channel<Vec<u8>>) -> bool {
    let mut state = shared.lock().unwrap();
    if !state.scrollback.is_empty() && channel.send(state.scrollback.clone()).is_err() {
        return false;
    }
    let exited = state.exited;
    state.channel = Some(channel);
    !exited
}

/// Open a hosted terminal: spawn `command` in `cwd` inside a PTY, stream its bytes
/// to `on_output`, and capture the session into `cwd`'s Grafiki project (if any).
/// If a LIVE session with this id already exists, reattach to it instead (open is
/// idempotent — a double mount must never spawn or kill anything).
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
    {
        let mut sessions = registry.0.lock().unwrap();
        if let Some(existing) = sessions.get(&id) {
            if !existing.shared.lock().unwrap().exited {
                attach_channel(&existing.shared, on_output);
                return Ok(id);
            }
            // Exited leftover under this id: drop it and spawn fresh below.
            let stale = sessions.remove(&id);
            drop(sessions);
            if let Some(stale) = stale {
                finish_session(stale);
            }
        }
    }

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

    let shared = Arc::new(Mutex::new(TermShared {
        scrollback: Vec::new(),
        channel: Some(on_output),
        capture: Vec::new(),
        exited: false,
    }));

    // Reader thread: drain the PTY for the session's whole life — buffering
    // scrollback + teeing capture even while no UI is attached. A send failure
    // only detaches the channel; it never stops the session.
    {
        let shared = shared.clone();
        let capture_id = capture_id.clone();
        let project_root = cwd.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break, // child exited or the pty closed
                    Ok(n) => {
                        let bytes = &chunk[..n];
                        let flush = {
                            let mut state = shared.lock().unwrap();
                            state.push_scrollback(bytes);
                            if let Some(channel) = &state.channel {
                                if channel.send(bytes.to_vec()).is_err() {
                                    state.channel = None; // UI went away; keep draining
                                }
                            }
                            if capture_id.is_some() {
                                state.capture.extend_from_slice(bytes);
                                if state.capture.len() > CAPTURE_FLUSH_THRESHOLD {
                                    Some(std::mem::take(&mut state.capture))
                                } else {
                                    None
                                }
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
            // Session over: mark exited, tell any attached UI, flush the tail of
            // the capture and close the capture session.
            let remainder = {
                let mut state = shared.lock().unwrap();
                state.exited = true;
                let marker = b"\r\n\x1b[2m[session ended]\x1b[0m\r\n";
                state.push_scrollback(marker);
                if let Some(channel) = &state.channel {
                    let _ = channel.send(marker.to_vec());
                }
                std::mem::take(&mut state.capture)
            };
            if let Some(capture) = capture_id {
                flush_capture(&project_root, &Some(capture.clone()), remainder);
                let _ = stop_capture_session(StopCaptureOptions {
                    project_name: None,
                    start_dir: PathBuf::from(&project_root),
                    grafiki_home: None,
                    capture_id: capture,
                });
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
            shared,
        },
    );
    Ok(id)
}

/// Reattach the UI to an existing session: replay its scrollback through
/// `on_output`, then stream live bytes. `found: false` means no such session
/// (the caller can revive or start fresh).
#[tauri::command]
pub fn terminal_attach(
    registry: State<TerminalRegistry>,
    id: String,
    on_output: Channel<Vec<u8>>,
) -> Result<AttachReply, String> {
    let sessions = registry.0.lock().unwrap();
    match sessions.get(&id) {
        Some(session) => {
            let alive = attach_channel(&session.shared, on_output);
            Ok(AttachReply {
                found: true,
                exited: !alive,
                cwd: session.project_root.clone(),
            })
        }
        None => Ok(AttachReply {
            found: false,
            exited: false,
            cwd: String::new(),
        }),
    }
}

/// Detach the UI from a session WITHOUT stopping it: the PTY keeps running and
/// being captured in the background. Called when the pane unmounts (tab switch).
#[tauri::command]
pub fn terminal_detach(registry: State<TerminalRegistry>, id: String) -> Result<(), String> {
    let sessions = registry.0.lock().unwrap();
    if let Some(session) = sessions.get(&id) {
        session.shared.lock().unwrap().channel = None;
    }
    Ok(())
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

/// EXPLICITLY end a session: kill the child, flush the remaining output to
/// capture, and stop the capture session. This is a user action ("End session"),
/// never a side effect of navigation.
#[tauri::command]
pub fn terminal_close(registry: State<TerminalRegistry>, id: String) -> Result<(), String> {
    let session = registry.0.lock().unwrap().remove(&id);
    if let Some(session) = session {
        finish_session(session);
    }
    Ok(())
}

/// Kill + flush + stop-capture for a session that is being discarded. The reader
/// thread also flushes on EOF, but `ingest_capture_event` dedups by content hash,
/// so double flushing the same bytes is harmless.
fn finish_session(mut session: TerminalSession) {
    let _ = session.child.kill();
    let raw = std::mem::take(&mut session.shared.lock().unwrap().capture);
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
    use super::{strip_ansi, TermShared, SCROLLBACK_MAX};

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

    #[test]
    fn scrollback_caps_and_trims_to_newline_boundary() {
        let mut shared = TermShared {
            scrollback: Vec::new(),
            channel: None,
            capture: Vec::new(),
            exited: false,
        };
        // Fill well past the cap with recognizable lines.
        for i in 0..40_000 {
            shared.push_scrollback(format!("line {i}\n").as_bytes());
        }
        assert!(shared.scrollback.len() <= SCROLLBACK_MAX);
        // The buffer starts at a line boundary (not mid-line).
        let text = String::from_utf8_lossy(&shared.scrollback);
        assert!(text.starts_with("line "));
        // The newest line is retained.
        assert!(text.ends_with("line 39999\n"));
    }
}
