use std::time::Duration;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use crate::AppState;

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

fn check_and_fire(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();

    // Coleta agendamentos vencidos (libera o lock antes de processar)
    let due = {
        let db = state.db.lock().unwrap();
        get_due_schedules(&db).unwrap_or_default()
    };

    for (sched_id, project_id, suite_id, label, recurrence) in due {
        // Atualiza last_run_at e avança data conforme recorrência
        {
            let db = state.db.lock().unwrap();
            match recurrence.as_str() {
                "daily" => {
                    // replace() garante separador T mesmo após datetime() retornar com espaço
                    let _ = db.execute(
                        "UPDATE schedules \
                         SET last_run_at = scheduled_at, \
                             scheduled_at = replace(datetime(scheduled_at, '+1 day'), ' ', 'T') \
                         WHERE id = ?1",
                        params![&sched_id],
                    );
                }
                "weekly" => {
                    let _ = db.execute(
                        "UPDATE schedules \
                         SET last_run_at = scheduled_at, \
                             scheduled_at = replace(datetime(scheduled_at, '+7 days'), ' ', 'T') \
                         WHERE id = ?1",
                        params![&sched_id],
                    );
                }
                _ => {
                    // "once" — desativa após executar
                    let _ = db.execute(
                        "UPDATE schedules \
                         SET last_run_at = scheduled_at, enabled = 0 \
                         WHERE id = ?1",
                        params![&sched_id],
                    );
                }
            }
        }

        // Dispara evento para o frontend executar o startRun
        let _ = app.emit(
            "schedule-triggered",
            ScheduleTriggeredEvent {
                schedule_id: sched_id,
                project_id,
                suite_id,
                label,
            },
        );

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
