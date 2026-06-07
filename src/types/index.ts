// ===== 仓库配置 =====
export interface RepoConfig {
  id: string
  path: string
  alias: string
  currentBranch?: string
  flowTemplate?: FlowTemplate
}

// ===== 提交流程模板 =====
export interface FlowTemplate {
  id: string
  name: string
  sourceBranch: string       // 源分支模式, 如 prep-{username}
  mrTargetBranch: string     // MR 目标分支
  cpTargetBranch: string     // Cherry-pick 目标分支
}

// ===== GitLab 配置 =====
export interface GitLabConfig {
  url: string
  token: string
  projects: Record<string, string>  // project name -> project id
}

// ===== 工具定义 =====
export type ToolId = 'standard-cp' | 'hotfix-mr'

export interface ToolDef {
  id: ToolId
  title: string
  description: string
  version: string
  status: 'ready' | 'running' | 'done' | 'error'
  steps: ToolStep[]
}

export interface ToolStep {
  label: string
  status: 'pending' | 'running' | 'done' | 'error'
  detail?: string
}

// ===== 终端日志 =====
export interface LogEntry {
  text: string
  cls: 'cmd' | 'output' | 'info' | 'success' | 'warn' | 'error' | 'dim' | 'prompt'
  timestamp: number
}

// ===== 仓库 Tab =====
export interface RepoTab {
  id: string
  repoId: string
  path: string
  alias: string
  logs: LogEntry[]
  activeTool: ToolId | null
  tools: ToolDef[]
}

// ===== 全局设置 =====
export interface GlobalSettings {
  shellType: 'pwsh5' | 'pwsh7' | 'gitbash' | 'cmd'
  gitPath: string
  logRetention: number
}

// ===== 应用页面 =====
export type AppPage = 'main' | 'config' | 'help'
