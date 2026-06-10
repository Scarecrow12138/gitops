use crate::db::{get_connected_pool, DbState};
use crate::http_retry::{
    gitlab_retry_delay_seconds, retry_after_seconds, should_retry_gitlab_status,
    MAX_GITLAB_RETRIES,
};
use reqwest::{Client, Response};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::ipc::Channel;

const OPERATOR_ID: &str = "1800513641113452545";
const WAITING_RELEASE_URL: &str =
    "https://lasenyun.com/admin-api/oa/project-manage/release/all-waiting-release";
const COMPLETE_RELEASE_URL: &str =
    "https://lasenyun.com/admin-api/oa/project-manage/release/complete-by-operator";
const PREP_BRANCH: &str = "prep-3.0";
const MAIN_BRANCH: &str = "main";
const SNAPSHOT_JOB: &str = "lasen-prod-snapshot-release";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "event", content = "data")]
pub enum ReleaseProgressEvent {
    Step {
        step_index: usize,
        status: String,
        message: String,
    },
    Log {
        level: String,
        message: String,
    },
    Retry {
        step_index: usize,
        api: String,
        retry_no: u32,
        delay_seconds: u64,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRecord {
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub id: Option<i64>,
    pub rel_no: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub developer_id: Option<i64>,
    pub remark: Option<String>,
    pub service: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskIdentity {
    pub basis: String,
    pub task_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTaskSnapshot {
    pub record_ids: Vec<i64>,
    pub rel_nos: Vec<String>,
    pub services: Vec<String>,
    pub raw_records: Vec<ReleaseRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTaskDto {
    pub id: String,
    pub task_key: String,
    pub status: String,
    pub record_ids: Vec<i64>,
    pub rel_nos: Vec<String>,
    pub services: Vec<String>,
    pub attempts_count: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAttemptDto {
    pub id: i64,
    pub task_id: String,
    pub attempt_no: i64,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub failed_step: Option<String>,
    pub log_output: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTaskPayload {
    pub task: Option<ReleaseTaskDto>,
    pub attempts: Vec<ReleaseAttemptDto>,
    pub logs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAttemptPayload {
    pub task: ReleaseTaskDto,
    pub attempts: Vec<ReleaseAttemptDto>,
    pub logs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WaitingReleaseResponse {
    code: i64,
    msg: Option<String>,
    #[serde(default)]
    data: Vec<ReleaseRecord>,
}

#[derive(Debug, FromRow)]
struct ReleaseTaskRow {
    id: String,
    task_key: String,
    status: String,
    record_ids: String,
    rel_nos: String,
    service_snapshot: String,
    raw_snapshot: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct ReleaseAttemptRow {
    id: i64,
    task_id: String,
    attempt_no: i64,
    status: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    failed_step: Option<String>,
    log_output: Option<String>,
}

#[derive(Debug, FromRow)]
struct RuntimeReleaseConfig {
    jenkins_url: Option<String>,
    jenkins_username: Option<String>,
    jenkins_token: Option<String>,
    commit_limit: i32,
}

#[derive(Debug, FromRow)]
struct RuntimeGitlabConfig {
    url: String,
    token: Option<String>,
}

#[derive(Debug, Clone)]
struct MergeRequest {
    project_id: String,
    iid: i64,
    web_url: String,
}

#[derive(Debug, Deserialize)]
struct GitlabMergeRequest {
    iid: i64,
    project_id: Option<i64>,
    web_url: Option<String>,
}

#[derive(Debug, Clone)]
struct JenkinsPendingBuild {
    job_name: String,
    previous_number: Option<i64>,
}

#[derive(Debug, Clone)]
struct JenkinsBuildResult {
    job_name: String,
    build_number: i64,
    result: String,
}

#[tauri::command]
pub async fn refresh_release_task(
    state: tauri::State<'_, DbState>,
) -> Result<ReleaseTaskPayload, String> {
    let pool = get_connected_pool(state.inner())?;
    let records = fetch_waiting_release_records().await?;
    if records.is_empty() {
        return Ok(ReleaseTaskPayload {
            task: None,
            attempts: vec![],
            logs: vec!["暂无待发布任务".to_string()],
        });
    }

    let row = upsert_release_task(&pool, &records).await?;
    let attempts = load_attempts(&pool, &row.id).await?;
    let task = row_to_task_dto(&pool, row).await?;

    Ok(ReleaseTaskPayload {
        task: Some(task),
        attempts,
        logs: vec!["发布任务已刷新".to_string()],
    })
}

#[tauri::command]
pub async fn start_release_attempt(
    state: tauri::State<'_, DbState>,
    task_id: String,
    on_event: Channel<ReleaseProgressEvent>,
) -> Result<ReleaseAttemptPayload, String> {
    let pool = get_connected_pool(state.inner())?;
    let task = load_task_row(&pool, &task_id).await?;
    let snapshot = row_to_snapshot(&task)?;
    let attempt_no = next_attempt_no(&pool, &task_id).await?;
    let attempt_id = insert_attempt(&pool, &task_id, attempt_no).await?;

    sqlx::query("UPDATE release_tasks SET status='running', updated_at=CURRENT_TIMESTAMP WHERE id=?1")
        .bind(&task_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut logs = vec![];
    push_progress_log(
        &mut logs,
        &on_event,
        "info",
        format!("开始第 {} 次发布执行", attempt_no),
    );
    let result = run_release_steps(&pool, &snapshot, &mut logs, &on_event).await;

    match result {
        Ok(()) => {
            finish_attempt(&pool, attempt_id, "success", None, &logs).await?;
            sqlx::query("UPDATE release_tasks SET status='success', updated_at=CURRENT_TIMESTAMP WHERE id=?1")
                .bind(&task_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        Err(err) => {
            logs.push(format!("发布失败: {}", err));
            finish_attempt(&pool, attempt_id, "failed", Some(&err), &logs).await?;
            sqlx::query("UPDATE release_tasks SET status='failed', updated_at=CURRENT_TIMESTAMP WHERE id=?1")
                .bind(&task_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    let task = row_to_task_dto(&pool, load_task_row(&pool, &task_id).await?).await?;
    let attempts = load_attempts(&pool, &task_id).await?;
    Ok(ReleaseAttemptPayload {
        task,
        attempts,
        logs,
    })
}

#[tauri::command]
pub async fn complete_release_task(
    state: tauri::State<'_, DbState>,
    task_id: String,
) -> Result<ReleaseTaskPayload, String> {
    let pool = get_connected_pool(state.inner())?;
    call_complete_release().await?;
    sqlx::query("UPDATE release_tasks SET status='success', updated_at=CURRENT_TIMESTAMP WHERE id=?1")
        .bind(&task_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let task = row_to_task_dto(&pool, load_task_row(&pool, &task_id).await?).await?;
    let attempts = load_attempts(&pool, &task_id).await?;
    Ok(ReleaseTaskPayload {
        task: Some(task),
        attempts,
        logs: vec!["发布任务已标记完成".to_string()],
    })
}

pub fn build_task_identity(records: &[ReleaseRecord]) -> Result<TaskIdentity, String> {
    if records.is_empty() {
        return Err("待发布记录为空，无法生成发布任务标识".to_string());
    }

    let mut ids = records
        .iter()
        .filter_map(|record| record.id)
        .collect::<Vec<_>>();
    if ids.len() == records.len() {
        ids.sort_unstable();
        let basis = format!(
            "release_ids:{}",
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        return Ok(TaskIdentity {
            task_key: stable_hash(&basis),
            basis,
        });
    }

    let mut rel_nos = records
        .iter()
        .filter_map(|record| normalize_text(record.rel_no.as_deref()))
        .collect::<Vec<_>>();
    if rel_nos.len() == records.len() {
        rel_nos.sort();
        let basis = format!("release_rel_nos:{}", rel_nos.join(","));
        return Ok(TaskIdentity {
            task_key: stable_hash(&basis),
            basis,
        });
    }

    let snapshot = build_task_snapshot(records)?;
    let basis = format!("release_content:services:{}", snapshot.services.join(","));
    Ok(TaskIdentity {
        task_key: stable_hash(&basis),
        basis,
    })
}

pub fn build_task_snapshot(records: &[ReleaseRecord]) -> Result<ReleaseTaskSnapshot, String> {
    let mut record_ids = records.iter().filter_map(|record| record.id).collect::<Vec<_>>();
    record_ids.sort_unstable();

    let mut rel_nos = records
        .iter()
        .filter_map(|record| normalize_text(record.rel_no.as_deref()))
        .collect::<Vec<_>>();
    rel_nos.sort();

    let mut service_set = BTreeSet::new();
    for record in records {
        if let Some(service) = &record.service {
            for item in service.split(',') {
                if let Some(normalized) = normalize_service(item) {
                    service_set.insert(normalized);
                }
            }
        }
    }

    Ok(ReleaseTaskSnapshot {
        record_ids,
        rel_nos,
        services: service_set.into_iter().collect(),
        raw_records: records.to_vec(),
    })
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_service(value: &str) -> Option<String> {
    let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let mapped = match normalized.as_str() {
        "basedata" => "base-data",
        "ecjob" => "ec-job",
        other => other,
    };
    Some(format!("lasen-prod-{}", mapped))
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .ok_or_else(|| DeError::custom("number is not a valid i64"))
            .map(Some),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<i64>()
                    .map(Some)
                    .map_err(|e| DeError::custom(e.to_string()))
            }
        }
        _ => Err(DeError::custom("expected i64, string, null, or missing value")),
    }
}

fn parse_waiting_release_records(payload: &str) -> Result<Vec<ReleaseRecord>, String> {
    let payload = serde_json::from_str::<WaitingReleaseResponse>(payload)
        .map_err(|e| format!("解析待发布任务失败: {}", e))?;

    if payload.code != 0 {
        return Err(format!(
            "获取待发布任务失败: {}",
            payload.msg.unwrap_or_else(|| payload.code.to_string())
        ));
    }

    Ok(payload.data)
}

async fn fetch_waiting_release_records() -> Result<Vec<ReleaseRecord>, String> {
    let response = Client::new()
        .get(WAITING_RELEASE_URL)
        .query(&[("operatorId", OPERATOR_ID)])
        .send()
        .await
        .map_err(|e| format!("获取待发布任务失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("获取待发布任务失败({}): {}", status, body));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("读取待发布任务响应失败: {}", e))?;

    parse_waiting_release_records(&body)
}

async fn upsert_release_task(
    pool: &SqlitePool,
    records: &[ReleaseRecord],
) -> Result<ReleaseTaskRow, String> {
    let identity = build_task_identity(records)?;
    let snapshot = build_task_snapshot(records)?;

    let record_ids = serde_json::to_string(&snapshot.record_ids).map_err(|e| e.to_string())?;
    let rel_nos = serde_json::to_string(&snapshot.rel_nos).map_err(|e| e.to_string())?;
    let services = serde_json::to_string(&snapshot.services).map_err(|e| e.to_string())?;
    let raw_records = serde_json::to_string(&snapshot.raw_records).map_err(|e| e.to_string())?;

    if let Some(row) = sqlx::query_as::<_, ReleaseTaskRow>(
        "SELECT id, task_key, status, record_ids, rel_nos, service_snapshot, raw_snapshot, created_at, updated_at
         FROM release_tasks
         WHERE task_key=?1 AND status IN ('pending','running','failed')
         ORDER BY updated_at DESC
         LIMIT 1",
    )
    .bind(&identity.task_key)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        sqlx::query(
            "UPDATE release_tasks
             SET record_ids=?1, rel_nos=?2, service_snapshot=?3, raw_snapshot=?4, updated_at=CURRENT_TIMESTAMP
             WHERE id=?5",
        )
        .bind(record_ids)
        .bind(rel_nos)
        .bind(services)
        .bind(raw_records)
        .bind(&row.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return load_task_row(pool, &row.id).await;
    }

    let id = format!("release-{}", now_millis());
    sqlx::query(
        "INSERT INTO release_tasks (id, task_key, status, record_ids, rel_nos, service_snapshot, raw_snapshot)
         VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(&identity.task_key)
    .bind(record_ids)
    .bind(rel_nos)
    .bind(services)
    .bind(raw_records)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    load_task_row(pool, &id).await
}

async fn load_task_row(pool: &SqlitePool, task_id: &str) -> Result<ReleaseTaskRow, String> {
    sqlx::query_as::<_, ReleaseTaskRow>(
        "SELECT id, task_key, status, record_ids, rel_nos, service_snapshot, raw_snapshot, created_at, updated_at
         FROM release_tasks WHERE id=?1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "发布任务不存在".to_string())
}

async fn row_to_task_dto(pool: &SqlitePool, row: ReleaseTaskRow) -> Result<ReleaseTaskDto, String> {
    let attempts_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM release_attempts WHERE task_id=?1")
            .bind(&row.id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    let snapshot = row_to_snapshot(&row)?;
    Ok(ReleaseTaskDto {
        id: row.id,
        task_key: row.task_key,
        status: row.status,
        record_ids: snapshot.record_ids,
        rel_nos: snapshot.rel_nos,
        services: snapshot.services,
        attempts_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn row_to_snapshot(row: &ReleaseTaskRow) -> Result<ReleaseTaskSnapshot, String> {
    Ok(ReleaseTaskSnapshot {
        record_ids: serde_json::from_str(&row.record_ids).map_err(|e| e.to_string())?,
        rel_nos: serde_json::from_str(&row.rel_nos).map_err(|e| e.to_string())?,
        services: serde_json::from_str(&row.service_snapshot).map_err(|e| e.to_string())?,
        raw_records: serde_json::from_str(&row.raw_snapshot).map_err(|e| e.to_string())?,
    })
}

async fn load_attempts(pool: &SqlitePool, task_id: &str) -> Result<Vec<ReleaseAttemptDto>, String> {
    let rows = sqlx::query_as::<_, ReleaseAttemptRow>(
        "SELECT id, task_id, attempt_no, status, started_at, finished_at, failed_step, log_output
         FROM release_attempts WHERE task_id=?1 ORDER BY attempt_no DESC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(row_to_attempt_dto).collect())
}

fn row_to_attempt_dto(row: ReleaseAttemptRow) -> ReleaseAttemptDto {
    ReleaseAttemptDto {
        id: row.id,
        task_id: row.task_id,
        attempt_no: row.attempt_no,
        status: row.status,
        started_at: row.started_at,
        finished_at: row.finished_at,
        failed_step: row.failed_step,
        log_output: row.log_output,
    }
}

async fn next_attempt_no(pool: &SqlitePool, task_id: &str) -> Result<i64, String> {
    let max_attempt: Option<i64> =
        sqlx::query_scalar("SELECT MAX(attempt_no) FROM release_attempts WHERE task_id=?1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(max_attempt.unwrap_or(0) + 1)
}

async fn insert_attempt(pool: &SqlitePool, task_id: &str, attempt_no: i64) -> Result<i64, String> {
    let result = sqlx::query(
        "INSERT INTO release_attempts (task_id, attempt_no, status, started_at)
         VALUES (?1, ?2, 'running', CURRENT_TIMESTAMP)",
    )
    .bind(task_id)
    .bind(attempt_no)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.last_insert_rowid())
}

async fn finish_attempt(
    pool: &SqlitePool,
    attempt_id: i64,
    status: &str,
    failed_step: Option<&str>,
    logs: &[String],
) -> Result<(), String> {
    sqlx::query(
        "UPDATE release_attempts
         SET status=?1, finished_at=CURRENT_TIMESTAMP, failed_step=?2, log_output=?3
         WHERE id=?4",
    )
    .bind(status)
    .bind(failed_step)
    .bind(logs.join("\n"))
    .bind(attempt_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn emit_progress(on_event: &Channel<ReleaseProgressEvent>, event: ReleaseProgressEvent) {
    let _ = on_event.send(event);
}

fn push_progress_log(
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    level: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    logs.push(message.clone());
    emit_progress(
        on_event,
        ReleaseProgressEvent::Log {
            level: level.to_string(),
            message,
        },
    );
}

fn set_progress_step(
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
    status: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    logs.push(message.clone());
    emit_progress(
        on_event,
        ReleaseProgressEvent::Step {
            step_index,
            status: status.to_string(),
            message,
        },
    );
}

fn emit_retry_progress(
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
    api: &str,
    retry_no: u32,
    delay_seconds: u64,
    reason: &str,
) {
    let message = format!(
        "{} 调用失败，{} 秒后第 {} 次重试：{}",
        api, delay_seconds, retry_no, reason
    );
    logs.push(message.clone());
    emit_progress(
        on_event,
        ReleaseProgressEvent::Retry {
            step_index,
            api: api.to_string(),
            retry_no,
            delay_seconds,
            reason: reason.to_string(),
        },
    );
}

async fn run_release_steps(
    pool: &SqlitePool,
    snapshot: &ReleaseTaskSnapshot,
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
) -> Result<(), String> {
    let client = Client::new();
    let gitlab = load_gitlab_config(pool).await?;
    let release_config = load_release_config(pool).await?;

    set_progress_step(logs, on_event, 0, "running", "1. 获取并校验合并到 prep-3.0 的 MR");
    let prep_mrs = match get_merge_to_prep_requests(&client, &gitlab, logs, on_event, 0).await {
        Ok(value) => value,
        Err(error) => {
            set_progress_step(logs, on_event, 0, "failed", format!("1. 获取并校验合并到 prep-3.0 的 MR失败: {}", error));
            return Err(error);
        }
    };
    if let Err(error) =
        check_commit_num(&client, &gitlab, &prep_mrs, release_config.commit_limit, logs, on_event, 0).await
    {
        set_progress_step(logs, on_event, 0, "failed", format!("1. 获取并校验合并到 prep-3.0 的 MR失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 0, "success", "1. 获取并校验合并到 prep-3.0 的 MR完成");

    set_progress_step(logs, on_event, 1, "running", "2. 合并源分支到 prep-3.0");
    if let Err(error) = merge_requests(&client, &gitlab, &prep_mrs, logs, on_event, 1).await {
        set_progress_step(logs, on_event, 1, "failed", format!("2. 合并源分支到 prep-3.0失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 1, "success", "2. 合并源分支到 prep-3.0完成");

    set_progress_step(logs, on_event, 2, "running", "3. 创建 prep-3.0 到 main 的 MR");
    let main_mrs = match create_merge_requests_to_main(&client, &gitlab, logs, on_event, 2).await {
        Ok(value) => value,
        Err(error) => {
            set_progress_step(logs, on_event, 2, "failed", format!("3. 创建 prep-3.0 到 main 的 MR失败: {}", error));
            return Err(error);
        }
    };
    if let Err(error) =
        check_commit_num(&client, &gitlab, &main_mrs, release_config.commit_limit, logs, on_event, 2).await
    {
        set_progress_step(logs, on_event, 2, "failed", format!("3. 创建 prep-3.0 到 main 的 MR失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 2, "success", "3. 创建 prep-3.0 到 main 的 MR完成");

    set_progress_step(logs, on_event, 3, "running", "4. 备份 main 分支并合并主干");
    if let Err(error) = backup_main_branches(&client, &gitlab, &main_mrs, logs, on_event, 3).await {
        set_progress_step(logs, on_event, 3, "failed", format!("4. 备份 main 分支并合并主干失败: {}", error));
        return Err(error);
    }
    let main_mrs = match close_empty_merge_requests(&client, &gitlab, &main_mrs, logs, on_event, 3).await {
        Ok(value) => value,
        Err(error) => {
            set_progress_step(logs, on_event, 3, "failed", format!("4. 备份 main 分支并合并主干失败: {}", error));
            return Err(error);
        }
    };
    if let Err(error) = merge_requests(&client, &gitlab, &main_mrs, logs, on_event, 3).await {
        set_progress_step(logs, on_event, 3, "failed", format!("4. 备份 main 分支并合并主干失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 3, "success", "4. 备份 main 分支并合并主干完成");

    set_progress_step(logs, on_event, 4, "running", "5. 触发 Jenkins 构建");
    let service_builds = match trigger_jenkins_builds(&client, &release_config, &snapshot.services, logs, on_event).await {
        Ok(value) => value,
        Err(error) => {
            set_progress_step(logs, on_event, 4, "failed", format!("5. 触发 Jenkins 构建失败: {}", error));
            return Err(error);
        }
    };
    set_progress_step(logs, on_event, 4, "success", "5. 触发 Jenkins 构建完成");

    set_progress_step(logs, on_event, 5, "running", "6. 检查 Jenkins 构建结果");
    if let Err(error) = check_jenkins_build_results(&client, &release_config, service_builds, logs, on_event).await {
        set_progress_step(logs, on_event, 5, "failed", format!("6. 检查 Jenkins 构建结果失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 5, "success", "6. 检查 Jenkins 构建结果完成");

    set_progress_step(logs, on_event, 6, "running", "7. 完成发布回调");
    if let Err(error) = call_complete_release().await {
        set_progress_step(logs, on_event, 6, "failed", format!("7. 完成发布回调失败: {}", error));
        return Err(error);
    }
    set_progress_step(logs, on_event, 6, "success", "7. 完成发布回调完成");
    push_progress_log(logs, on_event, "success", "发布执行成功");
    Ok(())
}

async fn load_gitlab_config(pool: &SqlitePool) -> Result<(RuntimeGitlabConfig, Vec<String>), String> {
    let config = sqlx::query_as::<_, RuntimeGitlabConfig>(
        "SELECT url, token FROM gitlab_configs WHERE id='default'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "请先配置 GitLab 地址和 Token".to_string())?;

    if config.token.as_deref().unwrap_or_default().trim().is_empty() {
        return Err("请先在配置管理中填写 GitLab Access Token".to_string());
    }

    let projects = sqlx::query_scalar::<_, String>(
        "SELECT project_gitlab_id FROM project_mappings WHERE gitlab_config_id='default' ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if projects.is_empty() {
        return Err("请先在配置管理中维护 GitLab 项目映射".to_string());
    }

    Ok((config, projects))
}

async fn load_release_config(pool: &SqlitePool) -> Result<RuntimeReleaseConfig, String> {
    let config = sqlx::query_as::<_, RuntimeReleaseConfig>(
        "SELECT jenkins_url, jenkins_username, jenkins_token, commit_limit
         FROM release_configs WHERE id='default'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "请先配置发布管理参数".to_string())?;

    validate_release_config(&config)?;
    Ok(config)
}

fn validate_release_config(config: &RuntimeReleaseConfig) -> Result<(), String> {
    // 发布状态只依赖 Jenkins，这里不再校验外部服务注册配置。
    if config.jenkins_url.as_deref().unwrap_or_default().trim().is_empty()
        || config
            .jenkins_username
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        || config
            .jenkins_token
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err("请先在配置管理中填写 Jenkins 地址、用户名和 Token".to_string());
    }
    Ok(())
}

async fn send_gitlab_request<F>(
    api: &str,
    step_index: usize,
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    require_success: bool,
    build_request: F,
) -> Result<Response, String>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    for attempt in 0..=MAX_GITLAB_RETRIES {
        match build_request().send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Ok(response);
                }

                if should_retry_gitlab_status(status) && attempt < MAX_GITLAB_RETRIES {
                    let retry_no = attempt + 1;
                    let delay_seconds = retry_after_seconds(response.headers(), retry_no);
                    let body = response.text().await.unwrap_or_default();
                    let reason = if body.trim().is_empty() {
                        status.to_string()
                    } else {
                        format!("{} {}", status, body.trim())
                    };
                    emit_retry_progress(
                        logs,
                        on_event,
                        step_index,
                        api,
                        retry_no,
                        delay_seconds,
                        &reason,
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
                    continue;
                }

                if require_success {
                    let body = response.text().await.unwrap_or_default();
                    return Err(format!("{} 失败({}): {}", api, status, body));
                }
                return Ok(response);
            }
            Err(error) => {
                if attempt < MAX_GITLAB_RETRIES {
                    let retry_no = attempt + 1;
                    let delay_seconds = gitlab_retry_delay_seconds(retry_no);
                    let reason = error.to_string();
                    emit_retry_progress(
                        logs,
                        on_event,
                        step_index,
                        api,
                        retry_no,
                        delay_seconds,
                        &reason,
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
                    continue;
                }
                return Err(format!("{} 失败: {}", api, error));
            }
        }
    }

    Err(format!("{} 失败: 重试次数已耗尽", api))
}

async fn get_merge_to_prep_requests(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<Vec<MergeRequest>, String> {
    let mut result = vec![];
    for project_id in &gitlab.1 {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests",
            gitlab.0.url.trim_end_matches('/'),
            project_id
        );
        let api = format!("获取 GitLab MR(project={})", project_id);
        let response = send_gitlab_request(
            &api,
            step_index,
            logs,
            on_event,
            true,
            || {
                client
                    .get(&url)
                    .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                    .query(&[("state", "opened"), ("target_branch", PREP_BRANCH)])
            },
        )
        .await?;
        let items = response
            .json::<Vec<GitlabMergeRequest>>()
            .await
            .map_err(|e| format!("解析 GitLab MR 失败: {}", e))?;
        result.extend(items.into_iter().map(|item| MergeRequest {
            project_id: item
                .project_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| project_id.clone()),
            iid: item.iid,
            web_url: item.web_url.unwrap_or_default(),
        }));
    }
    Ok(result)
}

async fn create_merge_requests_to_main(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<Vec<MergeRequest>, String> {
    for project_id in &gitlab.1 {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests",
            gitlab.0.url.trim_end_matches('/'),
            project_id
        );
        let params = [
            ("source_branch", PREP_BRANCH),
            ("target_branch", MAIN_BRANCH),
            ("title", "合并"),
        ];
        let api = format!("创建主干 MR(project={})", project_id);
        let _ = send_gitlab_request(&api, step_index, logs, on_event, false, || {
            client
                .post(&url)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .form(&params)
        })
        .await?;
    }

    let mut result = vec![];
    for project_id in &gitlab.1 {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests",
            gitlab.0.url.trim_end_matches('/'),
            project_id
        );
        let query = [
            ("state", "opened"),
            ("source_branch", PREP_BRANCH),
            ("target_branch", MAIN_BRANCH),
        ];
        let api = format!("查询主干 MR(project={})", project_id);
        let response = send_gitlab_request(&api, step_index, logs, on_event, true, || {
            client
                .get(&url)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .query(&query)
        })
        .await?;
        let items = response
            .json::<Vec<GitlabMergeRequest>>()
            .await
            .map_err(|e| format!("解析主干 MR 失败: {}", e))?;
        result.extend(items.into_iter().map(|item| MergeRequest {
            project_id: item
                .project_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| project_id.clone()),
            iid: item.iid,
            web_url: item.web_url.unwrap_or_default(),
        }));
    }

    if result.len() != gitlab.1.len() {
        return Err(format!(
            "创建 prep-3.0 到 main 的 MR 数量不一致，应为 {}，实际 {}",
            gitlab.1.len(),
            result.len()
        ));
    }
    Ok(result)
}

async fn check_commit_num(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    requests: &[MergeRequest],
    limit: i32,
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<(), String> {
    let mut errors = vec![];
    for request in requests {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests/{}/commits",
            gitlab.0.url.trim_end_matches('/'),
            request.project_id,
            request.iid
        );
        let api = format!("获取 MR 提交数(project={}, iid={})", request.project_id, request.iid);
        let response = send_gitlab_request(&api, step_index, logs, on_event, true, || {
            client
                .get(&url)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .query(&[("per_page", "100")])
        })
        .await?;
        let commits = response
            .json::<Vec<Value>>()
            .await
            .map_err(|e| format!("解析 MR 提交数失败: {}", e))?;
        if commits.len() as i32 > limit {
            errors.push(format!(
                "{} 提交数为 {}，超过阈值 {}",
                request.web_url,
                commits.len(),
                limit
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

async fn merge_requests(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    requests: &[MergeRequest],
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<(), String> {
    let error_messages = [
        "401 Unauthorized",
        "405 Method Not Allowed",
        "SHA does not match HEAD of source branch",
        "Branch cannot be merged",
        "DIY_ERROR",
    ];
    let mut errors = vec![];
    for request in requests {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests/{}/merge",
            gitlab.0.url.trim_end_matches('/'),
            request.project_id,
            request.iid
        );
        let api = format!("合并 MR(project={}, iid={})", request.project_id, request.iid);
        let response = send_gitlab_request(&api, step_index, logs, on_event, false, || {
            client
                .put(&url)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
        })
        .await?;
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| value.get("message").cloned())
            .and_then(|value| match value {
                Value::String(text) => Some(text),
                other => Some(other.to_string()),
            })
            .unwrap_or_default();
        if error_messages.contains(&message.as_str()) {
            errors.push(format!("{} 合并失败: {}", request.web_url, message));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

async fn backup_main_branches(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    requests: &[MergeRequest],
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<(), String> {
    let project_ids = requests
        .iter()
        .map(|request| request.project_id.clone())
        .collect::<BTreeSet<_>>();
    let now = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    for project_id in project_ids {
        let base = format!(
            "{}/api/v4/projects/{}/repository/branches",
            gitlab.0.url.trim_end_matches('/'),
            project_id
        );
        let branch_name = format!("main-back-{}", now);
        let params = [("branch", branch_name.as_str()), ("ref", MAIN_BRANCH)];
        let api = format!("备份 main 分支(project={})", project_id);
        let _ = send_gitlab_request(&api, step_index, logs, on_event, false, || {
            client
                .post(&base)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .form(&params)
        })
        .await?;

        let api = format!("查询 main 备份分支(project={})", project_id);
        let response = send_gitlab_request(&api, step_index, logs, on_event, true, || {
            client
                .get(&base)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .query(&[("search", "^main-back")])
        })
        .await?;
        let branches = response
            .json::<Vec<Value>>()
            .await
            .map_err(|e| format!("解析 main 备份分支失败: {}", e))?;
        let mut branch_names = branches
            .into_iter()
            .filter_map(|branch| branch.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        branch_names.sort();
        let delete_count = branch_names.len().saturating_sub(5);
        for branch in branch_names.into_iter().take(delete_count) {
            let url = format!("{}/{}", base, branch);
            let api = format!("删除旧 main 备份分支(project={}, branch={})", project_id, branch);
            let _ = send_gitlab_request(&api, step_index, logs, on_event, false, || {
                client
                    .delete(&url)
                    .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
            })
            .await?;
        }
    }
    Ok(())
}

async fn close_empty_merge_requests(
    client: &Client,
    gitlab: &(RuntimeGitlabConfig, Vec<String>),
    requests: &[MergeRequest],
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
    step_index: usize,
) -> Result<Vec<MergeRequest>, String> {
    let mut result = vec![];
    for request in requests {
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests/{}/commits",
            gitlab.0.url.trim_end_matches('/'),
            request.project_id,
            request.iid
        );
        let api = format!("检查空 MR(project={}, iid={})", request.project_id, request.iid);
        let response = send_gitlab_request(&api, step_index, logs, on_event, true, || {
            client
                .get(&url)
                .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                .query(&[("per_page", "100")])
        })
        .await?;
        let commits = response
            .json::<Vec<Value>>()
            .await
            .map_err(|e| format!("解析空 MR 失败: {}", e))?;
        if commits.is_empty() {
            let close_url = format!(
                "{}/api/v4/projects/{}/merge_requests/{}",
                gitlab.0.url.trim_end_matches('/'),
                request.project_id,
                request.iid
            );
            let body = serde_json::json!({ "state_event": "close" });
            let api = format!("关闭空 MR(project={}, iid={})", request.project_id, request.iid);
            let _ = send_gitlab_request(&api, step_index, logs, on_event, false, || {
                client
                    .put(&close_url)
                    .header("PRIVATE-TOKEN", gitlab.0.token.as_deref().unwrap_or_default())
                    .json(&body)
            })
            .await?;
        } else {
            result.push(request.clone());
        }
    }
    Ok(result)
}

async fn trigger_jenkins_job(
    client: &Client,
    config: &RuntimeReleaseConfig,
    job_name: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/job/{}/build",
        config.jenkins_url.as_deref().unwrap_or_default().trim_end_matches('/'),
        job_name
    );
    client
        .post(url)
        .basic_auth(
            config.jenkins_username.as_deref().unwrap_or_default(),
            Some(config.jenkins_token.as_deref().unwrap_or_default()),
        )
        .send()
        .await
        .map_err(|e| format!("触发 Jenkins 任务失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("触发 Jenkins 任务失败: {}", e))?;
    Ok(())
}

async fn get_jenkins_last_build_number(
    client: &Client,
    config: &RuntimeReleaseConfig,
    job_name: &str,
) -> Result<Option<i64>, String> {
    let url = format!(
        "{}/job/{}/lastBuild/api/json",
        config.jenkins_url.as_deref().unwrap_or_default().trim_end_matches('/'),
        job_name
    );
    let response = client
        .get(url)
        .basic_auth(
            config.jenkins_username.as_deref().unwrap_or_default(),
            Some(config.jenkins_token.as_deref().unwrap_or_default()),
        )
        .send()
        .await
        .map_err(|e| format!("查询 Jenkins 最近构建失败: {}", e))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let value = response
        .error_for_status()
        .map_err(|e| format!("查询 Jenkins 最近构建失败: {}", e))?
        .json::<Value>()
        .await
        .map_err(|e| format!("解析 Jenkins 最近构建失败: {}", e))?;
    Ok(value.get("number").and_then(Value::as_i64))
}

async fn wait_jenkins_build_after(
    client: &Client,
    config: &RuntimeReleaseConfig,
    pending: &JenkinsPendingBuild,
) -> Result<JenkinsBuildResult, String> {
    let previous_number = pending.previous_number.unwrap_or(0);
    for _ in 0..180 {
        let url = format!(
            "{}/job/{}/lastBuild/api/json",
            config.jenkins_url.as_deref().unwrap_or_default().trim_end_matches('/'),
            pending.job_name
        );
        let response = client
            .get(url)
            .basic_auth(
                config.jenkins_username.as_deref().unwrap_or_default(),
                Some(config.jenkins_token.as_deref().unwrap_or_default()),
            )
            .send()
            .await
            .map_err(|e| format!("查询 Jenkins 构建结果失败: {}", e))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            continue;
        }
        let value = response
            .error_for_status()
            .map_err(|e| format!("查询 Jenkins 构建结果失败: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("解析 Jenkins 构建结果失败: {}", e))?;
        let number = value.get("number").and_then(Value::as_i64).unwrap_or(0);
        let building = value.get("building").and_then(Value::as_bool).unwrap_or(false);
        let result = value.get("result").and_then(Value::as_str).unwrap_or("");
        if number > previous_number && !building && !result.is_empty() {
            return Ok(JenkinsBuildResult {
                job_name: pending.job_name.clone(),
                build_number: number,
                result: result.to_string(),
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
    Err(format!("等待 Jenkins 任务 {} 完成超时", pending.job_name))
}

async fn trigger_jenkins_builds(
    client: &Client,
    config: &RuntimeReleaseConfig,
    services: &[String],
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
) -> Result<Vec<JenkinsPendingBuild>, String> {
    let snapshot_pending = JenkinsPendingBuild {
        job_name: SNAPSHOT_JOB.to_string(),
        previous_number: get_jenkins_last_build_number(client, config, SNAPSHOT_JOB).await?,
    };
    push_progress_log(logs, on_event, "info", format!("触发 Jenkins 任务: {}", SNAPSHOT_JOB));
    trigger_jenkins_job(client, config, SNAPSHOT_JOB).await?;
    let snapshot_result = wait_jenkins_build_after(client, config, &snapshot_pending).await?;
    push_progress_log(
        logs,
        on_event,
        "info",
        format!(
            "{} #{} 构建结果: {}",
            snapshot_result.job_name, snapshot_result.build_number, snapshot_result.result
        ),
    );
    if !is_acceptable_jenkins_result(&snapshot_result.result) {
        return Err(format!(
            "{} #{}: {}",
            snapshot_result.job_name, snapshot_result.build_number, snapshot_result.result
        ));
    }

    let mut pending_builds = vec![];
    for service in services {
        let pending = JenkinsPendingBuild {
            job_name: service.clone(),
            previous_number: get_jenkins_last_build_number(client, config, service).await?,
        };
        push_progress_log(logs, on_event, "info", format!("触发 Jenkins 任务: {}", service));
        trigger_jenkins_job(client, config, service).await?;
        pending_builds.push(pending);
    }
    Ok(pending_builds)
}

async fn check_jenkins_build_results(
    client: &Client,
    config: &RuntimeReleaseConfig,
    pending_builds: Vec<JenkinsPendingBuild>,
    logs: &mut Vec<String>,
    on_event: &Channel<ReleaseProgressEvent>,
) -> Result<(), String> {
    let mut failures = vec![];
    for pending in pending_builds {
        let build = wait_jenkins_build_after(client, config, &pending).await?;
        push_progress_log(
            logs,
            on_event,
            "info",
            format!("{} #{} 构建结果: {}", build.job_name, build.build_number, build.result),
        );
        if !is_acceptable_jenkins_result(&build.result) {
            failures.push(format!("{} #{}: {}", build.job_name, build.build_number, build.result));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("以下 Jenkins 任务执行失败: {}", failures.join(", ")))
    }
}

fn is_acceptable_jenkins_result(result: &str) -> bool {
    matches!(result, "SUCCESS" | "UNSTABLE")
}

async fn call_complete_release() -> Result<(), String> {
    let response = Client::new()
        .get(COMPLETE_RELEASE_URL)
        .query(&[("operatorId", OPERATOR_ID)])
        .send()
        .await
        .map_err(|e| format!("完成发布回调失败: {}", e))?;
    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!("完成发布回调失败({}): {}", status, body))
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: Option<i64>, rel_no: Option<&str>, service: &str) -> ReleaseRecord {
        ReleaseRecord {
            id,
            rel_no: rel_no.map(str::to_string),
            developer_id: None,
            remark: None,
            service: Some(service.to_string()),
        }
    }

    #[test]
    fn task_identity_prefers_sorted_record_ids() {
        let first = vec![
            record(Some(12), Some("REL-12"), "ec,front"),
            record(Some(3), Some("REL-3"), "gateway"),
        ];
        let second = vec![
            record(Some(3), Some("REL-3"), "gateway"),
            record(Some(12), Some("REL-12"), "front,ec"),
        ];

        let first_identity = build_task_identity(&first).unwrap();
        let second_identity = build_task_identity(&second).unwrap();

        assert_eq!(first_identity.basis, "release_ids:3,12");
        assert_eq!(first_identity.task_key, second_identity.task_key);
    }

    #[test]
    fn task_identity_falls_back_to_rel_no_when_ids_are_missing() {
        let records = vec![
            record(None, Some("REL-2"), "ec"),
            record(None, Some("REL-1"), "front"),
        ];

        let identity = build_task_identity(&records).unwrap();

        assert_eq!(identity.basis, "release_rel_nos:REL-1,REL-2");
    }

    #[test]
    fn task_identity_uses_normalized_content_only_as_last_resort() {
        let first = vec![
            record(None, None, "front, ec"),
            record(None, None, "gateway"),
        ];
        let second = vec![
            record(None, None, "gateway"),
            record(None, None, "ec,front"),
        ];

        let first_identity = build_task_identity(&first).unwrap();
        let second_identity = build_task_identity(&second).unwrap();

        assert!(first_identity.basis.starts_with("release_content:"));
        assert_eq!(first_identity.task_key, second_identity.task_key);
    }

    #[test]
    fn task_identity_last_resort_uses_services_only() {
        let first = parse_waiting_release_records(r#"{
            "code": 0,
            "data": [{
                "service": "front, ec"
            }]
        }"#).unwrap();
        let second = parse_waiting_release_records(r#"{
            "code": 0,
            "data": [{
                "service": "ec,front"
            }]
        }"#).unwrap();

        let first_identity = build_task_identity(&first).unwrap();
        let second_identity = build_task_identity(&second).unwrap();

        assert_eq!(first_identity.basis, "release_content:services:lasen-prod-ec,lasen-prod-front");
        assert_eq!(first_identity.task_key, second_identity.task_key);
    }

    #[test]
    fn task_snapshot_deduplicates_and_maps_release_services() {
        let records = vec![
            record(Some(1), None, "basedata, ecjob, front"),
            record(Some(2), None, "front,gateway"),
        ];

        let snapshot = build_task_snapshot(&records).unwrap();

        assert_eq!(
            snapshot.services,
            vec![
                "lasen-prod-base-data".to_string(),
                "lasen-prod-ec-job".to_string(),
                "lasen-prod-front".to_string(),
                "lasen-prod-gateway".to_string(),
            ]
        );
    }

    #[test]
    fn waiting_release_payload_accepts_string_ids() {
        let payload = r#"{
            "code": 0,
            "msg": "success",
            "data": [
                {
                    "id": "1001",
                    "relNo": "REL-1001",
                    "developerId": "1800513641113452545",
                    "remark": null,
                    "service": "front,ec"
                }
            ]
        }"#;

        let records = parse_waiting_release_records(payload).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, Some(1001));
        assert_eq!(records[0].developer_id, Some(1800513641113452545));
    }

    #[test]
    fn validate_release_config_only_requires_jenkins_credentials() {
        // 只依赖 Jenkins 判断发布状态，外部服务注册配置不应阻断发布。
        let config = RuntimeReleaseConfig {
            jenkins_url: Some("http://jenkins.example.com".to_string()),
            jenkins_username: Some("release-user".to_string()),
            jenkins_token: Some("release-token".to_string()),
            commit_limit: 50,
        };

        assert!(validate_release_config(&config).is_ok());
    }

    #[test]
    fn gitlab_retry_backoff_grows_by_five_seconds() {
        let delays = (1..=MAX_GITLAB_RETRIES)
            .map(gitlab_retry_delay_seconds)
            .collect::<Vec<_>>();

        assert_eq!(delays, vec![5, 10, 15, 20, 25]);
    }

    #[test]
    fn gitlab_retry_only_retries_transient_status_codes() {
        assert!(should_retry_gitlab_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(should_retry_gitlab_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
        assert!(should_retry_gitlab_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(should_retry_gitlab_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(should_retry_gitlab_status(reqwest::StatusCode::GATEWAY_TIMEOUT));

        assert!(!should_retry_gitlab_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!should_retry_gitlab_status(reqwest::StatusCode::FORBIDDEN));
        assert!(!should_retry_gitlab_status(reqwest::StatusCode::CONFLICT));
        assert!(!should_retry_gitlab_status(reqwest::StatusCode::UNPROCESSABLE_ENTITY));
    }

    #[test]
    fn jenkins_success_policy_accepts_success_and_unstable_only() {
        assert!(is_acceptable_jenkins_result("SUCCESS"));
        assert!(is_acceptable_jenkins_result("UNSTABLE"));

        assert!(!is_acceptable_jenkins_result("FAILURE"));
        assert!(!is_acceptable_jenkins_result("ABORTED"));
        assert!(!is_acceptable_jenkins_result(""));
    }

    #[test]
    fn release_progress_event_uses_camel_case_payload() {
        let event = ReleaseProgressEvent::Retry {
            step_index: 0,
            api: "GET /merge_requests".to_string(),
            retry_no: 2,
            delay_seconds: 10,
            reason: "429 Too Many Requests".to_string(),
        };

        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["event"], "retry");
        assert_eq!(value["data"]["stepIndex"], 0);
        assert_eq!(value["data"]["retryNo"], 2);
        assert_eq!(value["data"]["delaySeconds"], 10);
    }
}
