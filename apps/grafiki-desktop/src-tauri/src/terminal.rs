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
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use grafiki_core::{
    ingest_capture_event, load_capture_config, read_live_transcript, redact_text,
    start_capture_session, stop_capture_session, CaptureConfigOptions, IngestCaptureEventOptions,
    LiveTranscriptTurn, StartCaptureOptions, StopCaptureOptions,
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tauri::State;

/// Cap on the replayable scrollback kept per session (raw bytes incl. ANSI).
const SCROLLBACK_MAX: usize = 512 * 1024;
/// Capture is flushed to `capture_events` whenever this much output accumulates.
const CAPTURE_FLUSH_THRESHOLD: usize = 64 * 1024;
/// Trailing raw bytes held back (not flushed) at each full-capture flush and
/// carried into the next accumulation. `redact_text` works line-by-line, so a
/// flush that lands mid-secret — the key on one side of the 64 KiB boundary,
/// the value on the other — would otherwise redact each side independently
/// and let the half with no key-bearing context through in the clear. Holding
/// back a trailing window reunites a boundary-straddling secret in one
/// redaction call as long as it starts within the window.
const REDACT_CARRY: usize = 4 * 1024;
/// How much (ANSI-stripped) tail is persisted to disk for cross-relaunch resume.
const RESUME_TAIL_MAX: usize = 32 * 1024;
/// Machine-readable exit signal sent through the output channel when the child
/// dies — an OSC sequence xterm renders as nothing, matched verbatim by the UI
/// (App.tsx) to flip the session header to "ended" in real time.
pub const GRAFIKI_EXIT_SENTINEL: &str = "\x1b]7777;grafiki-session-exited\x07";
/// App-level file (under the Grafiki home dir) holding resumable session
/// descriptors — the terminal's equivalent of an editor's session store.
const DESCRIPTOR_FILE: &str = "terminal_sessions.json";

/// What survives an app relaunch: enough to re-open a shell in the same folder,
/// show the previous output, and resume the agent (`claude --continue`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SessionDescriptor {
    id: String,
    cwd: String,
    /// The agent command originally launched ("" = plain shell).
    launch: String,
    /// Capture session that owns memories produced by this terminal launch.
    #[serde(default)]
    capture_id: Option<String>,
    /// Consent mode at the time this descriptor was written.
    #[serde(default)]
    capture_mode: String,
    /// ANSI-stripped tail of the session output, replayed on revive.
    #[serde(default)]
    tail: String,
    #[serde(default)]
    updated_at: u64,
}

/// Serializes read-modify-write cycles on the descriptor file across the
/// commands and every session's reader thread.
static DESCRIPTOR_LOCK: Mutex<()> = Mutex::new(());

fn descriptor_path() -> Option<PathBuf> {
    grafiki_core::grafiki_home()
        .ok()
        .map(|home| home.join(DESCRIPTOR_FILE))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn load_descriptors(path: &PathBuf) -> HashMap<String, SessionDescriptor> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Upsert (or with `descriptor: None`, remove) one session's descriptor.
/// Best-effort: persistence must never disrupt the live terminal.
fn store_descriptor(id: &str, descriptor: Option<SessionDescriptor>) {
    let Some(path) = descriptor_path() else {
        return;
    };
    let _guard = DESCRIPTOR_LOCK.lock().unwrap();
    let mut all = load_descriptors(&path);
    match descriptor {
        Some(descriptor) => {
            all.insert(id.to_owned(), descriptor);
        }
        None => {
            all.remove(id);
        }
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    if let Ok(json) = serde_json::to_string_pretty(&all) {
        // Write to a sibling temp file, then rename it over the target. A
        // truncate-and-write in place leaves a corrupt (or empty) store if
        // the process dies mid-write; a same-directory rename is atomic, so
        // a crash here always leaves either the old file or the new one, never
        // a partial one.
        let tmp_path = path.with_extension("tmp");
        #[cfg(unix)]
        let result = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&tmp_path)
                .and_then(|mut file| file.write_all(json.as_bytes()))
                .and_then(|()| std::fs::rename(&tmp_path, &path))
        };
        #[cfg(not(unix))]
        let result =
            std::fs::write(&tmp_path, json).and_then(|()| std::fs::rename(&tmp_path, &path));
        let _ = result;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureMode {
    Off,
    Digest,
    Full,
}

impl CaptureMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Digest => "digest",
            Self::Full => "full",
        }
    }

    fn allows_resume_tail(self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Debug, Default)]
struct DigestBuffer {
    observed_bytes: usize,
    observed_lines: usize,
}

impl DigestBuffer {
    fn push(&mut self, bytes: &[u8]) {
        self.observed_bytes = self.observed_bytes.saturating_add(bytes.len());
        self.observed_lines = self
            .observed_lines
            .saturating_add(bytes.iter().filter(|byte| **byte == b'\n').count());
    }

    fn take(&mut self) -> Option<String> {
        if self.observed_bytes == 0 {
            return None;
        }
        let observed = std::mem::take(&mut self.observed_bytes);
        let lines = std::mem::take(&mut self.observed_lines);
        Some(format!(
            "Terminal digest: {observed} output bytes across approximately {lines} lines were observed. No terminal output sample or full output was retained in digest mode."
        ))
    }
}

fn load_descriptor(id: &str) -> Option<SessionDescriptor> {
    let path = descriptor_path()?;
    let _guard = DESCRIPTOR_LOCK.lock().unwrap();
    load_descriptors(&path).remove(id)
}

/// A live (not exited) hosted session, summarized for the Home ledger.
#[derive(serde::Serialize, Clone)]
pub struct LiveTerminalInfo {
    pub id: String,
    pub launch: String,
    pub cwd: String,
    /// Last few non-empty output lines, ANSI-stripped (the Home card preview).
    pub tail: String,
    pub capturing: bool,
    /// User-facing reason capture is off (`None` while capturing). Surfaced on
    /// the Home live card so the off state isn't a bare, unexplained label
    /// (2026-07-04 don-norman-design-critic finding).
    pub capture_hint: Option<String>,
    pub capture_id: Option<String>,
    pub capture_mode: String,
}

/// Snapshot every live session (for Home's live-session card).
pub fn live_sessions(registry: &TerminalRegistry) -> Vec<LiveTerminalInfo> {
    let sessions = registry.0.lock().unwrap();
    sessions
        .iter()
        .filter_map(|(id, session)| {
            let state = session.shared.lock().unwrap();
            if state.exited {
                return None;
            }
            let tail = if session.capture_mode.allows_resume_tail() {
                let scrollback = &state.scrollback;
                let start = scrollback.len().saturating_sub(600);
                let text = redact_text(&strip_ansi(&scrollback[start..])).0;
                let mut lines: Vec<&str> = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                lines.split_off(lines.len().saturating_sub(3)).join("\n")
            } else {
                String::new()
            };
            Some(LiveTerminalInfo {
                id: id.clone(),
                launch: session.launch.clone(),
                cwd: session.project_root.clone(),
                tail,
                capturing: session.capture_id.is_some(),
                capture_hint: session.capture_hint.clone(),
                capture_id: session.capture_id.clone(),
                capture_mode: session.capture_mode.as_str().to_owned(),
            })
        })
        .collect()
}

/// The most recently updated on-disk session descriptor — what "Resume last
/// session" on Home points at after an app relaunch.
#[derive(serde::Serialize, Clone)]
pub struct ResumableInfo {
    pub id: String,
    pub launch: String,
    pub cwd: String,
    pub updated_at: u64,
    pub capture_id: Option<String>,
    pub capture_mode: String,
}

pub fn latest_resumable() -> Option<ResumableInfo> {
    let path = descriptor_path()?;
    let _guard = DESCRIPTOR_LOCK.lock().unwrap();
    load_descriptors(&path)
        .into_values()
        .max_by_key(|descriptor| descriptor.updated_at)
        .map(|descriptor| ResumableInfo {
            id: descriptor.id,
            launch: descriptor.launch,
            cwd: descriptor.cwd,
            updated_at: descriptor.updated_at,
            capture_id: descriptor.capture_id,
            capture_mode: descriptor.capture_mode,
        })
}

/// Refresh a session's persisted tail from its current scrollback.
fn persist_tail(
    id: &str,
    cwd: &str,
    launch: &str,
    capture_id: &Option<String>,
    capture_mode: CaptureMode,
    shared: &Arc<Mutex<TermShared>>,
) {
    let tail = if capture_mode.allows_resume_tail() {
        let state = shared.lock().unwrap();
        let scrollback = &state.scrollback;
        let start = scrollback.len().saturating_sub(RESUME_TAIL_MAX);
        let stripped = strip_ansi(&scrollback[start..]);
        redact_text(&stripped).0
    } else {
        String::new()
    };
    store_descriptor(
        id,
        Some(SessionDescriptor {
            id: id.to_owned(),
            cwd: cwd.to_owned(),
            launch: launch.to_owned(),
            capture_id: capture_id.clone(),
            capture_mode: capture_mode.as_str().to_owned(),
            tail,
            updated_at: unix_now(),
        }),
    );
}

/// State shared between the reader thread and the Tauri commands. One mutex
/// guards scrollback + the attached channel so replay-then-attach is atomic
/// (no byte can slip between the replayed snapshot and the live stream).
struct TermShared {
    scrollback: Vec<u8>,
    channel: Option<Channel<Vec<u8>>>,
    capture: Vec<u8>,
    digest: DigestBuffer,
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

/// Consent gate and storage mode for hosted-terminal output capture.
/// The default config ships `terminal_output: "off"`, so capture is opt-in.
fn capture_policy(cwd: &str) -> Result<CaptureMode, String> {
    match load_capture_config(CaptureConfigOptions {
        project_name: None,
        start_dir: PathBuf::from(cwd),
        grafiki_home: None,
    }) {
        Ok(report) if !report.config.sources.terminal || report.config.terminal_output == "off" => {
            Err("terminal capture is off in Settings".to_owned())
        }
        Ok(report) if report.config.terminal_output == "digest" => Ok(CaptureMode::Digest),
        Ok(_) => Ok(CaptureMode::Full),
        Err(_) => Err("initialize this folder in Settings".to_owned()),
    }
}

fn transcript_signature(turn: &LiveTranscriptTurn) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        turn.role,
        turn.timestamp.as_deref().unwrap_or_default(),
        turn.text
    )
}

/// A live hosted terminal session.
struct TerminalSession {
    /// Own mutex, SEPARATE from the registry's — writing to a PTY is a
    /// blocking syscall (a slow-to-start shell/agent, or a burst of typed
    /// input, can stall it for real time). Holding the registry lock during
    /// that write would freeze every other terminal command AND the
    /// `live_sessions()` background poll for as long as the write blocks —
    /// this is exactly what happened in the first real fresh-install test
    /// (2026-07-04): the whole app went unresponsive. Look up this Arc under
    /// the registry lock, then drop that lock before writing.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Grafiki capture session id (`None` when the folder isn't a Grafiki project).
    capture_id: Option<String>,
    /// User-facing reason capture is off (`None` while capturing).
    capture_hint: Option<String>,
    capture_mode: CaptureMode,
    project_root: String,
    /// The agent command this session was started for ("" = plain shell).
    launch: String,
    /// Last transcript turn that existed before this terminal launched. The chat
    /// lens uses it as a boundary so an old project transcript never appears as
    /// this session's conversation.
    transcript_baseline: Option<String>,
    shared: Arc<Mutex<TermShared>>,
}

/// Tauri managed state: all live terminal sessions by id.
#[derive(Default)]
pub struct TerminalRegistry(Mutex<HashMap<String, TerminalSession>>);

/// Return only transcript turns attributable to one hosted terminal. Claude's
/// project transcript directory does not expose our terminal id, so we bind by
/// the last turn that existed before launch. When two live Claude terminals use
/// the same project the mapping is ambiguous; returning an explicit error is
/// safer than showing one session's private conversation in another tab.
pub fn session_live_transcript(
    registry: &TerminalRegistry,
    id: &str,
    start_dir: &std::path::Path,
) -> Result<Vec<LiveTranscriptTurn>, String> {
    let (baseline, project_root) = {
        let sessions = registry.0.lock().unwrap();
        let Some(session) = sessions.get(id) else {
            return Err("This terminal session is no longer running.".to_owned());
        };
        if !session.launch.trim().starts_with("claude") {
            return Ok(Vec::new());
        }
        let same_project_claude = sessions
            .values()
            .filter(|candidate| {
                !candidate.shared.lock().unwrap().exited
                    && candidate.project_root == session.project_root
                    && candidate.launch.trim().starts_with("claude")
            })
            .count();
        if same_project_claude > 1 {
            return Err(
                "Chat view is disabled while multiple Claude sessions run in this project; use Terminal so conversations cannot be mixed."
                    .to_owned(),
            );
        }
        (
            session.transcript_baseline.clone(),
            session.project_root.clone(),
        )
    };

    if std::path::Path::new(&project_root) != start_dir && !start_dir.as_os_str().is_empty() {
        return Err("The requested transcript does not belong to this project.".to_owned());
    }
    let turns = read_live_transcript(&PathBuf::from(project_root), 500)
        .map_err(|error| error.to_string())?;
    let session_turns = if let Some(baseline) = baseline {
        match turns
            .iter()
            .rposition(|turn| transcript_signature(turn) == baseline)
        {
            Some(index) => turns.into_iter().skip(index + 1).collect::<Vec<_>>(),
            // A new transcript file replaced the pre-launch newest file.
            None => turns,
        }
    } else {
        turns
    };
    let keep_from = session_turns.len().saturating_sub(80);
    Ok(session_turns.into_iter().skip(keep_from).collect())
}

/// What `terminal_attach` tells the UI about a session it asked for.
#[derive(serde::Serialize)]
pub struct AttachReply {
    pub found: bool,
    pub exited: bool,
    pub cwd: String,
    /// A Grafiki capture session is recording this terminal.
    pub capturing: bool,
    /// User-facing reason capture is off (`None` while capturing).
    pub capture_hint: Option<String>,
    pub capture_id: Option<String>,
    pub capture_mode: String,
}

/// What `terminal_open` (and a revive's spawn) tells the UI.
#[derive(serde::Serialize)]
pub struct OpenReply {
    pub id: String,
    /// A Grafiki capture session is recording this terminal (`false` when the
    /// folder isn't an initialized Grafiki project or capture consent is off).
    pub capturing: bool,
    /// User-facing reason capture is off (`None` while capturing).
    pub capture_hint: Option<String>,
    pub capture_id: Option<String>,
    pub capture_mode: String,
}

/// What `terminal_revive` tells the UI about a disk-restored session.
#[derive(serde::Serialize)]
pub struct ReviveReply {
    pub found: bool,
    pub launch: String,
    pub cwd: String,
    pub capturing: bool,
    pub capture_hint: Option<String>,
    pub capture_id: Option<String>,
    pub capture_mode: String,
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
/// `launch` is the agent the UI will type into the shell (recorded for resume).
/// If a LIVE session with this id already exists, reattach to it instead (open is
/// idempotent — a double mount must never spawn or kill anything).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn terminal_open(
    registry: State<TerminalRegistry>,
    id: String,
    cwd: String,
    command: String,
    launch: String,
    rows: u16,
    cols: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<OpenReply, String> {
    spawn_session(
        &registry, id, cwd, command, launch, rows, cols, on_output, None,
    )
}

/// Revive a session from its on-disk descriptor after an app relaunch: re-open a
/// shell in the same folder and replay the previous output as a dimmed preamble.
/// The frontend then types the agent's own resume command (`claude --continue`).
/// `found: false` means there is nothing to revive (never opened / explicitly
/// ended) — the caller shows the launcher.
#[tauri::command]
pub fn terminal_revive(
    registry: State<TerminalRegistry>,
    id: String,
    rows: u16,
    cols: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<ReviveReply, String> {
    let Some(descriptor) = load_descriptor(&id) else {
        return Ok(ReviveReply {
            found: false,
            launch: String::new(),
            cwd: String::new(),
            capturing: false,
            capture_hint: None,
            capture_id: None,
            capture_mode: CaptureMode::Off.as_str().to_owned(),
        });
    };
    let mut preamble = Vec::new();
    let tail_replay_allowed = capture_policy(&descriptor.cwd).is_ok()
        && matches!(descriptor.capture_mode.as_str(), "digest" | "full");
    if tail_replay_allowed && !descriptor.tail.trim().is_empty() {
        let redacted_tail = redact_text(&descriptor.tail).0;
        preamble.extend_from_slice(
            b"\x1b[2m\xe2\x94\x80\xe2\x94\x80 previous session \xe2\x94\x80\xe2\x94\x80\x1b[0m\r\n",
        );
        preamble.extend_from_slice(redacted_tail.replace('\n', "\r\n").as_bytes());
        preamble.extend_from_slice(b"\r\n\x1b[2m\xe2\x94\x80\xe2\x94\x80 end of previous session \xe2\x94\x80\xe2\x94\x80 resuming\x1b[0m\r\n");
    }
    let opened = spawn_session(
        &registry,
        id,
        descriptor.cwd.clone(),
        String::new(),
        descriptor.launch.clone(),
        rows,
        cols,
        on_output,
        Some(preamble),
    )?;
    Ok(ReviveReply {
        found: true,
        launch: descriptor.launch,
        cwd: descriptor.cwd,
        capturing: opened.capturing,
        capture_hint: opened.capture_hint,
        capture_id: opened.capture_id,
        capture_mode: opened.capture_mode,
    })
}

/// Shared spawn path for `terminal_open` and `terminal_revive`.
#[allow(clippy::too_many_arguments)]
fn spawn_session(
    registry: &State<TerminalRegistry>,
    id: String,
    cwd: String,
    command: String,
    launch: String,
    rows: u16,
    cols: u16,
    on_output: Channel<Vec<u8>>,
    preamble: Option<Vec<u8>>,
) -> Result<OpenReply, String> {
    {
        let mut sessions = registry.0.lock().unwrap();
        if let Some(existing) = sessions.get(&id) {
            if !existing.shared.lock().unwrap().exited {
                attach_channel(&existing.shared, on_output);
                let capturing = existing.capture_id.is_some();
                let capture_hint = existing.capture_hint.clone();
                return Ok(OpenReply {
                    id,
                    capturing,
                    capture_hint,
                    capture_id: existing.capture_id.clone(),
                    capture_mode: existing.capture_mode.as_str().to_owned(),
                });
            }
            // Exited leftover under this id: drop it and spawn fresh below.
            let stale = sessions.remove(&id);
            drop(sessions);
            if let Some(stale) = stale {
                finish_session(stale);
            }
        }
    }

    // No project folder configured yet → the user's home dir, never "" (an
    // empty cwd makes the PTY spawn fail or land in an undefined directory).
    let cwd = if cwd.trim().is_empty() {
        std::env::var_os("HOME")
            .map(|home| home.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_owned())
    } else {
        cwd
    };

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

    // Best-effort capture session for this folder — but ONLY with the user's
    // consent (Settings → Capture Consent; the default config says off). Skipped
    // when it isn't a Grafiki project. The terminal itself always works —
    // capture is additive, never blocking.
    let (capture_id, capture_mode, capture_hint) = match capture_policy(&cwd) {
        Ok(mode) => match start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: PathBuf::from(&cwd),
            grafiki_home: None,
            scope: String::new(),
            source_app: Some("grafiki-terminal".to_owned()),
            consent_profile: None,
            redaction_profile: None,
        }) {
            Ok(report) => (Some(report.capture.id), mode, None),
            Err(_) => (
                None,
                CaptureMode::Off,
                Some("capture could not start for this folder".to_owned()),
            ),
        },
        Err(hint) => (None, CaptureMode::Off, Some(hint)),
    };

    let transcript_baseline = if launch.trim().starts_with("claude") {
        read_live_transcript(&PathBuf::from(&cwd), 500)
            .ok()
            .and_then(|turns| turns.last().map(transcript_signature))
    } else {
        None
    };

    let preamble = preamble.unwrap_or_default();
    let shared = Arc::new(Mutex::new(TermShared {
        scrollback: preamble.clone(),
        channel: Some(on_output),
        capture: Vec::new(),
        digest: DigestBuffer::default(),
        exited: false,
    }));
    // Show the revive preamble before any live output (the reader thread only
    // starts sending after this, so ordering holds).
    if !preamble.is_empty() {
        let state = shared.lock().unwrap();
        if let Some(channel) = &state.channel {
            let _ = channel.send(preamble);
        }
    }

    // Reader thread: drain the PTY for the session's whole life — buffering
    // scrollback + teeing capture even while no UI is attached. A send failure
    // only detaches the channel; it never stops the session.
    {
        let shared = shared.clone();
        // Mutable: re-checked against the live capture policy on every flush so
        // a mid-session "capture off" in Settings actually stops persistence
        // instead of the reader thread running for the session's whole life on
        // the consent it was spawned with.
        let mut capture_id = capture_id.clone();
        let mut capture_mode_for_reader = capture_mode;
        let project_root = cwd.clone();
        let id = id.clone();
        let launch = launch.clone();
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
                                match capture_mode_for_reader {
                                    CaptureMode::Full => {
                                        state.capture.extend_from_slice(bytes);
                                        take_capture_for_flush(&mut state.capture)
                                            .map(CaptureFlush::Full)
                                    }
                                    CaptureMode::Digest => {
                                        state.digest.push(bytes);
                                        (state.digest.observed_bytes > CAPTURE_FLUSH_THRESHOLD)
                                            .then(|| state.digest.take())
                                            .flatten()
                                            .map(CaptureFlush::Digest)
                                    }
                                    CaptureMode::Off => None,
                                }
                            } else {
                                None
                            }
                        };
                        if let Some(flush) = flush {
                            // Re-read the live capture policy before persisting —
                            // a mid-session "capture off" in Settings must stop
                            // persistence, not just be honored on the next
                            // terminal_open. Re-reading once per ~64 KiB flush is
                            // cheap; don't thrash the config file per byte.
                            let still_capturing =
                                capture_id.is_some() && capture_policy(&project_root).is_ok();
                            if still_capturing {
                                flush_capture(&project_root, &capture_id, flush);
                                // Piggyback resume-tail persistence on the capture
                                // cadence so a hard app quit loses little context.
                                persist_tail(
                                    &id,
                                    &project_root,
                                    &launch,
                                    &capture_id,
                                    capture_mode_for_reader,
                                    &shared,
                                );
                            } else if let Some(stopped) = capture_id.take() {
                                // Capture was turned off mid-session: drop this
                                // already-buffered chunk instead of persisting it,
                                // finalize the capture session once, and flip to
                                // Off so no further flush is even attempted.
                                let _ = stop_capture_session(StopCaptureOptions {
                                    project_name: None,
                                    start_dir: PathBuf::from(&project_root),
                                    grafiki_home: None,
                                    capture_id: stopped,
                                });
                                capture_mode_for_reader = CaptureMode::Off;
                                persist_tail(
                                    &id,
                                    &project_root,
                                    &launch,
                                    &capture_id,
                                    capture_mode_for_reader,
                                    &shared,
                                );
                            }
                        }
                    }
                }
            }
            // Session over: mark exited, tell any attached UI, flush the tail of
            // the capture and close the capture session.
            let remainder = {
                let mut state = shared.lock().unwrap();
                state.exited = true;
                // Machine-readable exit sentinel first — an OSC xterm renders as
                // nothing, but the UI flips its header/composer to "ended" on it.
                // Without this the status dot stayed "live" while keystrokes fell
                // into a dead PTY (I/O error swallowed by a void invoke).
                let sentinel = GRAFIKI_EXIT_SENTINEL.as_bytes();
                state.push_scrollback(sentinel);
                let marker = b"\r\n\x1b[2m[session ended]\x1b[0m\r\n";
                state.push_scrollback(marker);
                if let Some(channel) = &state.channel {
                    let _ = channel.send(sentinel.to_vec());
                    let _ = channel.send(marker.to_vec());
                }
                match capture_mode_for_reader {
                    CaptureMode::Full => {
                        Some(CaptureFlush::Full(std::mem::take(&mut state.capture)))
                    }
                    CaptureMode::Digest => state.digest.take().map(CaptureFlush::Digest),
                    CaptureMode::Off => None,
                }
            };
            persist_tail(
                &id,
                &project_root,
                &launch,
                &capture_id,
                capture_mode_for_reader,
                &shared,
            );
            if let Some(capture) = capture_id {
                if let Some(remainder) = remainder {
                    flush_capture(&project_root, &Some(capture.clone()), remainder);
                }
                let _ = stop_capture_session(StopCaptureOptions {
                    project_name: None,
                    start_dir: PathBuf::from(&project_root),
                    grafiki_home: None,
                    capture_id: capture,
                });
            }
        });
    }

    // Persist the descriptor immediately so even a session that quits without
    // producing output can be revived into its folder.
    persist_tail(&id, &cwd, &launch, &capture_id, capture_mode, &shared);

    let capturing = capture_id.is_some();
    let reply_capture_id = capture_id.clone();
    registry.0.lock().unwrap().insert(
        id.clone(),
        TerminalSession {
            writer: Arc::new(Mutex::new(writer)),
            master,
            child,
            capture_id,
            capture_hint: capture_hint.clone(),
            capture_mode,
            project_root: cwd,
            launch,
            transcript_baseline,
            shared,
        },
    );
    Ok(OpenReply {
        id,
        capturing,
        capture_hint,
        capture_id: reply_capture_id,
        capture_mode: capture_mode.as_str().to_owned(),
    })
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
                capturing: session.capture_id.is_some(),
                capture_hint: session.capture_hint.clone(),
                capture_id: session.capture_id.clone(),
                capture_mode: session.capture_mode.as_str().to_owned(),
            })
        }
        None => Ok(AttachReply {
            found: false,
            exited: false,
            cwd: String::new(),
            capturing: false,
            capture_hint: None,
            capture_id: None,
            capture_mode: CaptureMode::Off.as_str().to_owned(),
        }),
    }
}

/// Detach the UI from a session WITHOUT stopping it: the PTY keeps running and
/// being captured in the background. Called when the pane unmounts (tab switch).
/// Also refreshes the on-disk resume tail — a tab-away is the last reliable
/// moment before a possible app quit.
#[tauri::command]
pub fn terminal_detach(registry: State<TerminalRegistry>, id: String) -> Result<(), String> {
    let persist = {
        let sessions = registry.0.lock().unwrap();
        sessions.get(&id).map(|session| {
            session.shared.lock().unwrap().channel = None;
            (
                session.project_root.clone(),
                session.launch.clone(),
                session.capture_id.clone(),
                session.capture_mode,
                session.shared.clone(),
            )
        })
    };
    if let Some((cwd, launch, capture_id, capture_mode, shared)) = persist {
        persist_tail(&id, &cwd, &launch, &capture_id, capture_mode, &shared);
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
    // Only the lookup happens under the registry lock; the Arc clone is cheap.
    // The actual write (a blocking syscall) happens on the session's OWN
    // writer mutex, so a slow child process can only ever stall writes to
    // ITS OWN session, never the registry or any other session.
    let writer = {
        let sessions = registry.0.lock().unwrap();
        sessions.get(&id).map(|session| session.writer.clone())
    };
    // A missing session must be an error, not a silent no-op: the UI clears
    // the composer as soon as this call returns Ok, so an `Ok(())` here for a
    // session that no longer exists tells the user their input landed when it
    // was actually dropped on the floor.
    let Some(writer) = writer else {
        return Err(format!("terminal session {id} is no longer running"));
    };
    let mut writer = writer.lock().unwrap();
    writer
        .write_all(data.as_bytes())
        .map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())?;
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
    // Explicitly ended sessions are not resumable.
    store_descriptor(&id, None);
    Ok(())
}

/// Kill + flush + stop-capture for a session that is being discarded. The reader
/// thread also flushes on EOF, but `ingest_capture_event` dedups by content hash,
/// so double flushing the same bytes is harmless.
fn finish_session(mut session: TerminalSession) {
    let _ = session.child.kill();
    let flush = {
        let mut shared = session.shared.lock().unwrap();
        match session.capture_mode {
            CaptureMode::Full => Some(CaptureFlush::Full(std::mem::take(&mut shared.capture))),
            CaptureMode::Digest => shared.digest.take().map(CaptureFlush::Digest),
            CaptureMode::Off => None,
        }
    };
    if let Some(capture_id) = session.capture_id.clone() {
        if let Some(flush) = flush {
            flush_capture(&session.project_root, &session.capture_id, flush);
        }
        let _ = stop_capture_session(StopCaptureOptions {
            project_name: None,
            start_dir: PathBuf::from(&session.project_root),
            grafiki_home: None,
            capture_id,
        });
    }
}

/// Split a full-capture accumulator at a flush point, holding back the last
/// `REDACT_CARRY` bytes (raw, unredacted) in `capture` so a secret whose key
/// falls in that trailing window survives into the next accumulation instead
/// of being redacted (or not) independently of its value. Returns `None`
/// until `capture` exceeds the flush threshold.
fn take_capture_for_flush(capture: &mut Vec<u8>) -> Option<Vec<u8>> {
    if capture.len() <= CAPTURE_FLUSH_THRESHOLD {
        return None;
    }
    let split_at = capture.len().saturating_sub(REDACT_CARRY);
    Some(capture.drain(..split_at).collect())
}

enum CaptureFlush {
    Full(Vec<u8>),
    Digest(String),
}

/// Persist a chunk of terminal output as a capture event (ANSI-stripped). Silent
/// on error — capture must never disrupt the live terminal.
fn flush_capture(project_root: &str, capture_id: &Option<String>, flush: CaptureFlush) {
    let (text, title, already_redacted) = match flush {
        CaptureFlush::Full(raw) => {
            let stripped = strip_ansi(&raw);
            let (redacted, changed) = redact_text(&stripped);
            (redacted, "Hosted terminal session", changed)
        }
        CaptureFlush::Digest(digest) => (redact_text(&digest).0, "Hosted terminal digest", true),
    };
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
        title: Some(title.to_owned()),
        text: Some(text),
        payload: None,
        metadata: None,
        privacy_level: None,
        redacted: already_redacted,
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
                            if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
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
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                // CRLF pair — an ordinary line ending (many programs emit
                // this), not an in-place redraw. Drop the CR; the LF is
                // handled normally on the next iteration.
                i += 1;
            }
            b'\r' => {
                // A BARE CR (no following LF) means "return to column 0" — a
                // real terminal then overwrites the line in place (spinners,
                // progress/status lines all redraw this way). Naively
                // dropping it instead glued every redraw frame together into
                // unreadable, word-glued text (2026-07-04: "the text is not
                // formatted" — reproduced verbatim on the Home live-session
                // preview). Truncate back to the start of the current line so
                // only the FINAL redraw survives, matching what's on screen.
                i += 1;
                match out.rfind('\n') {
                    Some(last_newline) => out.truncate(last_newline + 1),
                    None => out.clear(),
                }
            }
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
    use super::{
        strip_ansi, take_capture_for_flush, DigestBuffer, TermShared, CAPTURE_FLUSH_THRESHOLD,
        REDACT_CARRY, SCROLLBACK_MAX,
    };
    use grafiki_core::redact_text;

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
    fn bare_cr_redraw_keeps_only_the_final_frame() {
        // A spinner/status line redrawing itself in place via bare CR (no
        // following LF) — e.g. Claude Code's own status footer. The OLD
        // behavior dropped the CR and glued every frame together into
        // unreadable text (2026-07-04 bug, reproduced verbatim on Grafiki's
        // own Home live-session preview: "Fable5withusage|credits...").
        let raw = b"spinner-frame-1\rspinner-frame-2\rFinal status line\n";
        let out = strip_ansi(raw);
        assert_eq!(out, "Final status line\n");
    }

    #[test]
    fn bare_cr_redraw_only_erases_the_current_line() {
        // A completed line stays intact; only the line CURRENTLY being
        // redrawn (after the last real newline) gets truncated.
        let raw = b"line one\nworking...\rdone\n";
        let out = strip_ansi(raw);
        assert_eq!(out, "line one\ndone\n");
    }

    #[test]
    fn scrollback_caps_and_trims_to_newline_boundary() {
        let mut shared = TermShared {
            scrollback: Vec::new(),
            channel: None,
            capture: Vec::new(),
            digest: DigestBuffer::default(),
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

    #[test]
    fn digest_retains_counts_but_no_output_sample() {
        let mut digest = DigestBuffer::default();
        digest.push(b"API_TOKEN=super-secret-value\n");
        for index in 0..2_000 {
            digest.push(format!("ordinary output line {index}\n").as_bytes());
        }
        let report = digest.take().expect("digest");
        assert!(report.contains("No terminal output sample"));
        assert!(!report.contains("super-secret-value"));
        assert_eq!(digest.observed_bytes, 0);
        assert_eq!(digest.observed_lines, 0);
    }

    #[test]
    fn digest_never_reassembles_or_persists_split_secrets() {
        let mut digest = DigestBuffer::default();
        digest.push(b"API_TOKEN=super-");
        digest.push(b"secret-value\nnext line\n");
        let report = digest.take().expect("digest");
        assert!(!report.contains("super-"));
        assert!(!report.contains("secret-value"));
        assert!(!report.contains("API_TOKEN"));
        assert!(report.contains("2 lines"));
    }

    #[test]
    fn full_capture_redacts_a_secret_split_across_the_flush_boundary() {
        // Read 1: harmless filler followed immediately by the START of a
        // secret assignment ("API_TOKEN=sk-") — sized so the READ ALONE
        // crosses the flush threshold, with the key landing in the trailing
        // bytes. Without a carry, a naive flush takes the WHOLE accumulator
        // here (key, no value yet — nothing leaks on this call) and resets
        // to empty; the value then arrives on its own later with no key on
        // its line, and leaks unredacted. `take_capture_for_flush` instead
        // holds back the last `REDACT_CARRY` bytes — which the key sits
        // inside of — so it survives into the next accumulation instead.
        let secret_key = b"API_TOKEN=sk-";
        let filler_len = CAPTURE_FLUSH_THRESHOLD + 64 - secret_key.len() - 1;
        let mut capture: Vec<u8> = vec![b'x'; filler_len];
        capture.push(b'\n');
        capture.extend_from_slice(secret_key);
        assert!(capture.len() > CAPTURE_FLUSH_THRESHOLD);

        let flush1 = take_capture_for_flush(&mut capture).expect("threshold crossed on read 1");
        // The key survived into the carry rather than being flushed alone.
        assert_eq!(capture.len(), REDACT_CARRY);
        let raw_flush1 = String::from_utf8_lossy(&flush1);
        assert!(!raw_flush1.contains("API_TOKEN"));

        // Read 2: the PTY delivers the rest of the secret. The session then
        // ends, so the whole remaining (carried) buffer is flushed as one.
        capture.extend_from_slice(b"SECRETVALUE\n");
        let flush2 = std::mem::take(&mut capture);

        let (redacted1, _) = redact_text(&strip_ansi(&flush1));
        let (redacted2, _) = redact_text(&strip_ansi(&flush2));

        assert!(!redacted1.contains("SECRETVALUE"));
        assert!(!redacted2.contains("SECRETVALUE"));
        assert!(!redacted2.contains("sk-SECRETVALUE"));
        assert!(redacted2.contains("REDACTED"));
    }
}
