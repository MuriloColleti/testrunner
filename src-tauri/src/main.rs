#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

mod scheduler;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Suite {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) system: String,
    pub(crate) tag: String,       // "E2E" | "API" | "Unit"
    pub(crate) command: String,   // "playwright" | "vitest"
    pub(crate) args: Vec<String>,
    pub(crate) cwd: String,       // path relative to project root where command runs
}

/// Everything needed to execute a suite — built either from the `run_suite`
/// command (frontend) or by the scheduler from data persisted in the DB.
#[derive(Debug, Clone)]
pub(crate) struct ExecParams {
    pub(crate) project_id:    String,
    pub(crate) project_name:  String,
    pub(crate) project_path:  String,
    pub(crate) suite_id:      String,
    pub(crate) suite_name:    String,
    pub(crate) suite_tag:     String,
    pub(crate) suite_command: String,
    pub(crate) suite_cwd:     String,
    pub(crate) suite_args:    Vec<String>,
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

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
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

/// Atualiza os contadores de pass/fail a partir de uma linha de output do
/// runner (Playwright line-reporter, Vitest/Jest/Mocha unicode, sumários).
/// Linhas de build (Vite e afins) usam ✓ e "passed"-like sem serem testes —
/// ex.: "✓ 2658 modules transformed.", "✓ built in 27s", "dist/assets/...".
fn is_build_noise(t: &str) -> bool {
    t.contains("modules transformed")
        || t.contains("built in")
        || t.starts_with("dist/")
        || t.starts_with("transforming")
        || t.starts_with("rendering chunks")
        || t.starts_with("computing gzip")
}

fn update_counts_from_line(line: &str, pass_count: &mut i64, fail_count: &mut i64) {
    let t = line.trim_start();
    if is_build_noise(t) { return; }
    // Unicode check marks (Jest/Vitest/Mocha)
    if t.starts_with('✓') || t.starts_with('✔') { *pass_count += 1; }
    // Unicode cross marks (Jest/Vitest/Mocha)
    if t.starts_with('✗') || t.starts_with('×') { *fail_count += 1; }
    // Playwright line-reporter: "ok  N [worker] > ..."
    if t.starts_with("ok ") {
        let rest = t[3..].trim_start();
        if rest.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            *pass_count += 1;
        }
    }
    // Playwright line-reporter: "not ok  N [worker] > ..." or TAP fails
    if t.starts_with("not ok") { *fail_count += 1; }
    // Summary lines: "25 passed", "3 failed"
    // (overwrite if summary is higher — catches Cypress/Playwright totals)
    if let Some(idx) = t.find(" passed") {
        let before = &t[..idx];
        if let Some(n) = before.split_whitespace().last()
            .and_then(|s| s.parse::<i64>().ok())
        {
            if n > *pass_count { *pass_count = n; }
        }
    }
    if let Some(idx) = t.find(" failed") {
        let before = &t[..idx];
        if let Some(n) = before.split_whitespace().last()
            .and_then(|s| s.parse::<i64>().ok())
        {
            if n > *fail_count { *fail_count = n; }
        }
    }
}

// ── Preflight validation ──────────────────────────────────────────────────────

const PW_CONFIGS: &[&str] = &["playwright.config.ts", "playwright.config.js", "playwright.config.mjs"];
const VITEST_CONFIGS: &[&str] = &["vitest.config.ts", "vitest.config.js", "vitest.config.mjs", "vitest.config.cjs"];

struct PreflightIssue {
    fatal: bool,
    message: String,
}

fn issue(fatal: bool, message: String) -> PreflightIssue {
    PreflightIssue { fatal, message }
}

/// Extrai a primeira baseURL http(s) citada no conteúdo de um playwright.config.
/// Heurística de texto — cobre `baseURL: 'http://...'` e o padrão
/// `baseURL: process.env.X || 'http://...'`.
fn extract_base_url(config: &str) -> Option<String> {
    let mut rest = config;
    while let Some(idx) = rest.find("baseURL") {
        let after = &rest[idx + "baseURL".len()..];
        let mut limit = after.len().min(200);
        while !after.is_char_boundary(limit) { limit -= 1; }
        let window = &after[..limit];
        if let Some(q) = window.find(['\'', '"', '`']) {
            let quote = window[q..].chars().next().unwrap();
            let body = &window[q + quote.len_utf8()..];
            if let Some(end) = body.find(quote) {
                let url = &body[..end];
                if url.starts_with("http://") || url.starts_with("https://") {
                    return Some(url.to_string());
                }
            }
        }
        rest = after;
    }
    None
}

/// Host local = a aplicação precisa estar rodando NESTA máquina; se não
/// responde, o teste certamente falharia. Host remoto pode estar atrás de
/// firewall/VPN e ainda funcionar — não deve bloquear a execução.
fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.starts_with("127.")
        || host == "0.0.0.0"
        || host == "::1"
        || host == "[::1]"
}

/// Separa host e porta de uma URL http(s), com porta padrão por esquema.
fn host_port(url: &str) -> Option<(String, u16)> {
    let (rest, default_port) = url.strip_prefix("https://").map(|r| (r, 443u16))
        .or_else(|| url.strip_prefix("http://").map(|r| (r, 80u16)))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() { return None; }
    if let Some((host, port)) = authority.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return Some((host.to_string(), port));
        }
    }
    Some((authority.to_string(), default_port))
}

/// Valida estrutura do projeto e disponibilidade de servidores antes de rodar
/// a suite. Issues fatais abortam a execução com o motivo no terminal.
async fn preflight_suite(
    project_path: &str,
    suite_cwd: &str,
    suite_command: &str,
    suite_args: &[String],
    suite_tag: &str,
) -> Vec<PreflightIssue> {
    let mut issues: Vec<PreflightIssue> = Vec::new();

    let root = PathBuf::from(project_path);
    if !root.exists() {
        issues.push(issue(true, format!("[ERRO] Pasta do projeto não encontrada: {}", project_path)));
        return issues;
    }

    let dir = if suite_cwd.is_empty() { root.clone() } else { root.join(suite_cwd) };
    if !dir.exists() {
        issues.push(issue(true, format!("[ERRO] Pasta da suite não encontrada: {}", dir.display())));
        return issues;
    }

    // Estrutura esperada pelo runner
    match suite_command {
        "playwright" => {
            if !PW_CONFIGS.iter().any(|f| dir.join(f).exists()) {
                issues.push(issue(true, format!("[ERRO] playwright.config não encontrado em {}", dir.display())));
            }
        }
        "vitest" => {
            if !VITEST_CONFIGS.iter().any(|f| dir.join(f).exists()) {
                issues.push(issue(true, format!("[ERRO] vitest.config não encontrado em {}", dir.display())));
            }
        }
        "npm" => {
            let script = suite_args.get(1).cloned().unwrap_or_default();
            match std::fs::read_to_string(dir.join("package.json")) {
                Ok(content) => {
                    let has_script = serde_json::from_str::<serde_json::Value>(&content).ok()
                        .and_then(|p| p.get("scripts")?.get(&script).map(|_| ()))
                        .is_some();
                    if !has_script {
                        issues.push(issue(true, format!(
                            "[ERRO] Script \"{}\" não existe no package.json de {}", script, dir.display()
                        )));
                    }
                }
                Err(_) => issues.push(issue(true, format!("[ERRO] package.json não encontrado em {}", dir.display()))),
            }
        }
        _ => {}
    }

    // Dependências instaladas
    if !dir.join("node_modules").exists() && !root.join("node_modules").exists() {
        issues.push(issue(true, format!(
            "[ERRO] node_modules não encontrado — rode \"npm install\" em {}", dir.display()
        )));
    }

    // Servidor no ar para suites Playwright (E2E/API) sem webServer gerenciado
    if suite_command == "playwright" {
        let cfg = PW_CONFIGS.iter()
            .find(|f| dir.join(f).exists())
            .and_then(|f| std::fs::read_to_string(dir.join(f)).ok());
        if let Some(cfg) = cfg {
            if cfg.contains("webServer") {
                issues.push(issue(false, "[OK] webServer configurado — o Playwright sobe a aplicação sozinho".into()));
            } else if let Some(url) = extract_base_url(&cfg) {
                if let Some((host, port)) = host_port(&url) {
                    let reachable = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        tokio::net::TcpStream::connect((host.as_str(), port)),
                    ).await.map(|r| r.is_ok()).unwrap_or(false);
                    if reachable {
                        issues.push(issue(false, format!("[OK] Aplicação respondendo em {}", url)));
                    } else if is_local_host(&host) {
                        issues.push(issue(true, format!(
                            "[ERRO] Aplicação não está respondendo em {} — inicie o servidor local antes de rodar testes {}",
                            url, suite_tag
                        )));
                    } else {
                        // Servidor remoto: sonda TCP pode falhar por firewall/VPN
                        // mesmo com a aplicação no ar — avisa mas não bloqueia
                        issues.push(issue(false, format!(
                            "[AVISO] Sem resposta de {} (servidor remoto) — prosseguindo mesmo assim",
                            url
                        )));
                    }
                }
            } else {
                issues.push(issue(false,
                    "[AVISO] baseURL não encontrada no playwright.config — verificação de servidor ignorada".into()));
            }
        }
    }

    issues
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
async fn save_pdf(filename: String, data: String) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = STANDARD.decode(&data).map_err(|e| e.to_string())?;

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
            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
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
    execute_suite(&app, ExecParams {
        project_id, project_name, project_path,
        suite_id, suite_name, suite_tag,
        suite_command, suite_cwd, suite_args,
    });
    Ok(())
}

/// Builds the platform-specific command that runs a suite.
fn build_suite_command(suite_command: &str, suite_args: &[String], cwd: &PathBuf) -> Command {
    #[cfg(windows)]
    let mut cmd = {
        // npm/npx are .cmd batch files on Windows — they need cmd.exe
        let mut c = Command::new("cmd");
        c.arg("/C");
        if suite_command == "npm" {
            c.arg("npm");
        } else {
            c.arg("npx");
            c.arg(suite_command);
        }
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        if suite_command == "npm" {
            Command::new("npm")
        } else {
            let mut c = Command::new("npx");
            c.arg(suite_command);
            c
        }
    };

    cmd.args(suite_args);
    cmd.current_dir(cwd);
    cmd.env("FORCE_COLOR", "0");
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    #[cfg(unix)]
    cmd.process_group(0); // own process group → stop_suite can kill the whole tree

    cmd
}

/// Runs a suite end-to-end: spawns the process, streams output as events,
/// parses pass/fail counts and persists the result. Shared by the `run_suite`
/// command and the scheduler, so scheduled runs work without the webview.
pub(crate) fn execute_suite(app: &tauri::AppHandle, p: ExecParams) {
    let state = app.state::<AppState>();
    let ExecParams {
        project_id, project_name, project_path,
        suite_id, suite_name, suite_tag,
        suite_command, suite_cwd, suite_args,
    } = p;

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

    // Clone app — AppHandle is 'static and can be moved into the spawned task
    let app_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let app = app_task;
        let start = std::time::Instant::now();
        let sid = suite_id.clone();

        // Access state through AppHandle inside the task (lifetime is 'static here)
        let state = app.state::<AppState>();

        // Pré-validação: estrutura do projeto e servidores no ar
        let issues = preflight_suite(&project_path, &suite_cwd, &suite_command, &suite_args, &suite_tag).await;
        for i in &issues {
            {
                let mut meta = state.run_meta.lock().unwrap();
                if let Some(m) = meta.get_mut(&sid) { m.output_lines.push(i.message.clone()); }
            }
            let _ = app.emit("suite-output", OutputEvent { suite_id: sid.clone(), line: i.message.clone() });
        }
        if issues.iter().any(|i| i.fatal) {
            let msg = "Execução abortada pela pré-validação.".to_string();
            {
                let mut meta = state.run_meta.lock().unwrap();
                if let Some(m) = meta.get_mut(&sid) { m.output_lines.push(msg.clone()); }
            }
            let _ = app.emit("suite-output", OutputEvent { suite_id: sid.clone(), line: msg });
            save_run_to_db(&app, &sid, "failed", 0);
            let _ = app.emit("suite-done", StatusEvent {
                suite_id: sid.clone(),
                status: "failed".into(),
                duration: Some(0),
                exit_code: Some(-1),
            });
            state.running.lock().unwrap().remove(&sid);
            return;
        }

        let mut cmd = build_suite_command(&suite_command, &suite_args, &pw_dir);

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
                            update_counts_from_line(&clean, &mut m.pass_count, &mut m.fail_count);
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

    // Kill the entire process tree — child.kill() alone leaves grandchildren
    // (node spawned by npm/npx) running on both platforms
    if let Some(pid) = pid {
        #[cfg(windows)]
        let _ = tokio::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output()
            .await;
        #[cfg(not(windows))]
        // Spawned with process_group(0), so pgid == pid; negative pid kills the group
        let _ = tokio::process::Command::new("kill")
            .args(["-9", &format!("-{}", pid)])
            .output()
            .await;
    }

    // Send cancel signal so the spawn task cleans up and emits suite-done
    let mut running = state.running.lock().unwrap();
    if let Some(tx) = running.remove(&suite_id) { let _ = tx.send(()); }
    Ok(())
}

// ── Autostart ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }
        .map_err(|e| e.to_string())
}

/// Liga o autostart uma única vez, no primeiro boot. Depois disso quem manda
/// é a escolha do usuário (toggle na UI) — nunca religamos sozinhos.
fn configure_autostart_once(app: &tauri::App, conn: &Connection) {
    let configured = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'autostart_configured'",
            [],
            |row| row.get::<_, String>(0),
        )
        .is_ok();
    if configured { return; }

    if app.autolaunch().enable().is_ok() {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('autostart_configured', '1')",
            [],
        );
    }
}

// ── System tray ───────────────────────────────────────────────────────────────

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_item    = MenuItem::with_id(app, "open",    "Abrir",                     true, None::<&str>)?;
    let run_due_item = MenuItem::with_id(app, "run-due", "Executar agendados agora",  true, None::<&str>)?;
    let quit_item    = MenuItem::with_id(app, "quit",    "Sair",                      true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &run_due_item, &quit_item])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("bundled window icon missing").clone())
        .tooltip("TestRunner")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open"    => show_main_window(app),
            "run-due" => scheduler::check_and_fire(app),
            "quit"    => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Clique esquerdo abre a janela. No Linux (appindicator) eventos de
            // clique não são entregues — lá o item "Abrir" do menu cobre isso.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        // Deve ser o primeiro plugin: uma segunda instância só foca a existente
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let db_path = db_path(&app.handle());
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let conn = Connection::open(&db_path)
                .expect("failed to open database");
            init_db(&conn);
            configure_autostart_once(app, &conn);
            app.manage(AppState {
                db:       Mutex::new(conn),
                running:  Mutex::new(HashMap::new()),
                run_meta: Mutex::new(HashMap::new()),
            });
            setup_tray(app)?;
            scheduler::start_scheduler_loop(app.handle().clone());

            // A janela é criada oculta (visible: false no tauri.conf.json).
            // Boot normal mostra; boot via autostart (--hidden) fica só na bandeja.
            let start_hidden = std::env::args().any(|a| a == "--hidden");
            if !start_hidden {
                show_main_window(&app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fechar a janela esconde para a bandeja — o processo (e o
            // scheduler) continua vivo. Sair de verdade é pelo menu da bandeja.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
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
            get_autostart,
            set_autostart,
            scheduler::get_schedules,
            scheduler::save_schedule,
            scheduler::delete_schedule,
            scheduler::toggle_schedule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── strip_ansi ────────────────────────────────────────────────────────

    #[test]
    fn strip_ansi_removes_escape_sequences() {
        assert_eq!(strip_ansi("\x1b[32m✓ passou\x1b[0m"), "✓ passou");
        assert_eq!(strip_ansi("\x1b[1;31merro\x1b[0m fatal"), "erro fatal");
    }

    #[test]
    fn strip_ansi_removes_carriage_returns() {
        assert_eq!(strip_ansi("linha\r\n".trim_end_matches('\n')), "linha");
    }

    #[test]
    fn strip_ansi_keeps_plain_text() {
        assert_eq!(strip_ansi("ok  1 [chromium] > login"), "ok  1 [chromium] > login");
    }

    // ── update_counts_from_line ───────────────────────────────────────────

    fn count(lines: &[&str]) -> (i64, i64) {
        let (mut pass, mut fail) = (0, 0);
        for l in lines { update_counts_from_line(l, &mut pass, &mut fail); }
        (pass, fail)
    }

    #[test]
    fn counts_unicode_marks() {
        assert_eq!(count(&["✓ soma", "✔ subtração", "✗ divisão", "× módulo"]), (2, 2));
    }

    #[test]
    fn counts_playwright_line_reporter() {
        assert_eq!(
            count(&[
                "ok  1 [chromium] > login.spec.ts > deve logar",
                "ok  2 [chromium] > login.spec.ts > deve deslogar",
                "not ok  3 [chromium] > cart.spec.ts > checkout",
            ]),
            (2, 1)
        );
    }

    #[test]
    fn ok_without_number_is_not_a_test() {
        // "ok " seguido de texto não numérico não é resultado de teste
        assert_eq!(count(&["ok tudo certo por aqui"]), (0, 0));
    }

    #[test]
    fn summary_overrides_lower_counts() {
        // Contagem linha-a-linha perdeu testes; o sumário corrige para cima
        assert_eq!(count(&["✓ um", "25 passed (30s)", "3 failed"]), (25, 3));
    }

    #[test]
    fn build_output_is_not_counted_as_tests() {
        assert_eq!(
            count(&[
                "✓ 2658 modules transformed.",
                "✓ built in 27.31s",
                "dist/assets/index-CCUEPQSR.css  548.32 kB | gzip: 67.57 kB",
                "transforming ...",
                "rendering chunks ...",
                "computing gzip size ...",
            ]),
            (0, 0)
        );
    }

    #[test]
    fn summary_does_not_lower_counts() {
        let (mut pass, mut fail) = (10, 5);
        update_counts_from_line("2 passed", &mut pass, &mut fail);
        update_counts_from_line("1 failed", &mut pass, &mut fail);
        assert_eq!((pass, fail), (10, 5));
    }

    // ── extract_base_url / host_port (preflight) ──────────────────────────

    #[test]
    fn base_url_from_double_quotes() {
        let cfg = r#"use: { baseURL: "http://localhost:5173", trace: "on" }"#;
        assert_eq!(extract_base_url(cfg), Some("http://localhost:5173".into()));
    }

    #[test]
    fn base_url_from_single_quotes() {
        let cfg = "use: { baseURL: 'https://staging.znap.com.br/app' }";
        assert_eq!(extract_base_url(cfg), Some("https://staging.znap.com.br/app".into()));
    }

    #[test]
    fn base_url_with_env_fallback() {
        let cfg = "baseURL: process.env.BASE_URL || 'http://127.0.0.1:3000',";
        assert_eq!(extract_base_url(cfg), Some("http://127.0.0.1:3000".into()));
    }

    #[test]
    fn base_url_none_when_absent() {
        assert_eq!(extract_base_url("export default { retries: 2 }"), None);
    }

    #[test]
    fn host_port_explicit() {
        assert_eq!(host_port("http://localhost:5173/app"), Some(("localhost".into(), 5173)));
    }

    #[test]
    fn host_port_defaults_by_scheme() {
        assert_eq!(host_port("http://meuapp.local"), Some(("meuapp.local".into(), 80)));
        assert_eq!(host_port("https://meuapp.local/x?q=1"), Some(("meuapp.local".into(), 443)));
    }

    #[test]
    fn host_port_rejects_non_http() {
        assert_eq!(host_port("ws://localhost:8080"), None);
    }

    #[test]
    fn local_hosts_are_detected() {
        assert!(is_local_host("localhost"));
        assert!(is_local_host("LocalHost"));
        assert!(is_local_host("127.0.0.1"));
        assert!(is_local_host("::1"));
        assert!(!is_local_host("staging.znap.com.br"));
        assert!(!is_local_host("192.168.0.42"));
    }

    // ── read_coverage ─────────────────────────────────────────────────────

    #[test]
    fn read_coverage_parses_summary_json() {
        let dir = std::env::temp_dir().join(format!("testrunner-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("coverage")).unwrap();
        std::fs::write(
            dir.join("coverage").join("coverage-summary.json"),
            r#"{"total":{"lines":{"total":100,"covered":85,"pct":85.5}}}"#,
        ).unwrap();

        let pct = read_coverage(&dir.to_string_lossy(), "");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(pct, Some(85.5));
    }

    #[test]
    fn read_coverage_none_when_missing() {
        assert_eq!(read_coverage("Z:/caminho/que/nao/existe", ""), None);
    }

    #[test]
    fn read_coverage_joins_suite_cwd() {
        let dir = std::env::temp_dir().join(format!("testrunner-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("web").join("coverage")).unwrap();
        std::fs::write(
            dir.join("web").join("coverage").join("coverage-summary.json"),
            r#"{"total":{"lines":{"pct":42.0}}}"#,
        ).unwrap();

        let pct = read_coverage(&dir.to_string_lossy(), "web");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(pct, Some(42.0));
    }
}
