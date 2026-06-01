#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

mod scheduler;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Suite {
    id: String,
    name: String,
    system: String,
    tag: String,       // "E2E" | "API" | "Unit"
    command: String,   // "playwright" | "vitest"
    args: Vec<String>,
    cwd: String,       // path relative to project root where command runs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    id: String,
    name: String,
    path: String,
    suites: Vec<Suite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestRun {
    id: String,
    project_id: String,
    project_name: String,
    suite_id: String,
    suite_name: String,
    suite_tag: String,
    status: String,
    duration_ms: i64,
    pass_count: i64,
    fail_count: i64,
    started_at: String,
    coverage_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvent {
    suite_id: String,
    line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    suite_id: String,
    status: String,
    duration: Option<u64>,
    exit_code: Option<i32>,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub(crate) struct AppState {
    pub(crate) db: Mutex<Connection>,
    running:       Mutex<HashMap<String, oneshot::Sender<()>>>,
    run_meta:      Mutex<HashMap<String, RunMeta>>,
}

#[derive(Clone)]
struct RunMeta {
    run_id:       String,
    project_id:   String,
    project_name: String,
    suite_name:   String,
    suite_tag:    String,
    started_at:   String,
    pass_count:   i64,
    fail_count:   i64,
    pid:          Option<u32>,
    output_lines: Vec<String>,
    project_path: String,
    suite_cwd:    String,
}

// ── Database setup ────────────────────────────────────────────────────────────

fn db_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("failed to resolve app data dir")
        .join("testrunner.db")
}

fn init_db(conn: &Connection) {
    conn.execute_batch("
        PRAGMA journal_mode=WAL;

        CREATE TABLE IF NOT EXISTS projects (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            path       TEXT NOT NULL,
            suites     TEXT NOT NULL,  -- JSON
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS test_runs (
            id           TEXT PRIMARY KEY,
            project_id   TEXT NOT NULL,
            project_name TEXT NOT NULL,
            suite_id     TEXT NOT NULL,
            suite_name   TEXT NOT NULL,
            suite_tag    TEXT NOT NULL,
            status       TEXT NOT NULL,
            duration_ms  INTEGER NOT NULL DEFAULT 0,
            pass_count   INTEGER NOT NULL DEFAULT 0,
            fail_count   INTEGER NOT NULL DEFAULT 0,
            started_at   TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS schedules (
            id           TEXT PRIMARY KEY,
            project_id   TEXT NOT NULL,
            suite_id     TEXT NOT NULL,
            label        TEXT NOT NULL,
            scheduled_at TEXT NOT NULL,
            recurrence   TEXT NOT NULL DEFAULT 'once',
            enabled      INTEGER NOT NULL DEFAULT 1,
            last_run_at  TEXT,
            created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
    ").expect("failed to initialize database");

    // Migration: add output column (ignored if already exists)
    let _ = conn.execute("ALTER TABLE test_runs ADD COLUMN output TEXT", []);
    let _ = conn.execute("ALTER TABLE test_runs ADD COLUMN coverage_pct REAL", []);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for ch in chars.by_ref() { if ch.is_ascii_alphabetic() { break; } }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO-8601 approximation via SQLite datetime
    format!("datetime({}, 'unixepoch')", now)
}

fn read_coverage(project_path: &str, suite_cwd: &str) -> Option<f64> {
    let base = if suite_cwd.is_empty() {
        PathBuf::from(project_path)
    } else {
        PathBuf::from(project_path).join(suite_cwd)
    };
    let path = base.join("coverage").join("coverage-summary.json");
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("total")?.get("lines")?.get("pct")?.as_f64()
}

// ── Project commands ──────────────────────────────────────────────────────────

#[tauri::command]
fn get_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let db = state.db.lock().unwrap();
    let mut stmt = db.prepare("SELECT id, name, path, suites FROM projects ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let projects = stmt.query_map([], |row| {
        let suites_json: String = row.get(3)?;
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, suites_json))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .map(|(id, name, path, suites_json)| {
        let suites: Vec<Suite> = serde_json::from_str(&suites_json).unwrap_or_default();
        Project { id, name, path, suites }
    })
    .collect();

    Ok(projects)
}

#[tauri::command]
fn save_project(state: State<'_, AppState>, project: Project) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let suites_json = serde_json::to_string(&project.suites).map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO projects (id, name, path, suites) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, path=excluded.path, suites=excluded.suites",
        params![project.id, project.name, project.path, suites_json],
    ).map_err(|e| e.to_string())?;

    // Remove any OTHER entries with the same path (prevents accumulation of duplicates)
    db.execute(
        "DELETE FROM projects WHERE path = ?1 AND id != ?2",
        params![project.path, project.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, project_id: String) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.execute("DELETE FROM projects WHERE id = ?1", params![project_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Test run history commands ─────────────────────────────────────────────────

#[tauri::command]
fn get_runs(state: State<'_, AppState>, project_id: Option<String>) -> Result<Vec<TestRun>, String> {
    let db = state.db.lock().unwrap();

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<TestRun> {
        Ok(TestRun {
            id:           row.get(0)?,
            project_id:   row.get(1)?,
            project_name: row.get(2)?,
            suite_id:     row.get(3)?,
            suite_name:   row.get(4)?,
            suite_tag:    row.get(5)?,
            status:       row.get(6)?,
            duration_ms:  row.get(7)?,
            pass_count:   row.get(8)?,
            fail_count:   row.get(9)?,
            started_at:   row.get(10)?,
            coverage_pct: row.get(11)?,
        })
    };

    let runs: Vec<TestRun> = if let Some(pid) = project_id {
        let mut stmt = db.prepare(
            "SELECT id,project_id,project_name,suite_id,suite_name,suite_tag,status,\
             duration_ms,pass_count,fail_count,started_at,coverage_pct \
             FROM test_runs WHERE project_id=?1 ORDER BY started_at DESC LIMIT 500"
        ).map_err(|e| e.to_string())?;
        let rows: Vec<TestRun> = stmt.query_map(params![pid], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        rows
    } else {
        let mut stmt = db.prepare(
            "SELECT id,project_id,project_name,suite_id,suite_name,suite_tag,status,\
             duration_ms,pass_count,fail_count,started_at,coverage_pct \
             FROM test_runs ORDER BY started_at DESC LIMIT 500"
        ).map_err(|e| e.to_string())?;
        let rows: Vec<TestRun> = stmt.query_map([], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    Ok(runs)
}

// ── Run output command ────────────────────────────────────────────────────────

#[tauri::command]
fn get_run_output(state: State<'_, AppState>, run_id: String) -> Result<Vec<String>, String> {
    let db = state.db.lock().unwrap();
    let output_json: Option<String> = db.query_row(
        "SELECT output FROM test_runs WHERE id = ?1",
        params![run_id],
        |row| row.get(0),
    ).ok().flatten();

    match output_json {
        Some(json) if !json.is_empty() => {
            serde_json::from_str::<Vec<String>>(&json).map_err(|e| e.to_string())
        }
        _ => Ok(vec![]),
    }
}

// ── Folder picker ─────────────────────────────────────────────────────────────

#[tauri::command]
async fn pick_folder() -> Option<String> {
    tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Selecionar Pasta do Projeto")
            .pick_folder()
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .ok()
    .flatten()
}

// ── File save dialog ──────────────────────────────────────────────────────────

#[tauri::command]
async fn save_pdf(filename: String, data: Vec<u8>) -> Result<String, String> {
    let path = tokio::task::spawn_blocking(move || {
        let ext = filename.rsplit('.').next().unwrap_or("pdf");
        let (title, filter_name) = match ext {
            "xlsx" => ("Salvar Relatório Excel", "Excel"),
            _      => ("Salvar Relatório PDF",   "PDF"),
        };
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(&filename)
            .add_filter(filter_name, &[ext])
            .save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    match path {
        Some(p) => {
            std::fs::write(&p, &data).map_err(|e| e.to_string())?;
            Ok(p.to_string_lossy().to_string())
        }
        None => Err("cancelado".to_string()),
    }
}

// ── Suite scanning ────────────────────────────────────────────────────────────

/// Directories to never recurse into
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", ".git", ".next",
    "build", "coverage", ".cache", "out", ".turbo", ".svelte-kit",
];

fn scan_dir(root: &PathBuf, dir: &PathBuf, depth: usize, suites: &mut Vec<Suite>) {
    if depth > 4 { return; }

    // Skip known non-source directories
    if depth > 0 {
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        if SKIP_DIRS.contains(&name.as_ref()) { return; }
    }

    // Relative path from root → used as `cwd` in Suite (empty string = root)
    let rel = dir.strip_prefix(root)
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");

    let folder = dir.file_name()
        .unwrap_or(root.file_name().unwrap_or_default())
        .to_string_lossy()
        .to_string();

    // ── Playwright ────────────────────────────────────────────────────────
    let has_pw = ["playwright.config.ts", "playwright.config.js", "playwright.config.mjs"]
        .iter()
        .any(|f| dir.join(f).exists());

    if has_pw {
        let modules_dir = dir.join("modules");
        if modules_dir.exists() {
            // Multi-module layout: modules/<name>/frontend  and/or  modules/<name>/backend
            if let Ok(entries) = std::fs::read_dir(&modules_dir) {
                let mut mods: Vec<_> = entries.flatten()
                    .filter(|e| e.path().is_dir())
                    .collect();
                mods.sort_by_key(|e| e.file_name());

                for entry in mods {
                    let mp  = entry.path();
                    let mn  = entry.file_name().to_string_lossy().to_string();
                    if mn == "shared" || mn.starts_with('.') { continue; }

                    let id_prefix = if rel.is_empty() { mn.clone() } else { format!("{}/{}", rel, mn) };

                    if mp.join("frontend").exists() {
                        suites.push(Suite {
                            id:      format!("{}-frontend", id_prefix),
                            name:    "Frontend".into(),
                            system:  mn.clone(),
                            tag:     "E2E".into(),
                            command: "playwright".into(),
                            args:    vec!["test".into(), format!("modules/{}/frontend", mn), "--project=chromium".into(), "--reporter=list".into()],
                            cwd:     rel.clone(),
                        });
                    }
                    if mp.join("backend").exists() {
                        suites.push(Suite {
                            id:      format!("{}-backend", id_prefix),
                            name:    "Backend API".into(),
                            system:  mn.clone(),
                            tag:     "API".into(),
                            command: "playwright".into(),
                            args:    vec!["test".into(), format!("modules/{}/backend", mn), "--project=api".into(), "--reporter=list".into()],
                            cwd:     rel.clone(),
                        });
                    }
                }
            }
        } else {
            // Flat Playwright project
            let id = if rel.is_empty() { "playwright-e2e".into() }
                     else { format!("{}-e2e", rel.replace('/', "-")) };
            suites.push(Suite {
                id,
                name:    "E2E Tests".into(),
                system:  folder.clone(),
                tag:     "E2E".into(),
                command: "playwright".into(),
                args:    vec!["test".into(), "--reporter=list".into()],
                cwd:     rel.clone(),
            });
        }
    }

    // ── Vitest ────────────────────────────────────────────────────────────
    let has_vitest = ["vitest.config.ts", "vitest.config.js", "vitest.config.mjs", "vitest.config.cjs"]
        .iter()
        .any(|f| dir.join(f).exists());

    if has_vitest {
        let id = if rel.is_empty() { "vitest-unit".into() }
                 else { format!("{}-unit", rel.replace('/', "-")) };

        // Check if any vitest config has coverage configured
        let coverage_enabled = ["vitest.config.ts", "vitest.config.js", "vitest.config.mjs", "vitest.config.cjs"]
            .iter()
            .find(|f| dir.join(f).exists())
            .and_then(|f| std::fs::read_to_string(dir.join(f)).ok())
            .map(|c| c.contains("coverage:") || c.contains("coverage :"))
            .unwrap_or(false);

        let mut args = vec!["run".into(), "--reporter=verbose".into()];
        if coverage_enabled {
            args.push("--coverage".into());
        }

        suites.push(Suite {
            id,
            name:    "Unit Tests".into(),
            system:  folder.clone(),
            tag:     "Unit".into(),
            command: "vitest".into(),
            args,
            cwd:     rel.clone(),
        });
    }

    // ── package.json npm scripts ──────────────────────────────────────────
    let pkg_path = dir.join("package.json");
    if pkg_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
                    for (script_name, script_val) in scripts {
                        // Only pick up "test" or "test:*" scripts
                        if script_name != "test" && !script_name.starts_with("test:") { continue; }
                        // Skip echo stubs (placeholder scripts)
                        let val = script_val.as_str().unwrap_or("");
                        if val.starts_with("echo") { continue; }
                        // Skip scripts that are just wrappers around an already-detected runner
                        let val_lower = val.to_lowercase();
                        if has_vitest && val_lower.contains("vitest") { continue; }
                        if has_pw && val_lower.contains("playwright") { continue; }

                        let tag = {
                            let s = script_name.to_lowercase();
                            if s.contains("e2e") { "E2E" }
                            else if s.contains("unit") { "Unit" }
                            else if s.contains("api") { "API" }
                            else { "Unit" }
                        }.to_string();

                        let name = if script_name == "test" {
                            "Tests".to_string()
                        } else {
                            script_name.splitn(2, ':')
                                .nth(1).unwrap_or(script_name)
                                .split(':')
                                .map(|p| {
                                    let mut chars = p.chars();
                                    match chars.next() {
                                        None => String::new(),
                                        Some(ch) => ch.to_uppercase().to_string() + chars.as_str(),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        };

                        let id = if rel.is_empty() {
                            format!("npm-{}", script_name.replace(':', "-"))
                        } else {
                            format!("{}-npm-{}", rel.replace('/', "-"), script_name.replace(':', "-"))
                        };

                        suites.push(Suite {
                            id,
                            name,
                            system:  folder.clone(),
                            tag,
                            command: "npm".into(),
                            args:    vec!["run".into(), script_name.clone()],
                            cwd:     rel.clone(),
                        });
                    }
                }
            }
        }
    }

    // ── Recurse ───────────────────────────────────────────────────────────
    // Only recurse if no config was found here. A config file means this
    // directory owns its test scope — scanning deeper would create duplicates.
    if !has_pw && !has_vitest {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut subdirs: Vec<PathBuf> = entries.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.path())
                .collect();
            subdirs.sort();
            for sub in subdirs {
                scan_dir(root, &sub, depth + 1, suites);
            }
        }
    }
}

#[tauri::command]
fn scan_project(path: String) -> Result<Vec<Suite>, String> {
    let root = PathBuf::from(&path);
    if !root.exists() {
        return Err(format!("Caminho não encontrado: {}", path));
    }

    let mut suites: Vec<Suite> = Vec::new();
    scan_dir(&root, &root, 0, &mut suites);

    // Deduplicate by id using a HashSet (dedup_by only removes adjacent duplicates)
    let mut seen = std::collections::HashSet::new();
    suites.retain(|s| seen.insert(s.id.clone()));

    if suites.is_empty() {
        return Err(
            "Nenhuma suite encontrada (até 4 níveis de profundidade).\n\
             Certifique-se de que o projeto contém playwright.config.ts ou vitest.config.ts.".into()
        );
    }

    Ok(suites)
}

// ── Run / Stop ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn run_suite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    project_name: String,
    project_path: String,
    suite_id: String,
    suite_name: String,
    suite_tag: String,
    suite_command: String,
    suite_cwd: String,
    suite_args: Vec<String>,
) -> Result<(), String> {
    // Cancel previous run
    {
        let mut running = state.running.lock().unwrap();
        if let Some(tx) = running.remove(&suite_id) { let _ = tx.send(()); }
    }

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    state.running.lock().unwrap().insert(suite_id.clone(), cancel_tx);

    // Store run metadata for DB persistence on completion
    let run_id = Uuid::new_v4().to_string();
    let started_at = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // SQLite ISO format
        format!("{}", secs)
    };

    state.run_meta.lock().unwrap().insert(suite_id.clone(), RunMeta {
        run_id: run_id.clone(),
        project_id: project_id.clone(),
        project_name: project_name.clone(),
        suite_name: suite_name.clone(),
        suite_tag: suite_tag.clone(),
        started_at: started_at.clone(),
        pass_count: 0,
        fail_count: 0,
        pid: None,
        output_lines: Vec::new(),
        project_path: project_path.clone(),
        suite_cwd: suite_cwd.clone(),
    });

    let _ = app.emit("suite-started", StatusEvent {
        suite_id: suite_id.clone(),
        status: "running".into(),
        duration: None,
        exit_code: None,
    });

    let pw_dir = if suite_cwd.is_empty() {
        PathBuf::from(&project_path)
    } else {
        PathBuf::from(&project_path).join(&suite_cwd)
    };

    // Clone app — AppHandle is 'static and can be moved into tokio::spawn
    let app_task = app.clone();

    tokio::spawn(async move {
        let app = app_task;
        let start = std::time::Instant::now();
        let sid = suite_id.clone();

        // Access state through AppHandle inside the task (lifetime is 'static here)
        let state = app.state::<AppState>();

        let mut cmd = Command::new("cmd");
        cmd.arg("/C");
        if suite_command == "npm" {
            cmd.arg("npm");
        } else {
            cmd.arg("npx");
            cmd.arg(&suite_command);
        }
        cmd.args(&suite_args);
        cmd.current_dir(&pw_dir);
        cmd.env("FORCE_COLOR", "0");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("suite-output", OutputEvent { suite_id: sid.clone(), line: format!("Erro ao iniciar: {}", e) });
                let _ = app.emit("suite-done", StatusEvent { suite_id: sid.clone(), status: "failed".into(), duration: Some(0), exit_code: Some(-1) });
                save_run_to_db(&app, &sid, "failed", 0);
                return;
            }
        };

        // Store child PID immediately so stop_suite can kill the process tree
        if let Some(pid) = child.id() {
            let mut meta = state.run_meta.lock().unwrap();
            if let Some(m) = meta.get_mut(&sid) { m.pid = Some(pid); }
        }

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let app_out = app.clone(); let sid_out = sid.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                if !clean.trim().is_empty() {
                    {
                        let s = app_out.state::<AppState>();
                        let mut meta = s.run_meta.lock().unwrap();
                        if let Some(m) = meta.get_mut(&sid_out) {
                            let t = clean.trim_start();
                            if t.starts_with('✓') || t.starts_with('✔') { m.pass_count += 1; }
                            if t.starts_with('✗') || t.starts_with('×') { m.fail_count += 1; }
                            m.output_lines.push(clean.clone());
                        }
                    }
                    let _ = app_out.emit("suite-output", OutputEvent { suite_id: sid_out.clone(), line: clean });
                }
            }
        });

        let app_err = app.clone(); let sid_err = sid.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                if !clean.trim().is_empty() {
                    {
                        let s = app_err.state::<AppState>();
                        let mut meta = s.run_meta.lock().unwrap();
                        if let Some(m) = meta.get_mut(&sid_err) {
                            m.output_lines.push(clean.clone());
                        }
                    }
                    let _ = app_err.emit("suite-output", OutputEvent { suite_id: sid_err.clone(), line: clean });
                }
            }
        });

        let (final_status, exit_code) = tokio::select! {
            result = child.wait() => {
                // Abort instead of await: if any child process still holds the pipe
                // open (e.g. node.exe orphaned after npx exits), awaiting would hang forever.
                stdout_task.abort(); stderr_task.abort();
                match result {
                    Ok(s) => { let c = s.code().unwrap_or(-1); (if c == 0 { "passed" } else { "failed" }.to_string(), c) }
                    Err(_) => ("failed".to_string(), -1),
                }
            }
            _ = cancel_rx => {
                let _ = child.kill().await;
                stdout_task.abort(); stderr_task.abort();
                ("stopped".to_string(), -1)
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        save_run_to_db(&app, &sid, &final_status, duration_ms);

        let _ = app.emit("suite-done", StatusEvent {
            suite_id: sid,
            status: final_status,
            duration: Some(duration_ms),
            exit_code: Some(exit_code),
        });

        // Remove from running map
        state.running.lock().unwrap().remove(&suite_id);
    });

    Ok(())
}

fn save_run_to_db(app: &tauri::AppHandle, suite_id: &str, status: &str, duration_ms: u64) {
    let state = app.state::<AppState>();
    let meta = {
        let mut map = state.run_meta.lock().unwrap();
        map.remove(suite_id)
    };

    if let Some(m) = meta {
        let output_json = serde_json::to_string(&m.output_lines).unwrap_or_default();
        let coverage = read_coverage(&m.project_path, &m.suite_cwd);
        let db = state.db.lock().unwrap();
        let _ = db.execute(
            "INSERT INTO test_runs (id,project_id,project_name,suite_id,suite_name,suite_tag,status,duration_ms,pass_count,fail_count,started_at,output,coverage_pct)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,datetime(?11,'unixepoch'),?12,?13)",
            params![
                m.run_id, m.project_id, m.project_name,
                suite_id, m.suite_name, m.suite_tag,
                status, duration_ms as i64, m.pass_count, m.fail_count,
                m.started_at, output_json, coverage
            ],
        );
        // Emit updated history to frontend
        let _ = app.emit("runs-updated", ());
    }
}

#[tauri::command]
async fn stop_suite(state: State<'_, AppState>, suite_id: String) -> Result<(), String> {
    // Get the child PID from run_meta to kill the whole process tree
    let pid = {
        let meta = state.run_meta.lock().unwrap();
        meta.get(&suite_id).and_then(|m| m.pid)
    };

    // Kill the entire process tree (works on Windows where child.kill() is not enough)
    if let Some(pid) = pid {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output()
            .await;
    }

    // Send cancel signal so the spawn task cleans up and emits suite-done
    let mut running = state.running.lock().unwrap();
    if let Some(tx) = running.remove(&suite_id) { let _ = tx.send(()); }
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let db_path = db_path(&app.handle());
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let conn = Connection::open(&db_path)
                .expect("failed to open database");
            init_db(&conn);
            app.manage(AppState {
                db:       Mutex::new(conn),
                running:  Mutex::new(HashMap::new()),
                run_meta: Mutex::new(HashMap::new()),
            });
            scheduler::start_scheduler_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            save_pdf,
            get_projects,
            save_project,
            delete_project,
            scan_project,
            run_suite,
            stop_suite,
            get_runs,
            get_run_output,
            scheduler::get_schedules,
            scheduler::save_schedule,
            scheduler::delete_schedule,
            scheduler::toggle_schedule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
