use std::time::Duration;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use crate::{AppState, ExecParams, Suite};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id:           String,
    pub project_id:   String,
    pub suite_id:     String,
    pub label:        String,
    pub scheduled_at: String,   // "YYYY-MM-DDTHH:MM:SS" horário local
    pub recurrence:   String,   // "once" | "daily" | "weekly"
    pub enabled:      bool,
    pub last_run_at:  Option<String>,
    pub created_at:   String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleTriggeredEvent {
    schedule_id: String,
    project_id:  String,
    suite_id:    String,
    label:       String,
}

// ── CRUD commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_schedules(state: State<'_, AppState>) -> Result<Vec<Schedule>, String> {
    let db = state.db.lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, project_id, suite_id, label, scheduled_at, recurrence, \
                    enabled, last_run_at, created_at \
             FROM schedules ORDER BY scheduled_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let schedules = stmt
        .query_map([], |row| {
            Ok(Schedule {
                id:           row.get(0)?,
                project_id:   row.get(1)?,
                suite_id:     row.get(2)?,
                label:        row.get(3)?,
                scheduled_at: row.get(4)?,
                recurrence:   row.get(5)?,
                enabled:      row.get::<_, i64>(6)? != 0,
                last_run_at:  row.get(7)?,
                created_at:   row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(schedules)
}

#[tauri::command]
pub fn save_schedule(state: State<'_, AppState>, schedule: Schedule) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.execute(
        "INSERT INTO schedules \
             (id, project_id, suite_id, label, scheduled_at, recurrence, enabled, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now', 'localtime')) \
         ON CONFLICT(id) DO UPDATE SET \
             project_id   = excluded.project_id, \
             suite_id     = excluded.suite_id, \
             label        = excluded.label, \
             scheduled_at = excluded.scheduled_at, \
             recurrence   = excluded.recurrence, \
             enabled      = excluded.enabled",
        params![
            schedule.id,
            schedule.project_id,
            schedule.suite_id,
            schedule.label,
            schedule.scheduled_at,
            schedule.recurrence,
            schedule.enabled as i64,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(state: State<'_, AppState>, schedule_id: String) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.execute("DELETE FROM schedules WHERE id = ?1", params![schedule_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_schedule(
    state: State<'_, AppState>,
    schedule_id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.execute(
        "UPDATE schedules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, schedule_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Background scheduler loop ─────────────────────────────────────────────────

type DueTuple = (String, String, String, String, String);

fn get_due_schedules(db: &rusqlite::Connection) -> rusqlite::Result<Vec<DueTuple>> {
    let mut stmt = db.prepare(
        "SELECT id, project_id, suite_id, label, recurrence \
         FROM schedules \
         WHERE enabled = 1 \
           AND datetime(scheduled_at) <= datetime('now', 'localtime') \
           AND (last_run_at IS NULL OR last_run_at < scheduled_at)",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    rows.collect()
}

/// Monta os parâmetros de execução a partir do que está persistido no banco,
/// sem depender do frontend estar aberto.
fn resolve_exec_params(
    db: &rusqlite::Connection,
    project_id: &str,
    suite_id: &str,
) -> Option<ExecParams> {
    let (name, path, suites_json) = db
        .query_row(
            "SELECT name, path, suites FROM projects WHERE id = ?1",
            params![project_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .ok()?;

    let suites: Vec<Suite> = serde_json::from_str(&suites_json).ok()?;
    let suite = suites.into_iter().find(|s| s.id == suite_id)?;

    Some(ExecParams {
        project_id:    project_id.to_string(),
        project_name:  name,
        project_path:  path,
        suite_id:      suite.id,
        suite_name:    suite.name,
        suite_tag:     suite.tag,
        suite_command: suite.command,
        suite_cwd:     suite.cwd,
        suite_args:    suite.args,
    })
}

/// Avança a recorrência (ou desativa, se "once") — chamado somente depois
/// que a execução foi de fato disparada.
fn advance_recurrence(db: &rusqlite::Connection, sched_id: &str, recurrence: &str) {
    match recurrence {
        "daily" => {
            // replace() garante separador T mesmo após datetime() retornar com espaço
            let _ = db.execute(
                "UPDATE schedules \
                 SET last_run_at = scheduled_at, \
                     scheduled_at = replace(datetime(scheduled_at, '+1 day'), ' ', 'T') \
                 WHERE id = ?1",
                params![sched_id],
            );
        }
        "weekly" => {
            let _ = db.execute(
                "UPDATE schedules \
                 SET last_run_at = scheduled_at, \
                     scheduled_at = replace(datetime(scheduled_at, '+7 days'), ' ', 'T') \
                 WHERE id = ?1",
                params![sched_id],
            );
        }
        _ => {
            // "once" — desativa após executar
            let _ = db.execute(
                "UPDATE schedules \
                 SET last_run_at = scheduled_at, enabled = 0 \
                 WHERE id = ?1",
                params![sched_id],
            );
        }
    }
}

pub(crate) fn check_and_fire(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();

    // Coleta agendamentos vencidos (libera o lock antes de processar)
    let due = {
        let db = state.db.lock().unwrap();
        get_due_schedules(&db).unwrap_or_default()
    };

    for (sched_id, project_id, suite_id, label, recurrence) in due {
        // Resolve projeto/suite persistidos no banco
        let exec_params = {
            let db = state.db.lock().unwrap();
            resolve_exec_params(&db, &project_id, &suite_id)
        };

        let Some(exec_params) = exec_params else {
            // Projeto ou suite não existe mais — desativa para não retentar a cada ciclo
            {
                let db = state.db.lock().unwrap();
                let _ = db.execute(
                    "UPDATE schedules SET enabled = 0, last_run_at = scheduled_at WHERE id = ?1",
                    params![&sched_id],
                );
            }
            let _ = app.emit("schedules-updated", ());
            continue;
        };

        // Avisa o frontend ANTES de executar, para a UI (se aberta) anexar a aba
        // e o terminal. A execução em si não depende deste evento.
        let _ = app.emit(
            "schedule-triggered",
            ScheduleTriggeredEvent {
                schedule_id: sched_id.clone(),
                project_id,
                suite_id,
                label,
            },
        );

        // Executa direto no backend — funciona mesmo sem janela aberta
        crate::execute_suite(app, exec_params);

        // Recorrência só avança depois do disparo real
        {
            let db = state.db.lock().unwrap();
            advance_recurrence(&db, &sched_id, &recurrence);
        }

        // Notifica o frontend para recarregar a lista de schedules
        let _ = app.emit("schedules-updated", ());
    }
}

pub fn start_scheduler_loop(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            check_and_fire(&app);
        }
    });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::init_db(&conn);
        conn
    }

    /// Insere um schedule com scheduled_at relativo ao agora local
    /// (offset no formato do SQLite, ex.: "-1 hour", "+2 days").
    fn insert_schedule(conn: &Connection, id: &str, offset: &str, recurrence: &str, enabled: bool) {
        conn.execute(
            "INSERT INTO schedules (id, project_id, suite_id, label, scheduled_at, recurrence, enabled)
             VALUES (?1, 'p1', 's1', 'teste', replace(datetime('now', 'localtime', ?2), ' ', 'T'), ?3, ?4)",
            params![id, offset, recurrence, enabled as i64],
        ).unwrap();
    }

    fn scheduled_at(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT scheduled_at FROM schedules WHERE id = ?1", params![id], |r| r.get(0)).unwrap()
    }

    // ── get_due_schedules ─────────────────────────────────────────────────

    #[test]
    fn due_when_past_and_never_ran() {
        let db = test_db();
        insert_schedule(&db, "a", "-1 hour", "once", true);
        let due = get_due_schedules(&db).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, "a");
    }

    #[test]
    fn not_due_when_in_the_future() {
        let db = test_db();
        insert_schedule(&db, "a", "+1 hour", "once", true);
        assert!(get_due_schedules(&db).unwrap().is_empty());
    }

    #[test]
    fn not_due_when_disabled() {
        let db = test_db();
        insert_schedule(&db, "a", "-1 hour", "once", false);
        assert!(get_due_schedules(&db).unwrap().is_empty());
    }

    #[test]
    fn not_due_when_already_ran() {
        let db = test_db();
        insert_schedule(&db, "a", "-1 hour", "daily", true);
        db.execute("UPDATE schedules SET last_run_at = scheduled_at WHERE id = 'a'", []).unwrap();
        assert!(get_due_schedules(&db).unwrap().is_empty());
    }

    // ── advance_recurrence ────────────────────────────────────────────────

    #[test]
    fn daily_advances_one_day_keeping_t_separator() {
        let db = test_db();
        db.execute(
            "INSERT INTO schedules (id, project_id, suite_id, label, scheduled_at, recurrence, enabled)
             VALUES ('a', 'p1', 's1', 'teste', '2026-07-01T08:30:00', 'daily', 1)",
            [],
        ).unwrap();

        advance_recurrence(&db, "a", "daily");

        assert_eq!(scheduled_at(&db, "a"), "2026-07-02T08:30:00");
        let last: String = db.query_row("SELECT last_run_at FROM schedules WHERE id='a'", [], |r| r.get(0)).unwrap();
        assert_eq!(last, "2026-07-01T08:30:00");
    }

    #[test]
    fn weekly_advances_seven_days() {
        let db = test_db();
        db.execute(
            "INSERT INTO schedules (id, project_id, suite_id, label, scheduled_at, recurrence, enabled)
             VALUES ('a', 'p1', 's1', 'teste', '2026-07-01T08:30:00', 'weekly', 1)",
            [],
        ).unwrap();

        advance_recurrence(&db, "a", "weekly");

        assert_eq!(scheduled_at(&db, "a"), "2026-07-08T08:30:00");
    }

    #[test]
    fn once_disables_after_running() {
        let db = test_db();
        insert_schedule(&db, "a", "-1 hour", "once", true);

        advance_recurrence(&db, "a", "once");

        let enabled: i64 = db.query_row("SELECT enabled FROM schedules WHERE id='a'", [], |r| r.get(0)).unwrap();
        assert_eq!(enabled, 0);
        // Não volta a aparecer como vencido
        assert!(get_due_schedules(&db).unwrap().is_empty());
    }

    #[test]
    fn advanced_daily_schedule_is_not_due_again_today() {
        let db = test_db();
        insert_schedule(&db, "a", "-1 minute", "daily", true);
        assert_eq!(get_due_schedules(&db).unwrap().len(), 1);

        advance_recurrence(&db, "a", "daily");

        // Avançou para amanhã — não dispara de novo neste ciclo
        assert!(get_due_schedules(&db).unwrap().is_empty());
    }

    // ── resolve_exec_params ───────────────────────────────────────────────

    fn insert_project(conn: &Connection) {
        let suites = r#"[{
            "id": "s1", "name": "Unit Tests", "system": "web", "tag": "Unit",
            "command": "vitest", "args": ["run", "--coverage"], "cwd": "web"
        }]"#;
        conn.execute(
            "INSERT INTO projects (id, name, path, suites) VALUES ('p1', 'Meu Projeto', 'C:/proj', ?1)",
            params![suites],
        ).unwrap();
    }

    #[test]
    fn resolve_builds_exec_params_from_db() {
        let db = test_db();
        insert_project(&db);

        let p = resolve_exec_params(&db, "p1", "s1").expect("deveria resolver");
        assert_eq!(p.project_name, "Meu Projeto");
        assert_eq!(p.project_path, "C:/proj");
        assert_eq!(p.suite_name, "Unit Tests");
        assert_eq!(p.suite_command, "vitest");
        assert_eq!(p.suite_args, vec!["run".to_string(), "--coverage".to_string()]);
        assert_eq!(p.suite_cwd, "web");
    }

    #[test]
    fn resolve_none_when_project_missing() {
        let db = test_db();
        assert!(resolve_exec_params(&db, "inexistente", "s1").is_none());
    }

    #[test]
    fn resolve_none_when_suite_missing() {
        let db = test_db();
        insert_project(&db);
        assert!(resolve_exec_params(&db, "p1", "suite-que-nao-existe").is_none());
    }
}
