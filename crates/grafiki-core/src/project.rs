use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::db::{open_project_database, schema::initialize_schema};
use crate::memory::{propose_candidate, EvidenceInput, ProposeCandidateOptions};
use crate::{GrafikiError, Result};

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub project_name: Option<String>,
    pub project_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ProjectResolveOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CaptureConfigOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct CaptureSourceUpdates {
    pub git: Option<bool>,
    pub transcripts: Option<bool>,
    pub terminal: Option<bool>,
    pub files: Option<bool>,
    pub ide: Option<bool>,
    pub screen: Option<bool>,
    pub browser: Option<bool>,
    pub audio: Option<bool>,
    pub system: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateCaptureConfigOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub sources: CaptureSourceUpdates,
    pub add_blocked_paths: Vec<String>,
    pub remove_blocked_paths: Vec<String>,
    pub add_blocked_apps: Vec<String>,
    pub remove_blocked_apps: Vec<String>,
    pub redaction_profile: Option<String>,
    pub terminal_output: Option<String>,
    pub screen_policy: Option<String>,
    pub browser_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitReport {
    pub project: String,
    pub project_dir: PathBuf,
    pub marker_path: PathBuf,
    pub capture_config_path: PathBuf,
    pub db_path: PathBuf,
    pub created_marker: bool,
    pub created_capture_config: bool,
    pub created_database: bool,
    pub imported_files: Vec<InitImportedFile>,
    pub proposed_candidates: usize,
    pub trusted_records: usize,
    pub skipped_sources: Vec<String>,
    pub decisions_found: usize,
    pub rules_found: usize,
    pub next_agent_setup: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitImportedFile {
    pub path: PathBuf,
    pub source_type: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project: String,
    pub project_dir: PathBuf,
    pub marker_path: Option<PathBuf>,
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureConfigReport {
    pub project: String,
    pub project_dir: PathBuf,
    pub config_path: PathBuf,
    pub created: bool,
    pub config: CaptureConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub version: u32,
    pub sources: CaptureSourceConfig,
    pub blocked_paths: Vec<String>,
    pub blocked_apps: Vec<String>,
    pub redaction_profile: String,
    pub terminal_output: String,
    pub screen_policy: String,
    pub browser_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSourceConfig {
    pub git: bool,
    pub transcripts: bool,
    pub terminal: bool,
    pub files: bool,
    pub ide: bool,
    pub screen: bool,
    pub browser: bool,
    pub audio: bool,
    pub system: bool,
}

impl Default for CaptureSourceConfig {
    fn default() -> Self {
        Self {
            git: true,
            transcripts: true,
            terminal: true,
            files: true,
            ide: true,
            screen: false,
            browser: false,
            audio: false,
            system: true,
        }
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            version: 1,
            sources: CaptureSourceConfig::default(),
            blocked_paths: vec![
                ".git".to_owned(),
                ".grafiki".to_owned(),
                ".grafiki.capture.json".to_owned(),
                ".env".to_owned(),
                "node_modules".to_owned(),
                "target".to_owned(),
                "dist".to_owned(),
                "build".to_owned(),
            ],
            blocked_apps: Vec::new(),
            redaction_profile: "default".to_owned(),
            terminal_output: "off".to_owned(),
            screen_policy: "manual".to_owned(),
            browser_policy: "off".to_owned(),
        }
    }
}

pub fn init_project(options: InitOptions) -> Result<InitReport> {
    let project_dir = normalize_project_dir(&options.project_dir)?;
    let project = match options.project_name {
        Some(name) => validate_project_name(name.trim())?,
        None => infer_project_name(&project_dir)?,
    };

    let grafiki_home = match options.grafiki_home {
        Some(path) => path,
        None => grafiki_home()?,
    };
    fs::create_dir_all(&grafiki_home)?;

    let marker_path = project_dir.join(".grafiki");
    let created_marker = write_marker_if_needed(&marker_path, &project)?;
    let capture_config_path = capture_config_path(&project_dir);
    let created_capture_config = write_default_capture_config_if_needed(&capture_config_path)?;

    let db_path = grafiki_home.join(format!("{project}.db"));
    let created_database = !db_path.exists();
    let mut connection = open_project_database(&db_path)?;
    initialize_schema(&mut connection)?;
    drop(connection);

    let import_report = import_initial_memory(&project, &project_dir, Some(grafiki_home.clone()))?;

    Ok(InitReport {
        next_agent_setup: format!(
            "grafiki mcp --project {project} --path {}",
            project_dir.display()
        ),
        project,
        project_dir,
        marker_path,
        capture_config_path,
        db_path,
        created_marker,
        created_capture_config,
        created_database,
        imported_files: import_report.imported_files,
        proposed_candidates: import_report.proposed_candidates,
        trusted_records: 0,
        skipped_sources: import_report.skipped_sources,
        decisions_found: import_report.decisions_found,
        rules_found: import_report.rules_found,
    })
}

pub fn load_capture_config(options: CaptureConfigOptions) -> Result<CaptureConfigReport> {
    let project = resolve_project(ProjectResolveOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
    })?;
    let config_path = capture_config_path(&project.project_dir);
    let created = write_default_capture_config_if_needed(&config_path)?;
    let config = read_capture_config(&config_path)?;

    Ok(CaptureConfigReport {
        project: project.project,
        project_dir: project.project_dir,
        config_path,
        created,
        config,
    })
}

pub fn update_capture_config(options: UpdateCaptureConfigOptions) -> Result<CaptureConfigReport> {
    let project = resolve_project(ProjectResolveOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
    })?;
    let config_path = capture_config_path(&project.project_dir);
    write_default_capture_config_if_needed(&config_path)?;
    let mut config = read_capture_config(&config_path)?;

    apply_source_update(&mut config.sources.git, options.sources.git);
    apply_source_update(&mut config.sources.transcripts, options.sources.transcripts);
    apply_source_update(&mut config.sources.terminal, options.sources.terminal);
    apply_source_update(&mut config.sources.files, options.sources.files);
    apply_source_update(&mut config.sources.ide, options.sources.ide);
    apply_source_update(&mut config.sources.screen, options.sources.screen);
    apply_source_update(&mut config.sources.browser, options.sources.browser);
    apply_source_update(&mut config.sources.audio, options.sources.audio);
    apply_source_update(&mut config.sources.system, options.sources.system);

    merge_items(&mut config.blocked_paths, options.add_blocked_paths);
    remove_items(&mut config.blocked_paths, options.remove_blocked_paths);
    merge_items(&mut config.blocked_apps, options.add_blocked_apps);
    remove_items(&mut config.blocked_apps, options.remove_blocked_apps);

    if let Some(redaction_profile) = options.redaction_profile {
        config.redaction_profile = non_empty_config_value("redaction_profile", redaction_profile)?;
    }
    if let Some(terminal_output) = options.terminal_output {
        config.terminal_output = validate_enum_config_value(
            "terminal_output",
            terminal_output,
            &["off", "digest", "full"],
        )?;
    }
    if let Some(screen_policy) = options.screen_policy {
        config.screen_policy = validate_enum_config_value(
            "screen_policy",
            screen_policy,
            &["off", "manual", "allowlist"],
        )?;
    }
    if let Some(browser_policy) = options.browser_policy {
        config.browser_policy =
            validate_enum_config_value("browser_policy", browser_policy, &["off", "allowlist"])?;
    }

    write_capture_config(&config_path, &config)?;

    Ok(CaptureConfigReport {
        project: project.project,
        project_dir: project.project_dir,
        config_path,
        created: false,
        config,
    })
}

#[derive(Debug, Clone, Default)]
struct InitImportReport {
    imported_files: Vec<InitImportedFile>,
    proposed_candidates: usize,
    skipped_sources: Vec<String>,
    decisions_found: usize,
    rules_found: usize,
}

fn import_initial_memory(
    project: &str,
    project_dir: &Path,
    grafiki_home: Option<PathBuf>,
) -> Result<InitImportReport> {
    let mut report = InitImportReport::default();
    let mut sources = Vec::new();
    push_file_if_exists(
        &mut sources,
        project_dir.join("CLAUDE.md"),
        "init:claude-md",
    );
    collect_files_in_dir(
        &mut sources,
        &project_dir.join(".cursor").join("rules"),
        "init:cursor-rule",
    )?;
    collect_files_in_dir(
        &mut sources,
        &project_dir.join("memory-bank"),
        "init:cline-memory-bank",
    )?;
    collect_files_in_dir(
        &mut sources,
        &project_dir.join(".cline").join("memory-bank"),
        "init:cline-memory-bank",
    )?;
    push_file_if_exists(
        &mut sources,
        project_dir.join(".clinerules"),
        "init:cline-rule",
    );

    for (path, source_type) in sources {
        let content = match fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => content,
            Ok(_) => {
                report
                    .skipped_sources
                    .push(format!("{} was empty", path.display()));
                continue;
            }
            Err(error) => {
                report
                    .skipped_sources
                    .push(format!("{} could not be read: {error}", path.display()));
                continue;
            }
        };
        let source = path
            .strip_prefix(project_dir)
            .ok()
            .map(|path| path.display().to_string());
        if candidate_source_exists(
            project,
            grafiki_home.as_deref(),
            source_type,
            source.as_deref(),
        )? {
            report
                .skipped_sources
                .push(format!("{} was already imported", path.display()));
            continue;
        }
        let title = init_source_title(&path, source_type);
        let key = format!("init-{}", stable_key(&path));
        let candidate = propose_candidate(ProposeCandidateOptions {
            project_name: Some(project.to_owned()),
            start_dir: project_dir.to_path_buf(),
            grafiki_home: grafiki_home.clone(),
            source_type: source_type.to_owned(),
            source: source.clone(),
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": key,
                "title": title,
                "category": "onboarding",
                "content": content,
            }),
            scope: project.to_owned(),
            confidence: 0.82,
            rationale: Some(
                "Imported during grafiki init from an existing user-authored agent memory file."
                    .to_owned(),
            ),
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: source_type.to_owned(),
                source,
                title: Some(title),
                excerpt: content,
                uri: Some(format!("file://{}", path.display())),
                byte_start: None,
                byte_end: None,
                line_start: Some(1),
                line_end: None,
                captured_at: None,
            }],
        })?;
        if source_type == "init:cursor-rule" || source_type == "init:cline-rule" {
            report.rules_found += 1;
        }
        report.imported_files.push(InitImportedFile {
            path,
            source_type: source_type.to_owned(),
            candidate_id: candidate.candidate.id,
        });
        report.proposed_candidates += 1;
    }

    if let Some(git_summary) = git_history_summary(project_dir) {
        if candidate_source_exists(
            project,
            grafiki_home.as_deref(),
            "init:git-history",
            Some("git log"),
        )? {
            report
                .skipped_sources
                .push("recent git history was already imported".to_owned());
            return Ok(report);
        }
        let candidate = propose_candidate(ProposeCandidateOptions {
            project_name: Some(project.to_owned()),
            start_dir: project_dir.to_path_buf(),
            grafiki_home,
            source_type: "init:git-history".to_owned(),
            source: Some("git log".to_owned()),
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": "init-recent-git-history",
                "title": "Recent git history imported at init",
                "category": "audit",
                "content": git_summary,
            }),
            scope: project.to_owned(),
            confidence: 0.7,
            rationale: Some(
                "Imported during grafiki init from recent git history; review before trusting."
                    .to_owned(),
            ),
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: "git".to_owned(),
                source: Some("git log".to_owned()),
                title: Some("Recent git history".to_owned()),
                excerpt: git_summary,
                uri: None,
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        })?;
        report.imported_files.push(InitImportedFile {
            path: project_dir.join(".git"),
            source_type: "init:git-history".to_owned(),
            candidate_id: candidate.candidate.id,
        });
        report.proposed_candidates += 1;
    } else {
        report
            .skipped_sources
            .push("recent git history was unavailable".to_owned());
    }

    Ok(report)
}

pub fn resolve_project(options: ProjectResolveOptions) -> Result<ProjectContext> {
    let start_dir = options.start_dir.canonicalize()?;
    let grafiki_home = match options.grafiki_home {
        Some(path) => path,
        None => grafiki_home()?,
    };

    let marker = find_marker(&start_dir)?;
    let (project, project_dir, marker_path) = match options.project_name {
        Some(name) => {
            let project = validate_project_name(name.trim())?;
            let project_dir = marker
                .as_ref()
                .map(|marker| marker.project_dir.clone())
                .unwrap_or(start_dir);
            let marker_path = marker.map(|marker| marker.marker_path);
            (project, project_dir, marker_path)
        }
        None => match marker {
            Some(marker) => (marker.project, marker.project_dir, Some(marker.marker_path)),
            None => {
                let project = infer_project_name(&start_dir)?;
                (project, start_dir, None)
            }
        },
    };

    let db_path = grafiki_home.join(format!("{project}.db"));
    if !db_path.exists() {
        return Err(GrafikiError::ProjectNotInitialized(project));
    }

    Ok(ProjectContext {
        project,
        project_dir,
        marker_path,
        db_path,
    })
}

#[derive(Debug, Clone)]
struct Marker {
    project: String,
    project_dir: PathBuf,
    marker_path: PathBuf,
}

fn find_marker(start_dir: &Path) -> Result<Option<Marker>> {
    for dir in start_dir.ancestors() {
        let marker_path = dir.join(".grafiki");
        if marker_path.exists() {
            let project = validate_project_name(fs::read_to_string(&marker_path)?.trim())?;
            return Ok(Some(Marker {
                project,
                project_dir: dir.to_path_buf(),
                marker_path,
            }));
        }
    }

    Ok(None)
}

fn normalize_project_dir(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }

    fs::create_dir_all(path)?;
    Ok(path.canonicalize()?)
}

pub(crate) fn infer_project_name(project_dir: &Path) -> Result<String> {
    let name = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| GrafikiError::InvalidProjectName(project_dir.display().to_string()))?;

    validate_project_name(name)
}

pub(crate) fn validate_project_name(raw: &str) -> Result<String> {
    if raw.is_empty() || !raw.chars().all(is_valid_project_char) {
        return Err(GrafikiError::InvalidProjectName(raw.to_owned()));
    }

    Ok(raw.to_owned())
}

fn is_valid_project_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

pub(crate) fn grafiki_home() -> Result<PathBuf> {
    if let Some(home) = env::var_os("GRAFIKI_HOME") {
        return Ok(PathBuf::from(home));
    }

    let home = env::var_os("HOME").ok_or(GrafikiError::MissingHomeDir)?;
    Ok(PathBuf::from(home).join(".grafiki"))
}

fn write_marker_if_needed(marker_path: &Path, project: &str) -> Result<bool> {
    let desired = format!("{project}\n");

    if marker_path.exists() {
        let existing = fs::read_to_string(marker_path)?;
        if existing == desired {
            return Ok(false);
        }
    }

    fs::write(marker_path, desired)?;
    Ok(true)
}

fn capture_config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".grafiki.capture.json")
}

fn write_default_capture_config_if_needed(config_path: &Path) -> Result<bool> {
    if config_path.exists() {
        return Ok(false);
    }
    write_capture_config(config_path, &CaptureConfig::default())?;
    Ok(true)
}

fn read_capture_config(config_path: &Path) -> Result<CaptureConfig> {
    let content = fs::read_to_string(config_path)?;
    if content.trim().is_empty() {
        return Ok(CaptureConfig::default());
    }
    let mut config: CaptureConfig = serde_json::from_str(&content)?;
    normalize_capture_config(&mut config)?;
    Ok(config)
}

fn write_capture_config(config_path: &Path, config: &CaptureConfig) -> Result<()> {
    fs::write(
        config_path,
        format!("{}\n", serde_json::to_string_pretty(config)?),
    )?;
    Ok(())
}

fn normalize_capture_config(config: &mut CaptureConfig) -> Result<()> {
    config.version = 1;
    config.redaction_profile =
        non_empty_config_value("redaction_profile", config.redaction_profile.clone())?;
    config.terminal_output = validate_enum_config_value(
        "terminal_output",
        config.terminal_output.clone(),
        &["off", "digest", "full"],
    )?;
    config.screen_policy = validate_enum_config_value(
        "screen_policy",
        config.screen_policy.clone(),
        &["off", "manual", "allowlist"],
    )?;
    config.browser_policy = validate_enum_config_value(
        "browser_policy",
        config.browser_policy.clone(),
        &["off", "allowlist"],
    )?;
    dedupe_items(&mut config.blocked_paths);
    dedupe_items(&mut config.blocked_apps);
    Ok(())
}

fn apply_source_update(target: &mut bool, value: Option<bool>) {
    if let Some(value) = value {
        *target = value;
    }
}

fn merge_items(target: &mut Vec<String>, values: Vec<String>) {
    target.extend(values.into_iter().filter_map(normalize_list_item));
    dedupe_items(target);
}

fn remove_items(target: &mut Vec<String>, values: Vec<String>) {
    let removals = values
        .into_iter()
        .filter_map(normalize_list_item)
        .collect::<Vec<_>>();
    if removals.is_empty() {
        return;
    }
    target.retain(|item| !removals.iter().any(|remove| remove == item));
}

fn dedupe_items(items: &mut Vec<String>) {
    let mut normalized = Vec::new();
    for item in std::mem::take(items) {
        if let Some(item) = normalize_list_item(item) {
            if !normalized.iter().any(|existing| existing == &item) {
                normalized.push(item);
            }
        }
    }
    *items = normalized;
}

fn normalize_list_item(value: String) -> Option<String> {
    let value = value.trim().trim_end_matches('/').to_owned();
    (!value.is_empty()).then_some(value)
}

fn non_empty_config_value(key: &str, value: String) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(GrafikiError::InvalidCaptureConfig(format!(
            "{key} cannot be empty"
        )));
    }
    Ok(value)
}

fn validate_enum_config_value(key: &str, value: String, allowed: &[&str]) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if allowed.iter().any(|allowed| *allowed == value) {
        return Ok(value);
    }
    Err(GrafikiError::InvalidCaptureConfig(format!(
        "{key} must be one of: {}",
        allowed.join(", ")
    )))
}

fn push_file_if_exists(
    sources: &mut Vec<(PathBuf, &'static str)>,
    path: PathBuf,
    source_type: &'static str,
) {
    if path.is_file() {
        sources.push((path, source_type));
    }
}

fn collect_files_in_dir(
    sources: &mut Vec<(PathBuf, &'static str)>,
    dir: &Path,
    source_type: &'static str,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_in_dir(sources, &path, source_type)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| matches!(extension, "md" | "mdc" | "txt"))
            .unwrap_or(false)
        {
            sources.push((path, source_type));
        }
    }
    Ok(())
}

fn init_source_title(path: &Path, source_type: &str) -> String {
    match source_type {
        "init:claude-md" => "Imported CLAUDE.md memory".to_owned(),
        "init:cursor-rule" => format!(
            "Imported Cursor rule: {}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("rule")
        ),
        "init:cline-memory-bank" => format!(
            "Imported Cline memory bank: {}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("memory")
        ),
        "init:cline-rule" => "Imported Cline rule memory".to_owned(),
        _ => format!(
            "Imported memory: {}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("memory")
        ),
    }
}

fn stable_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .flat_map(|part| part.chars())
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn git_history_summary(project_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["log", "-n", "8", "--pretty=format:%h %cs %s"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let summary = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!summary.is_empty()).then_some(summary)
}

fn candidate_source_exists(
    project: &str,
    grafiki_home_path: Option<&Path>,
    source_type: &str,
    source: Option<&str>,
) -> Result<bool> {
    let db_path = match grafiki_home_path {
        Some(home) => home.join(format!("{project}.db")),
        None => grafiki_home()?.join(format!("{project}.db")),
    };
    let connection = open_project_database(&db_path)?;
    let count: i64 = match source {
        Some(source) => connection.query_row(
            "
            SELECT COUNT(*)
            FROM extraction_candidates
            WHERE source_type = ?1 AND source = ?2
            ",
            (source_type, source),
            |row| row.get(0),
        )?,
        None => connection.query_row(
            "
            SELECT COUNT(*)
            FROM extraction_candidates
            WHERE source_type = ?1 AND source IS NULL
            ",
            [source_type],
            |row| row.get(0),
        )?,
    };
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        init_project, load_capture_config, resolve_project, update_capture_config,
        CaptureConfigOptions, CaptureSourceUpdates, InitOptions, ProjectResolveOptions,
        UpdateCaptureConfigOptions,
    };

    #[test]
    fn init_creates_marker_and_database() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");

        let report = init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();

        assert_eq!(report.project, "example-project");
        assert!(report.created_marker);
        assert!(report.created_database);
        assert_eq!(
            fs::read_to_string(project_dir.join(".grafiki")).unwrap(),
            "example-project\n"
        );
        assert!(project_dir.join(".grafiki.capture.json").exists());
        assert!(report.created_capture_config);
        assert!(report
            .capture_config_path
            .ends_with(".grafiki.capture.json"));
        assert!(home.join("example-project.db").exists());
    }

    #[test]
    fn init_is_idempotent_for_existing_project() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");

        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        let report = init_project(InitOptions {
            project_name: None,
            project_dir,
            grafiki_home: Some(home),
        })
        .unwrap();

        assert!(!report.created_marker);
        assert!(!report.created_capture_config);
        assert!(!report.created_database);
    }

    #[test]
    fn capture_config_round_trip_updates_workspace_consent() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");

        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();

        let report = update_capture_config(UpdateCaptureConfigOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            sources: CaptureSourceUpdates {
                terminal: Some(false),
                screen: Some(true),
                ..CaptureSourceUpdates::default()
            },
            add_blocked_paths: vec!["secrets".to_owned(), ".env.local".to_owned()],
            remove_blocked_paths: vec!["target".to_owned()],
            terminal_output: Some("digest".to_owned()),
            screen_policy: Some("allowlist".to_owned()),
            ..UpdateCaptureConfigOptions::default()
        })
        .unwrap();

        assert!(!report.config.sources.terminal);
        assert!(report.config.sources.screen);
        assert_eq!(report.config.terminal_output, "digest");
        assert_eq!(report.config.screen_policy, "allowlist");
        assert!(report.config.blocked_paths.contains(&"secrets".to_owned()));
        assert!(!report.config.blocked_paths.contains(&"target".to_owned()));

        let loaded = load_capture_config(CaptureConfigOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
        })
        .unwrap();
        assert_eq!(loaded.config, report.config);
    }

    #[test]
    fn resolve_project_reads_marker_from_parent_directory() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");
        let child_dir = project_dir.join("backend").join("api");

        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        fs::create_dir_all(&child_dir).unwrap();

        let context = resolve_project(ProjectResolveOptions {
            project_name: None,
            start_dir: child_dir,
            grafiki_home: Some(home),
        })
        .unwrap();

        assert_eq!(context.project, "example-project");
        assert_eq!(context.project_dir, project_dir.canonicalize().unwrap());
        assert_eq!(
            context.marker_path,
            Some(project_dir.join(".grafiki").canonicalize().unwrap())
        );
    }
}
