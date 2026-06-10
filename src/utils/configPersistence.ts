import { invoke } from '@tauri-apps/api/core'
import type { FlowTemplate, GitLabConfig, GlobalSettings, ReleaseConfig, RepoConfig } from '../types'

interface ProjectMappingPayload {
  gitlab_config_id: string
  project_name: string
  project_gitlab_id: string
}

interface SaveConfigPayload {
  repositories: Array<{
    id: string
    path: string
    alias: string
    current_branch?: string
    flow_template_id?: string
  }>
  flow_templates: Array<{
    id: string
    name: string
    source_branch: string
    mr_target_branch: string
    cp_target_branch: string
  }>
  gitlab_configs: Array<{
    id: string
    url: string
    token?: string
  }>
  project_mappings: ProjectMappingPayload[]
  global_settings: Array<{
    shell_type: string
    git_path?: string
    log_retention: number
  }>
  release_configs: Array<{
    id: string
    jenkins_url?: string
    jenkins_username?: string
    jenkins_token?: string
    commit_limit: number
  }>
  repo_tools: Array<{
    repo_id: string
    tool_id: string
    sort_order: number
  }>
}

interface LoadedConfigPayload {
  repositories: Array<{
    id: string
    path: string
    alias: string
    current_branch?: string | null
    flow_template_id?: string | null
  }>
  flow_templates: Array<{
    id: string
    name: string
    source_branch?: string | null
    mr_target_branch: string
    cp_target_branch: string
  }>
  gitlab_configs: Array<{
    id: string
    url: string
    token?: string | null
  }>
  project_mappings: ProjectMappingPayload[]
  global_settings: Array<{
    shell_type: GlobalSettings['shellType']
    git_path?: string | null
    log_retention: number
  }>
  release_configs: Array<{
    id: string
    jenkins_url?: string | null
    jenkins_username?: string | null
    jenkins_token?: string | null
    commit_limit: number
  }>
  repo_tools: Array<{
    repo_id: string
    tool_id: string
    sort_order: number
  }>
}

interface ConfigStore {
  repos: RepoConfig[]
  flowTemplate: FlowTemplate
  gitLabConfig: GitLabConfig
  settings: GlobalSettings
  releaseConfig: ReleaseConfig
  replaceRepos(repositories: RepoConfig[]): void
  updateFlowTemplate(template: Partial<FlowTemplate>): void
  updateGitLabConfig(config: Partial<GitLabConfig>): void
  updateSettings(settings: Partial<GlobalSettings>): void
  updateReleaseConfig(config: Partial<ReleaseConfig>): void
}

export function buildConfigPayload(
  repositories: RepoConfig[],
  flowTemplate: FlowTemplate,
  gitLabConfig: GitLabConfig,
  settings: GlobalSettings,
  releaseConfig: ReleaseConfig,
): SaveConfigPayload {
  return {
    repositories: repositories.map((repo) => ({
      id: repo.id,
      path: repo.path,
      alias: repo.alias,
      current_branch: repo.currentBranch,
      flow_template_id: repo.flowTemplate?.id ?? flowTemplate.id,
    })),
    flow_templates: [{
      id: flowTemplate.id,
      name: flowTemplate.name,
      source_branch: flowTemplate.sourceBranch,
      mr_target_branch: flowTemplate.mrTargetBranch,
      cp_target_branch: flowTemplate.cpTargetBranch,
    }],
    gitlab_configs: [{
      id: 'default',
      url: gitLabConfig.url,
      token: gitLabConfig.token,
    }],
    project_mappings: Object.entries(gitLabConfig.projects).map(([name, id]) => ({
      gitlab_config_id: 'default',
      project_name: name,
      project_gitlab_id: id,
    })),
    global_settings: [{
      shell_type: settings.shellType,
      git_path: settings.gitPath,
      log_retention: settings.logRetention,
    }],
    release_configs: [{
      id: 'default',
      jenkins_url: releaseConfig.jenkinsUrl,
      jenkins_username: releaseConfig.jenkinsUsername,
      jenkins_token: releaseConfig.jenkinsToken,
      commit_limit: releaseConfig.commitLimit,
    }],
    repo_tools: repositories.flatMap((repo) => [
      { repo_id: repo.id, tool_id: 'standard-cp', sort_order: 1 },
      { repo_id: repo.id, tool_id: 'hotfix-mr', sort_order: 2 },
    ]),
  }
}

export function applyLoadedConfigToStore(store: ConfigStore, data: LoadedConfigPayload) {
  const repositories: RepoConfig[] = data.repositories.map((repo) => ({
    id: repo.id,
    path: repo.path,
    alias: repo.alias,
    currentBranch: repo.current_branch ?? undefined,
  }))
  store.replaceRepos(repositories)

  const flow = data.flow_templates[0]
  if (flow) {
    store.updateFlowTemplate({
      id: flow.id,
      name: flow.name,
      sourceBranch: flow.source_branch ?? '',
      mrTargetBranch: flow.mr_target_branch,
      cpTargetBranch: flow.cp_target_branch,
    })
  }

  const gitLab = data.gitlab_configs[0]
  if (gitLab) {
    const projects: Record<string, string> = {}
    data.project_mappings
      .filter((item) => item.gitlab_config_id === gitLab.id)
      .forEach((item) => {
        projects[item.project_name] = item.project_gitlab_id
      })

    store.updateGitLabConfig({
      url: gitLab.url,
      token: gitLab.token ?? '',
      projects,
    })
  }

  const settings = data.global_settings[0]
  if (settings) {
    store.updateSettings({
      shellType: settings.shell_type,
      gitPath: settings.git_path ?? '',
      logRetention: settings.log_retention,
    })
  }

  const releaseConfig = data.release_configs[0]
  if (releaseConfig) {
    store.updateReleaseConfig({
      jenkinsUrl: releaseConfig.jenkins_url ?? '',
      jenkinsUsername: releaseConfig.jenkins_username ?? '',
      jenkinsToken: releaseConfig.jenkins_token ?? '',
      commitLimit: releaseConfig.commit_limit,
    })
  }
}

export async function loadConfigFromDatabase(store: ConfigStore) {
  await ensureDatabaseConfigured()
  const data = await invoke<LoadedConfigPayload>('load_all_config')
  applyLoadedConfigToStore(store, data)
  return data
}

export async function ensureDatabaseConfigured() {
  const connected = await invoke<boolean>('check_database_connection')
  if (!connected) {
    await invoke('configure_database')
  }
}

export async function loadDatabasePath() {
  return invoke<string>('load_database_path')
}

export async function saveConfigToDatabase(
  repositories: RepoConfig[],
  flowTemplate: FlowTemplate,
  gitLabConfig: GitLabConfig,
  settings: GlobalSettings,
  releaseConfig: ReleaseConfig,
) {
  // 数据库连接池只存在于当前进程，未连接时自动初始化本地 SQLite。
  await ensureDatabaseConfigured()

  await invoke('save_all_config', {
    data: buildConfigPayload(repositories, flowTemplate, gitLabConfig, settings, releaseConfig),
  })
}
