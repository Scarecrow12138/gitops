use sqlx::postgres::PgPool;
use serde::{Serialize, Deserialize};
use std::sync::Mutex;

pub struct DbState {
    pub pool: Mutex<Option<PgPool>>,
}

impl DbState {
    pub fn new() -> Self {
        DbState { pool: Mutex::new(None) }
    }
}

fn get_connected_pool(state: &DbState) -> Result<PgPool, String> {
    let guard = state.pool.lock().map_err(|e| e.to_string())?;
    guard.clone().ok_or_else(|| "\u{8bf7}\u{5148}\u{914d}\u{7f6e}\u{6570}\u{636e}\u{5e93}\u{8fde}\u{63a5}".to_string())
}

#[tauri::command]
pub async fn configure_database(
    url: String,
    state: tauri::State<'_, DbState>,
) -> Result<String, String> {
    let pool = PgPool::connect(&url)
        .await
        .map_err(|e| format!("\u{8fde}\u{63a5}\u{5931}\u{8d25}: {}", e))?;
    let sql = include_str!("../../db/init.sql");
    sqlx::raw_sql(sql).execute(&pool).await
        .map_err(|e| format!("\u{521d}\u{59cb}\u{5316}\u{5931}\u{8d25}: {}", e))?;
    let mut guard = state.pool.lock().map_err(|e| e.to_string())?;
    *guard = Some(pool);
    Ok("\u{6570}\u{636e}\u{5e93}\u{5df2}\u{521d}\u{59cb}\u{5316}".to_string())
}

#[tauri::command]
pub async fn check_database_connection(state: tauri::State<'_, DbState>) -> Result<bool, String> {
    let guard = state.pool.lock().map_err(|e| e.to_string())?;
    Ok(guard.is_some())
}

// ===== Data types =====

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
    pub repo_tools: Vec<RepoToolData>,
}

// ===== Load all config =====

#[tauri::command]
pub async fn load_all_config(
    state: tauri::State<'_, DbState>,
) -> Result<AllConfigData, String> {
    let pool = get_connected_pool(state.inner())?;

    let repositories: Vec<RepoData> = sqlx::query_as::<_, RepoData>(
        "SELECT id, path, alias, current_branch, flow_template_id FROM repositories ORDER BY id"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let flow_templates: Vec<FlowTemplateData> = sqlx::query_as::<_, FlowTemplateData>(
        "SELECT id, name, source_branch, mr_target_branch, cp_target_branch FROM flow_templates ORDER BY id"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let gitlab_configs: Vec<GitLabConfigData> = sqlx::query_as::<_, GitLabConfigData>(
        "SELECT id, url, token FROM gitlab_configs ORDER BY id"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let project_mappings: Vec<ProjectMappingData> = sqlx::query_as::<_, ProjectMappingData>(
        "SELECT gitlab_config_id, project_name, project_gitlab_id FROM project_mappings ORDER BY id"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let global_settings: Vec<GlobalSettingsData> = sqlx::query_as::<_, GlobalSettingsData>(
        "SELECT shell_type, git_path, log_retention FROM global_settings ORDER BY id"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let repo_tools: Vec<RepoToolData> = sqlx::query_as::<_, RepoToolData>(
        "SELECT repo_id, tool_id, sort_order FROM repo_tools ORDER BY repo_id, sort_order"
    ).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    Ok(AllConfigData {
        repositories, flow_templates, gitlab_configs,
        project_mappings, global_settings, repo_tools,
    })
}

// ===== Save all config =====

#[tauri::command]
pub async fn save_all_config(
    state: tauri::State<'_, DbState>,
    data: AllConfigData,
) -> Result<String, String> {
    let pool = get_connected_pool(state.inner())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for r in &data.repositories {
        sqlx::query("INSERT INTO repositories (id,path,alias,current_branch,flow_template_id) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (id) DO UPDATE SET path=$2,alias=$3,current_branch=$4,flow_template_id=$5,updated_at=NOW()")
            .bind(&r.id).bind(&r.path).bind(&r.alias)
            .bind(&r.current_branch).bind(&r.flow_template_id)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    for f in &data.flow_templates {
        sqlx::query("INSERT INTO flow_templates (id,name,source_branch,mr_target_branch,cp_target_branch) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (id) DO UPDATE SET name=$2,source_branch=$3,mr_target_branch=$4,cp_target_branch=$5,updated_at=NOW()")
            .bind(&f.id).bind(&f.name).bind(&f.source_branch)
            .bind(&f.mr_target_branch).bind(&f.cp_target_branch)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    for g in &data.gitlab_configs {
        sqlx::query("INSERT INTO gitlab_configs (id,url,token) VALUES ($1,$2,$3) ON CONFLICT (id) DO UPDATE SET url=$2,token=$3,updated_at=NOW()")
            .bind(&g.id).bind(&g.url).bind(&g.token)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    sqlx::query("DELETE FROM project_mappings").execute(&mut *tx).await.map_err(|e| e.to_string())?;
    for p in &data.project_mappings {
        sqlx::query("INSERT INTO project_mappings (gitlab_config_id,project_name,project_gitlab_id) VALUES ($1,$2,$3)")
            .bind(&p.gitlab_config_id).bind(&p.project_name).bind(&p.project_gitlab_id)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    for s in &data.global_settings {
        sqlx::query("INSERT INTO global_settings (shell_type,git_path,log_retention) VALUES ($1,$2,$3) ON CONFLICT (id) DO UPDATE SET shell_type=$1,git_path=$2,log_retention=$3,updated_at=NOW()")
            .bind(&s.shell_type).bind(&s.git_path).bind(&s.log_retention)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    sqlx::query("DELETE FROM repo_tools").execute(&mut *tx).await.map_err(|e| e.to_string())?;
    for t in &data.repo_tools {
        sqlx::query("INSERT INTO repo_tools (repo_id,tool_id,sort_order) VALUES ($1,$2,$3)")
            .bind(&t.repo_id).bind(&t.tool_id).bind(&t.sort_order)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok("\u{914d}\u{7f6e}\u{5df2}\u{4fdd}\u{5b58}\u{5230}\u{6570}\u{636e}\u{5e93}".to_string())
}

// ===== DB URL persistence =====

#[tauri::command]
pub async fn save_db_url(url: String) -> Result<(), String> {
    let home = std::env::current_dir().map_err(|e| e.to_string())?;
    let dir = home.join("gitops");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("db_url.txt"), &url).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn load_saved_db_url() -> Result<String, String> {
    let home = std::env::current_dir().map_err(|e| e.to_string())?;
    let path = home.join("gitops").join("db_url.txt");
    std::fs::read_to_string(path).map_err(|_| "\u{672a}\u{627e}\u{5230}\u{4fdd}\u{5b58}\u{7684}\u{8fde}\u{63a5}\u{5b57}\u{7b26}\u{4e32}".to_string())
}
