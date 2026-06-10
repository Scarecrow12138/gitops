use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct DbState {
    pub pool: Mutex<Option<SqlitePool>>,
}

impl DbState {
    pub fn new() -> Self {
        DbState {
            pool: Mutex::new(None),
        }
    }
}

pub(crate) fn get_connected_pool(state: &DbState) -> Result<SqlitePool, String> {
    let guard = state.pool.lock().map_err(|e| e.to_string())?;
    guard
        .clone()
        .ok_or_else(|| "请先初始化本地数据库".to_string())
}

fn sqlite_database_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("gitops.sqlite"))
        .map_err(|e| e.to_string())
}

async fn connect_sqlite(app: &AppHandle) -> Result<SqlitePool, String> {
    let path = sqlite_database_path(app)?;
    if let Some(dir) = path.parent() {
        // 数据库属于本机用户配置，程序首次运行时自动创建目录和文件。
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }

    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(|e| format!("连接本地数据库失败: {}", e))?;

    let sql = include_str!("../../db/init.sql");
    sqlx::raw_sql(sql)
        .execute(&pool)
        .await
        .map_err(|e| format!("初始化本地数据库失败: {}", e))?;
    drop_release_sql_snapshot_column(&pool).await?;

    Ok(pool)
}

async fn drop_release_sql_snapshot_column(pool: &SqlitePool) -> Result<(), String> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('release_tasks') WHERE name='sql_snapshot'")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(());
    }

    // 旧版本保存过发布 SQL 快照；当前发布链路已删除该字段，初始化时顺带迁移旧库。
    sqlx::query("ALTER TABLE release_tasks DROP COLUMN sql_snapshot")
        .execute(pool)
        .await
        .map_err(|e| format!("迁移发布任务表失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn configure_database(
    app: AppHandle,
    state: tauri::State<'_, DbState>,
) -> Result<String, String> {
    let pool = connect_sqlite(&app).await?;
    let mut guard = state.pool.lock().map_err(|e| e.to_string())?;
    *guard = Some(pool);
    Ok("本地数据库已初始化".to_string())
}

#[tauri::command]
pub async fn check_database_connection(state: tauri::State<'_, DbState>) -> Result<bool, String> {
    let guard = state.pool.lock().map_err(|e| e.to_string())?;
    Ok(guard.is_some())
}

#[tauri::command]
pub async fn load_database_path(app: AppHandle) -> Result<String, String> {
    sqlite_database_path(&app).map(|path| path.to_string_lossy().to_string())
}

// ===== 数据结构 =====

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct RepoData {
    pub id: String,
    pub path: String,
    pub alias: String,
    pub current_branch: Option<String>,
    pub flow_template_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct FlowTemplateData {
    pub id: String,
    pub name: String,
    pub source_branch: Option<String>,
    pub mr_target_branch: String,
    pub cp_target_branch: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct GitLabConfigData {
    pub id: String,
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ProjectMappingData {
    pub gitlab_config_id: String,
    pub project_name: String,
    pub project_gitlab_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct GlobalSettingsData {
    pub shell_type: String,
    pub git_path: Option<String>,
    pub log_retention: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ReleaseConfigData {
    pub id: String,
    pub jenkins_url: Option<String>,
    pub jenkins_username: Option<String>,
    pub jenkins_token: Option<String>,
    pub commit_limit: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct RepoToolData {
    pub repo_id: String,
    pub tool_id: String,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AllConfigData {
    pub repositories: Vec<RepoData>,
    pub flow_templates: Vec<FlowTemplateData>,
    pub gitlab_configs: Vec<GitLabConfigData>,
    pub project_mappings: Vec<ProjectMappingData>,
    pub global_settings: Vec<GlobalSettingsData>,
    pub release_configs: Vec<ReleaseConfigData>,
    pub repo_tools: Vec<RepoToolData>,
}

// ===== 加载全部配置 =====

#[tauri::command]
pub async fn load_all_config(state: tauri::State<'_, DbState>) -> Result<AllConfigData, String> {
    let pool = get_connected_pool(state.inner())?;

    let repositories = sqlx::query_as::<_, RepoData>(
        "SELECT id, path, alias, current_branch, flow_template_id FROM repositories ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let flow_templates = sqlx::query_as::<_, FlowTemplateData>(
        "SELECT id, name, source_branch, mr_target_branch, cp_target_branch FROM flow_templates ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let gitlab_configs = sqlx::query_as::<_, GitLabConfigData>(
        "SELECT id, url, token FROM gitlab_configs ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let project_mappings = sqlx::query_as::<_, ProjectMappingData>(
        "SELECT gitlab_config_id, project_name, project_gitlab_id FROM project_mappings ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let global_settings = sqlx::query_as::<_, GlobalSettingsData>(
        "SELECT shell_type, git_path, log_retention FROM global_settings ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let release_configs = sqlx::query_as::<_, ReleaseConfigData>(
        "SELECT id, jenkins_url, jenkins_username, jenkins_token, commit_limit FROM release_configs ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let repo_tools = sqlx::query_as::<_, RepoToolData>(
        "SELECT repo_id, tool_id, sort_order FROM repo_tools ORDER BY repo_id, sort_order",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(AllConfigData {
        repositories,
        flow_templates,
        gitlab_configs,
        project_mappings,
        global_settings,
        release_configs,
        repo_tools,
    })
}

fn bind_repo_delete_query<'q>(
    sql: &'q str,
    repo_ids: &'q [String],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    let mut query = sqlx::query(sql);
    for repo_id in repo_ids {
        query = query.bind(repo_id);
    }
    query
}

// ===== 保存全部配置 =====

#[tauri::command]
pub async fn save_all_config(
    state: tauri::State<'_, DbState>,
    data: AllConfigData,
) -> Result<String, String> {
    let pool = get_connected_pool(state.inner())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for r in &data.repositories {
        sqlx::query(
            "INSERT INTO repositories (id,path,alias,current_branch,flow_template_id)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT (id) DO UPDATE SET
                path=excluded.path,
                alias=excluded.alias,
                current_branch=excluded.current_branch,
                flow_template_id=excluded.flow_template_id,
                updated_at=CURRENT_TIMESTAMP",
        )
        .bind(&r.id)
        .bind(&r.path)
        .bind(&r.alias)
        .bind(&r.current_branch)
        .bind(&r.flow_template_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    // 以前端仓库列表为准，删除数据库中已经移除的仓库。
    let repo_ids: Vec<String> = data.repositories.iter().map(|r| r.id.clone()).collect();
    if repo_ids.is_empty() {
        sqlx::query("DELETE FROM repositories")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let placeholders = std::iter::repeat("?")
            .take(repo_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM repositories WHERE id NOT IN ({})",
            placeholders
        );
        bind_repo_delete_query(&sql, &repo_ids)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    for f in &data.flow_templates {
        sqlx::query(
            "INSERT INTO flow_templates (id,name,source_branch,mr_target_branch,cp_target_branch)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT (id) DO UPDATE SET
                name=excluded.name,
                source_branch=excluded.source_branch,
                mr_target_branch=excluded.mr_target_branch,
                cp_target_branch=excluded.cp_target_branch,
                updated_at=CURRENT_TIMESTAMP",
        )
        .bind(&f.id)
        .bind(&f.name)
        .bind(&f.source_branch)
        .bind(&f.mr_target_branch)
        .bind(&f.cp_target_branch)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    for g in &data.gitlab_configs {
        sqlx::query(
            "INSERT INTO gitlab_configs (id,url,token)
             VALUES (?1,?2,?3)
             ON CONFLICT (id) DO UPDATE SET
                url=excluded.url,
                token=excluded.token,
                updated_at=CURRENT_TIMESTAMP",
        )
        .bind(&g.id)
        .bind(&g.url)
        .bind(&g.token)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    sqlx::query("DELETE FROM project_mappings")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for p in &data.project_mappings {
        sqlx::query(
            "INSERT INTO project_mappings (gitlab_config_id,project_name,project_gitlab_id)
             VALUES (?1,?2,?3)",
        )
        .bind(&p.gitlab_config_id)
        .bind(&p.project_name)
        .bind(&p.project_gitlab_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    for s in &data.global_settings {
        sqlx::query(
            "INSERT INTO global_settings (id,shell_type,git_path,log_retention)
             VALUES ('default',?1,?2,?3)
             ON CONFLICT (id) DO UPDATE SET
                shell_type=excluded.shell_type,
                git_path=excluded.git_path,
                log_retention=excluded.log_retention,
                updated_at=CURRENT_TIMESTAMP",
        )
        .bind(&s.shell_type)
        .bind(&s.git_path)
        .bind(&s.log_retention)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    for r in &data.release_configs {
        sqlx::query(
            "INSERT INTO release_configs (id,jenkins_url,jenkins_username,jenkins_token,commit_limit)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT (id) DO UPDATE SET
                jenkins_url=excluded.jenkins_url,
                jenkins_username=excluded.jenkins_username,
                jenkins_token=excluded.jenkins_token,
                commit_limit=excluded.commit_limit,
                updated_at=CURRENT_TIMESTAMP",
        )
        .bind(&r.id)
        .bind(&r.jenkins_url)
        .bind(&r.jenkins_username)
        .bind(&r.jenkins_token)
        .bind(&r.commit_limit)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    sqlx::query("DELETE FROM repo_tools")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for t in &data.repo_tools {
        sqlx::query("INSERT INTO repo_tools (repo_id,tool_id,sort_order) VALUES (?1,?2,?3)")
            .bind(&t.repo_id)
            .bind(&t.tool_id)
            .bind(&t.sort_order)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok("配置已保存到本地数据库".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir() -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gitops-sqlite-test-{}-{}-{}",
            std::process::id(),
            millis,
            seq
        ))
    }

    #[test]
    fn sqlite_database_file_name_is_stable() {
        let root = unique_temp_dir();
        let path = root.join("gitops.sqlite");

        assert_eq!(path.file_name().unwrap(), "gitops.sqlite");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sqlite_schema_initializes_without_postgres_syntax() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("test.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await.unwrap();

        sqlx::raw_sql(include_str!("../../db/init.sql"))
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM global_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let release_table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('release_tasks','release_attempts')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(release_table_count, 2);

        let sql_snapshot_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('release_tasks') WHERE name='sql_snapshot'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(sql_snapshot_count, 0);

        drop(pool);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn migration_removes_legacy_release_sql_snapshot_column() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("legacy.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await.unwrap();

        sqlx::query(
            "CREATE TABLE release_tasks (
                id TEXT PRIMARY KEY,
                task_key TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                record_ids TEXT NOT NULL DEFAULT '[]',
                rel_nos TEXT NOT NULL DEFAULT '[]',
                service_snapshot TEXT NOT NULL DEFAULT '[]',
                sql_snapshot TEXT NOT NULL DEFAULT '[]',
                raw_snapshot TEXT NOT NULL DEFAULT '[]',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        drop_release_sql_snapshot_column(&pool).await.unwrap();

        let sql_snapshot_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('release_tasks') WHERE name='sql_snapshot'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(sql_snapshot_count, 0);

        drop(pool);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repo_delete_query_uses_sqlite_placeholders() {
        let repo_ids = ["repo-a".to_string(), "repo-b".to_string()];
        let placeholders = std::iter::repeat("?")
            .take(repo_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM repositories WHERE id NOT IN ({})",
            placeholders
        );

        assert_eq!(sql, "DELETE FROM repositories WHERE id NOT IN (?,?)");
    }
}
