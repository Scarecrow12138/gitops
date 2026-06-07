mod db;
use log::info;
use std::process::Command;
use serde::Deserialize;

#[derive(Deserialize)]
struct MrResponse {
    web_url: String,
}

#[tauri::command]
fn greet(name: &str) -> String {
    info!("greet called with: {}", name);
    let now = chrono::Local::now();
    format!(
        "你好, {}!\n\n欢迎使用 Tauri + Vue 3 + Element Plus!\n\n当前时间: {}",
        name,
        now.format("%Y-%m-%d %H:%M:%S")
    )
}

#[tauri::command]
fn run_git_command(repo_path: String, args: Vec<String>) -> Result<String, String> {
    info!("run_git_command: repo={}, args={:?}", repo_path, args);

    let output = Command::new("git")
        .args(&args)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("无法执行 git 命令: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let err_msg = if !stderr.is_empty() { stderr } else { stdout.clone() };
        return Err(format!("git 命令执行失败: {}", err_msg.trim()));
    }

    // 合并 stdout 和 stderr（git 有时把信息写到 stderr）
    let mut result = stdout;
    if !stderr.is_empty() {
        result.push_str(&stderr);
    }

    Ok(result.trim().to_string())
}

#[tauri::command]
async fn create_gitlab_mr(
    gitlab_url: String,
    token: String,
    project_id: String,
    source_branch: String,
    target_branch: String,
    title: String,
) -> Result<String, String> {
    info!(
        "create_gitlab_mr: project={}, source={}, target={}",
        project_id, source_branch, target_branch
    );

    let url = format!(
        "{}/api/v4/projects/{}/merge_requests",
        gitlab_url.trim_end_matches('/'),
        project_id
    );

    let params = [
        ("source_branch", source_branch.as_str()),
        ("target_branch", target_branch.as_str()),
        ("title", title.as_str()),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("PRIVATE-TOKEN", &token)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("请求 GitLab API 失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitLab API 返回错误 ({}): {}",
            status, body
        ));
    }

    let mr: MrResponse = response
        .json()
        .await
        .map_err(|e| format!("解析 GitLab 响应失败: {}", e))?;

    Ok(mr.web_url)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, run_git_command, create_gitlab_mr, db::configure_database, db::check_database_connection, db::load_all_config, db::save_all_config, db::save_db_url, db::load_saved_db_url])
        .plugin(tauri_plugin_dialog::init())
        .manage(db::DbState::new())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}



