-- ============================================
-- GitOps 配置表 - SQLite DDL
-- 数据库文件由桌面应用在本机自动创建
-- ============================================

PRAGMA foreign_keys = ON;

-- ===== 1. 仓库表 =====
CREATE TABLE IF NOT EXISTS repositories (
    id               TEXT PRIMARY KEY,
    path             TEXT NOT NULL,
    alias            TEXT NOT NULL,
    current_branch   TEXT,
    flow_template_id TEXT,
    created_at       TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at       TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 2. 提交流程模板表 =====
CREATE TABLE IF NOT EXISTS flow_templates (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    source_branch    TEXT DEFAULT '{username}',
    mr_target_branch TEXT NOT NULL,
    cp_target_branch TEXT NOT NULL,
    created_at       TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at       TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 3. GitLab 配置表 =====
CREATE TABLE IF NOT EXISTS gitlab_configs (
    id         TEXT PRIMARY KEY DEFAULT 'default',
    url        TEXT NOT NULL,
    token      TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 4. 项目映射表 =====
CREATE TABLE IF NOT EXISTS project_mappings (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    gitlab_config_id   TEXT DEFAULT 'default'
                       REFERENCES gitlab_configs(id) ON DELETE CASCADE,
    project_name       TEXT NOT NULL,
    project_gitlab_id  TEXT NOT NULL,
    created_at         TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (gitlab_config_id, project_name)
);

-- ===== 5. 全局设置表 =====
CREATE TABLE IF NOT EXISTS global_settings (
    id            TEXT PRIMARY KEY DEFAULT 'default',
    shell_type    TEXT DEFAULT 'pwsh5'
                  CHECK (shell_type IN ('pwsh5', 'pwsh7', 'gitbash', 'cmd')),
    git_path      TEXT DEFAULT '',
    log_retention INTEGER DEFAULT 500
                  CHECK (log_retention >= 100 AND log_retention <= 5000),
    created_at    TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at    TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 6. 仓库-工具关联表 =====
CREATE TABLE IF NOT EXISTS repo_tools (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     TEXT NOT NULL
                REFERENCES repositories(id) ON DELETE CASCADE,
    tool_id     TEXT NOT NULL
                CHECK (tool_id IN ('standard-cp', 'hotfix-mr')),
    tool_config TEXT DEFAULT '{}',
    sort_order  INTEGER DEFAULT 0,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (repo_id, tool_id)
);

-- ===== 7. 执行历史表 =====
CREATE TABLE IF NOT EXISTS execution_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     TEXT
                REFERENCES repositories(id) ON DELETE SET NULL,
    tool_id     TEXT NOT NULL,
    status      TEXT DEFAULT 'ready'
                CHECK (status IN ('ready', 'running', 'done', 'error')),
    output      TEXT,
    duration_ms INTEGER,
    started_at  TEXT,
    finished_at TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 8. 输入历史表 =====
CREATE TABLE IF NOT EXISTS input_histories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    category   TEXT NOT NULL,
    value      TEXT NOT NULL,
    count      INTEGER DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (category, value)
);

-- ===== 9. Release config table =====
CREATE TABLE IF NOT EXISTS release_configs (
    id              TEXT PRIMARY KEY DEFAULT 'default',
    jenkins_url     TEXT DEFAULT '',
    jenkins_username TEXT DEFAULT '',
    jenkins_token   TEXT DEFAULT '',
    commit_limit    INTEGER DEFAULT 50,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 10. Release task table =====
CREATE TABLE IF NOT EXISTS release_tasks (
    id               TEXT PRIMARY KEY,
    task_key         TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'success', 'failed')),
    record_ids       TEXT NOT NULL DEFAULT '[]',
    rel_nos          TEXT NOT NULL DEFAULT '[]',
    service_snapshot TEXT NOT NULL DEFAULT '[]',
    raw_snapshot     TEXT NOT NULL DEFAULT '[]',
    created_at       TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at       TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ===== 11. Release attempt table =====
CREATE TABLE IF NOT EXISTS release_attempts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     TEXT NOT NULL
                REFERENCES release_tasks(id) ON DELETE CASCADE,
    attempt_no  INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'running'
                CHECK (status IN ('running', 'success', 'failed')),
    started_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    finished_at TEXT,
    failed_step TEXT,
    log_output  TEXT DEFAULT '',
    UNIQUE (task_id, attempt_no)
);

-- ============================================
-- 初始配置
-- ============================================

INSERT INTO gitlab_configs (id, url, token)
VALUES ('default', 'http://gitlab.5codemonkey.com:2818', '')
ON CONFLICT (id) DO NOTHING;

INSERT INTO project_mappings (gitlab_config_id, project_name, project_gitlab_id) VALUES
    ('default', 'lasen-rear',       '32'),
    ('default', 'lasen-ui',         '33'),
    ('default', 'base-framework',   '35'),
    ('default', 'lasen-module-ec',  '77')
ON CONFLICT (gitlab_config_id, project_name) DO NOTHING;

INSERT INTO flow_templates (id, name, source_branch, mr_target_branch, cp_target_branch) VALUES
    ('flow-1', 'prep-release 提交流程', 'prep-{username}', 'prep-3.0', 'dev-3.7')
ON CONFLICT (id) DO NOTHING;

INSERT INTO global_settings (id, shell_type, git_path, log_retention)
VALUES ('default', 'pwsh5', '', 500)
ON CONFLICT (id) DO NOTHING;

INSERT INTO release_configs (id, jenkins_url, jenkins_username, jenkins_token, commit_limit)
VALUES ('default', 'http://jenkins.5codemonkey.com:1820', '', '', 50)
ON CONFLICT (id) DO NOTHING;

-- 仓库和工具关联由用户在配置页维护，不写入测试仓库。

-- ============================================
-- 索引
-- ============================================
CREATE INDEX IF NOT EXISTS idx_project_mappings_gitlab_config
    ON project_mappings(gitlab_config_id);
CREATE INDEX IF NOT EXISTS idx_repo_tools_repo_id
    ON repo_tools(repo_id);
CREATE INDEX IF NOT EXISTS idx_execution_history_repo_id
    ON execution_history(repo_id);
CREATE INDEX IF NOT EXISTS idx_execution_history_created_at
    ON execution_history(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_input_histories_category_value
    ON input_histories(category, value);
CREATE INDEX IF NOT EXISTS idx_release_tasks_task_key_status
    ON release_tasks(task_key, status);
CREATE INDEX IF NOT EXISTS idx_release_tasks_updated_at
    ON release_tasks(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_release_attempts_task_id_attempt_no
    ON release_attempts(task_id, attempt_no DESC);
