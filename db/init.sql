-- ============================================
-- GitOps 配置落表 - PostgreSQL DDL
-- 数据库类型：Neon (PostgreSQL)
-- ============================================

-- ===== 1. 仓库表 =====
CREATE TABLE IF NOT EXISTS repositories (
    id          VARCHAR(64) PRIMARY KEY,
    path        TEXT NOT NULL,
    alias       VARCHAR(100) NOT NULL,
    current_branch VARCHAR(255),
    flow_template_id VARCHAR(64),
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

-- ===== 2. 提交流程模板表 =====
CREATE TABLE IF NOT EXISTS flow_templates (
    id               VARCHAR(64) PRIMARY KEY,
    name             VARCHAR(200) NOT NULL,
    source_branch    VARCHAR(255) DEFAULT '{username}',
    mr_target_branch VARCHAR(255) NOT NULL,
    cp_target_branch VARCHAR(255) NOT NULL,
    created_at       TIMESTAMPTZ DEFAULT NOW(),
    updated_at       TIMESTAMPTZ DEFAULT NOW()
);

-- ===== 3. GitLab 配置表 =====
CREATE TABLE IF NOT EXISTS gitlab_configs (
    id         VARCHAR(64) PRIMARY KEY DEFAULT 'default',
    url        VARCHAR(500) NOT NULL,
    token      TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ===== 4. 项目映射表 =====
CREATE TABLE IF NOT EXISTS project_mappings (
    id               SERIAL PRIMARY KEY,
    gitlab_config_id VARCHAR(64) DEFAULT 'default'
                     REFERENCES gitlab_configs(id) ON DELETE CASCADE,
    project_name     VARCHAR(200) NOT NULL,
    project_gitlab_id VARCHAR(64) NOT NULL,
    created_at       TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (gitlab_config_id, project_name)
);

-- ===== 5. 全局设置表 =====
CREATE TABLE IF NOT EXISTS global_settings (
    id            VARCHAR(64) PRIMARY KEY DEFAULT 'default',
    shell_type    VARCHAR(20) DEFAULT 'pwsh5'
                  CHECK (shell_type IN ('pwsh5', 'pwsh7', 'gitbash', 'cmd')),
    git_path      TEXT DEFAULT '',
    log_retention INTEGER DEFAULT 500
                  CHECK (log_retention >= 100 AND log_retention <= 5000),
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    updated_at    TIMESTAMPTZ DEFAULT NOW()
);

-- ===== 6. 仓库-工具关联表 =====
CREATE TABLE IF NOT EXISTS repo_tools (
    id          SERIAL PRIMARY KEY,
    repo_id     VARCHAR(64) NOT NULL
                REFERENCES repositories(id) ON DELETE CASCADE,
    tool_id     VARCHAR(64) NOT NULL
                CHECK (tool_id IN ('standard-cp', 'hotfix-mr')),
    tool_config JSONB DEFAULT '{}',
    sort_order  INTEGER DEFAULT 0,
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (repo_id, tool_id)
);

-- ===== 7. 执行历史表 =====
CREATE TABLE IF NOT EXISTS execution_history (
    id          SERIAL PRIMARY KEY,
    repo_id     VARCHAR(64)
                REFERENCES repositories(id) ON DELETE SET NULL,
    tool_id     VARCHAR(64) NOT NULL,
    status      VARCHAR(20) DEFAULT 'ready'
                CHECK (status IN ('ready', 'running', 'done', 'error')),
    output      TEXT,
    duration_ms INTEGER,
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- ===== 8. 输入历史表 =====
CREATE TABLE IF NOT EXISTS input_histories (
    id         SERIAL PRIMARY KEY,
    category   VARCHAR(100) NOT NULL,
    value      TEXT NOT NULL,
    count      INTEGER DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (category, value)
);

-- ============================================
-- 初始数据
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

INSERT INTO repositories (id, path, alias, flow_template_id) VALUES
    ('repo-1', 'D:\workspace\lcz-platform',  '预发环境', 'flow-1'),
    ('repo-2', 'D:\workspace\deepseek-api',  'AI 服务',  'flow-1'),
    ('repo-3', 'D:\workspace\ops-dash',      '运维',     NULL)
ON CONFLICT (id) DO NOTHING;

INSERT INTO repo_tools (repo_id, tool_id, sort_order) VALUES
    ('repo-1', 'standard-cp', 1),
    ('repo-1', 'hotfix-mr',   2),
    ('repo-2', 'standard-cp', 1),
    ('repo-2', 'hotfix-mr',   2),
    ('repo-3', 'standard-cp', 1),
    ('repo-3', 'hotfix-mr',   2)
ON CONFLICT (repo_id, tool_id) DO NOTHING;

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
