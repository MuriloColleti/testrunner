#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Suite {
    id: String,
    name: String,
    system: String,
    tag: String,
    args: Vec<String>,
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

struct AppState {
    running: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

// ── Suite definitions ─────────────────────────────────────────────────────────

fn suites() -> Vec<Suite> {
    let pw = |module: &str, project: &str| -> Vec<String> {
        vec![
            "test".into(),
            format!("modules/{}", module),
            format!("--project={}", project),
            "--reporter=list".into(),
        ]
    };

    vec![
        Suite { id: "fluke-frontend".into(),    name: "Frontend".into(),    system: "Fluke".into(),     tag: "E2E".into(), args: pw("fluke/frontend",    "chromium") },
        Suite { id: "fluke-backend".into(),     name: "Backend API".into(), system: "Fluke".into(),     tag: "API".into(), args: pw("fluke/backend",     "api")      },
        Suite { id: "vert-frontend".into(),     name: "Frontend".into(),    system: "Vert".into(),      tag: "E2E".into(), args: pw("vert/frontend",     "chromium") },
        Suite { id: "atmo-frontend".into(),     name: "Frontend".into(),    system: "Atmo".into(),      tag: "E2E".into(), args: pw("atmo/frontend",     "chromium") },
        Suite { id: "venturusv2-frontend".into(),name: "Frontend".into(),   system: "VenturusV2".into(),tag: "E2E".into(), args: pw("venturusV2/frontend","chromium") },
        Suite { id: "nissin-frontend".into(),   name: "Frontend".into(),    system: "Nissin".into(),    tag: "E2E".into(), args: pw("nissin/frontend",   "chromium") },
        Suite { id: "m2m-frontend".into(),      name: "Frontend".into(),    system: "M2M".into(),       tag: "E2E".into(), args: pw("m2m/frontend",      "chromium") },
    ]
}

fn playwright_dir() -> PathBuf {
    PathBuf::from(r"C:\Users\mu_co\Desktop\znap\Playwringht---Testes")
}

// ── ANSI stripper ─────────────────────────────────────────────────────────────

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for ch in chars.by_ref() {
                if ch.is_ascii_alphabetic() { break; }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_suites() -> Vec<Suite> {
    suites()
}

#[tauri::command]
async fn run_suite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    suite_id: String,
) -> Result<(), String> {
    let suite = suites()
        .into_iter()
        .find(|s| s.id == suite_id)
        .ok_or_else(|| format!("Suite '{}' not found", suite_id))?;

    // Cancel previous run for this suite if running
    {
        let mut running = state.running.lock().unwrap();
        if let Some(tx) = running.remove(&suite_id) {
            let _ = tx.send(());
        }
    }

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    state.running.lock().unwrap().insert(suite_id.clone(), cancel_tx);

    let _ = app.emit("suite-started", StatusEvent {
        suite_id: suite_id.clone(),
        status: "running".into(),
        duration: None,
        exit_code: None,
    });

    let pw_dir = playwright_dir();

    tokio::spawn(async move {
        let start = std::time::Instant::now();
        let sid = suite.id.clone();

        let mut cmd = Command::new("npx");
        cmd.arg("playwright");
        cmd.args(&suite.args);
        cmd.current_dir(&pw_dir);
        cmd.env("FORCE_COLOR", "0");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("suite-output", OutputEvent { suite_id: sid.clone(), line: format!("Erro ao iniciar: {}", e) });
                let _ = app.emit("suite-done", StatusEvent { suite_id: sid, status: "failed".into(), duration: Some(0), exit_code: Some(-1) });
                return;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Stream stdout
        let app_out = app.clone();
        let sid_out = sid.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                if !clean.trim().is_empty() {
                    let _ = app_out.emit("suite-output", OutputEvent { suite_id: sid_out.clone(), line: clean });
                }
            }
        });

        // Stream stderr
        let app_err = app.clone();
        let sid_err = sid.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                if !clean.trim().is_empty() {
                    let _ = app_err.emit("suite-output", OutputEvent { suite_id: sid_err.clone(), line: clean });
                }
            }
        });

        let (final_status, exit_code) = tokio::select! {
            result = child.wait() => {
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                match result {
                    Ok(s) => {
                        let code = s.code().unwrap_or(-1);
                        (if code == 0 { "passed" } else { "failed" }.to_string(), code)
                    }
                    Err(_) => ("failed".to_string(), -1),
                }
            }
            _ = cancel_rx => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                ("stopped".to_string(), -1)
            }
        };

        let _ = app.emit("suite-done", StatusEvent {
            suite_id: sid,
            status: final_status,
            duration: Some(start.elapsed().as_millis() as u64),
            exit_code: Some(exit_code),
        });
    });

    Ok(())
}

#[tauri::command]
async fn stop_suite(
    state: State<'_, AppState>,
    suite_id: String,
) -> Result<(), String> {
    let mut running = state.running.lock().unwrap();
    if let Some(tx) = running.remove(&suite_id) {
        let _ = tx.send(());
    }
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            running: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![get_suites, run_suite, stop_suite])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
