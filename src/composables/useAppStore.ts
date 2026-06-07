import { reactive, ref, computed } from "vue"
import type { AppPage, RepoTab, LogEntry, ToolDef, ToolStep, RepoConfig, FlowTemplate, GitLabConfig, GlobalSettings, ToolId } from "../types"

function createStandardCpTool(): ToolDef {
  return {
    id: "standard-cp" as ToolId,
    title: "\u666e\u901a Cherry-Pick",
    description: "\u63d0\u4ea4\u5e76\u63a8\u9001\u5f53\u524d\u5206\u652f\u7684\u53d8\u66f4\uff0c\u7136\u540e cherry-pick \u5230\u76ee\u6807\u5206\u652f",
    version: "v1",
    status: "ready" as const,
    steps: [
      { label: "\u540c\u6b65\u8fdc\u7a0b\u5206\u652f\u4fe1\u606f (fetch)", status: "pending" as const },
      { label: "\u6682\u5b58\u5e76\u63d0\u4ea4\u672c\u5730\u53d8\u66f4 (add & commit)", status: "pending" as const },
      { label: "\u63a8\u9001\u6e90\u5206\u652f (push)", status: "pending" as const },
      { label: "\u5207\u6362\u5230\u76ee\u6807\u5206\u652f (checkout)", status: "pending" as const },
      { label: "\u62c9\u53d6\u76ee\u6807\u5206\u652f\u6700\u65b0\u4ee3\u7801 (pull)", status: "pending" as const },
      { label: "\u6267\u884c cherry-pick", status: "pending" as const },
      { label: "\u63a8\u9001\u76ee\u6807\u5206\u652f (push)", status: "pending" as const },
      { label: "\u5207\u56de\u6e90\u5206\u652f", status: "pending" as const },
    ],
  }
}

function createHotfixMrTool(): ToolDef {
  return {
    id: "hotfix-mr" as ToolId,
    title: "Hotfix + GitLab MR",
    description: "\u5c06\u6307\u5b9a\u63d0\u4ea4 cherry-pick \u5230 hotfix \u5206\u652f\uff0c\u63a8\u9001\u540e\u81ea\u52a8\u521b\u5efa GitLab Merge Request",
    version: "v1",
    status: "ready" as const,
    steps: [
      { label: "\u540c\u6b65\u8fdc\u7a0b\u5206\u652f\u4fe1\u606f (fetch)", status: "pending" as const },
      { label: "\u68c0\u67e5/\u521b\u5efa hotfix \u5206\u652f", status: "pending" as const },
      { label: "\u540c\u6b65\u5408\u5e76\u76ee\u6807\u5206\u652f\u5230 hotfix", status: "pending" as const },
      { label: "\u6267\u884c cherry-pick", status: "pending" as const },
      { label: "\u63a8\u9001 hotfix \u5206\u652f", status: "pending" as const },
      { label: "\u521b\u5efa GitLab Merge Request", status: "pending" as const },
      { label: "\u5207\u56de\u6e90\u5206\u652f", status: "pending" as const },
    ],
  }
}

const defaultRepos: RepoConfig[] = [
  { id: "repo-1", path: "D:\\\\workspace\\\\lcz-platform", alias: "\u9884\u53d1\u73af\u5883" },
  { id: "repo-2", path: "D:\\\\workspace\\\\deepseek-api", alias: "AI \u670d\u52a1" },
  { id: "repo-3", path: "D:\\\\workspace\\\\ops-dash", alias: "\u8fd0\u7ef4" },
]

const defaultGitLab: GitLabConfig = {
  url: "http://gitlab.5codemonkey.com:2818",
  token: "",
  projects: {
    "lasen-rear": "32",
    "lasen-ui": "33",
    "base-framework": "35",
    "lasen-module-ec": "77",
  },
}

const defaultFlow: FlowTemplate = {
  id: "flow-1",
  name: "prep-release \u63d0\u4ea4\u6d41\u7a0b",
  sourceBranch: "prep-{username}",
  mrTargetBranch: "prep-3.0",
  cpTargetBranch: "dev-3.7",
}

const defaultSettings: GlobalSettings = {
  shellType: "pwsh5",
  gitPath: "",
  logRetention: 500,
}

const currentPage = ref<AppPage>("main")
const repos = reactive<RepoConfig[]>(defaultRepos)
const gitLabConfig = reactive<GitLabConfig>(defaultGitLab)
const flowTemplate = reactive<FlowTemplate>(defaultFlow)
const settings = reactive<GlobalSettings>(defaultSettings)

const repoTabs = reactive<RepoTab[]>(
  defaultRepos.map((repo, i) => ({
    id: ("tab-" + i),
    repoId: repo.id,
    path: repo.path,
    alias: repo.alias,
    logs: [],
    activeTool: null as ToolId | null,
    tools: [createStandardCpTool(), createHotfixMrTool()],
  }))
)

const activeTabId = ref(repoTabs.length > 0 ? repoTabs[0].id : "")

const activeTab = computed(() => repoTabs.find((t) => t.id === activeTabId.value) || repoTabs[0])

function setPage(page: AppPage) {
  currentPage.value = page
}

function addLog(tabId: string, entry: LogEntry) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (!tab) return
  tab.logs.push(entry)
  if (tab.logs.length > settings.logRetention) {
    tab.logs = tab.logs.slice(-settings.logRetention)
  }
}

function clearLogs(tabId: string) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (tab) tab.logs = []
}

function setToolStatus(tabId: string, toolId: ToolId, status: ToolDef["status"]) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (!tab) return
  const tool = tab.tools.find((t) => t.id === toolId)
  if (tool) tool.status = status
}

function setStepStatus(tabId: string, toolId: ToolId, stepIndex: number, status: ToolDef["steps"][0]["status"]) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (!tab) return
  const tool = tab.tools.find((t) => t.id === toolId)
  if (tool && tool.steps[stepIndex]) tool.steps[stepIndex].status = status
}

function setActiveTool(tabId: string, toolId: ToolId | null) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (tab) tab.activeTool = toolId
}

function switchTab(tabId: string) {
  activeTabId.value = tabId
}

function addRepo(repo: RepoConfig) {
  repos.push(repo)
  const tab: RepoTab = {
    id: "tab-" + Date.now(),
    repoId: repo.id,
    path: repo.path,
    alias: repo.alias,
    logs: [],
    activeTool: null,
    tools: [createStandardCpTool(), createHotfixMrTool()],
  }
  repoTabs.push(tab)
  activeTabId.value = tab.id
}

function removeTab(tabId: string) {
  const idx = repoTabs.findIndex((t) => t.id === tabId)
  if (idx < 0) return
  repoTabs.splice(idx, 1)
  if (activeTabId.value === tabId && repoTabs.length > 0) {
    activeTabId.value = repoTabs[Math.min(idx, repoTabs.length - 1)].id
  }
}

function updateGitLabConfig(config: Partial<GitLabConfig>) {
  Object.assign(gitLabConfig, config)
}

function updateFlowTemplate(template: Partial<FlowTemplate>) {
  Object.assign(flowTemplate, template)
}

function updateSettings(s: Partial<GlobalSettings>) {
  Object.assign(settings, s)
}

function resetAllSteps(tabId: string) {
  const tab = repoTabs.find((t) => t.id === tabId)
  if (!tab) return
  tab.tools.forEach((tool) => {
    tool.status = "ready"
    tool.steps.forEach((s) => {
      s.status = "pending"
      s.detail = undefined
    })
  })
}

export function useAppStore() {
  return reactive({
    currentPage, repos, gitLabConfig, flowTemplate, settings,
    repoTabs, activeTabId, activeTab,
    setPage, addLog, clearLogs, setToolStatus, setStepStatus,
    setActiveTool, switchTab, addRepo, removeTab,
    updateGitLabConfig, updateFlowTemplate, updateSettings, resetAllSteps,
  })
}


