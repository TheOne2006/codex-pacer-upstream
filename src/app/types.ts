export type OverviewBucket =
  | 'five_hour'
  | 'day'
  | 'seven_day'
  | 'week'
  | 'month'
  | 'year'
  | 'custom'
  | 'total'

export type ShareMode = 'value' | 'tokens'
export type ShareDimension = 'model' | 'composition' | 'source'
export type AppView = 'overview' | 'conversations'
export type MenuBarPopupModuleId =
  | 'api_value'
  | 'token_count'
  | 'scan_freshness'
  | 'live_quota_freshness'
  | 'payoff_ratio'
  | 'conversation_count'

export interface SyncSettings {
  codexHome: string | null
  autoScanEnabled: boolean
  autoScanIntervalMinutes: number
  liveQuotaRefreshIntervalSeconds: number
  hideDockIconWhenMenuBarVisible: boolean
  showMenuBarLogo: boolean
  showMenuBarDailyApiValue: boolean
  showMenuBarLiveQuotaPercent: boolean
  menuBarLiveQuotaMetric: 'remaining_percent' | 'suggested_usage_speed'
  menuBarLiveQuotaBucket: 'five_hour' | 'seven_day'
  menuBarBucket: OverviewBucket
  menuBarSpeedShowEmoji: boolean
  menuBarSpeedFastThresholdPercent: number
  menuBarSpeedSlowThresholdPercent: number
  menuBarSpeedHealthyEmoji: string
  menuBarSpeedFastEmoji: string
  menuBarSpeedSlowEmoji: string
  menuBarPopupEnabled: boolean
  menuBarPopupModules: MenuBarPopupModuleId[]
  menuBarPopupShowResetTimeline: boolean
  menuBarPopupShowActions: boolean
  lastScanStartedAt: string | null
  lastScanCompletedAt: string | null
  updatedAt: string
}

export interface CodexSource {
  id: string
  kind: 'local' | 'ssh' | string
  label: string
  sshAlias: string | null
  hostName: string | null
  user: string | null
  port: number | null
  remoteCodexHome: string | null
  localCodexHome: string | null
  selected: boolean
  status: string
  lastDiscoveredAt: string | null
  lastDownloadedAt: string | null
  lastScannedAt: string | null
  lastError: string | null
  createdAt: string
  updatedAt: string
}

export interface CodexSourceCandidate {
  id: string
  label: string
  sshAlias: string
  hostName: string | null
  user: string | null
  port: number | null
  remoteCodexHome: string
  ignoredReason: string | null
}

export interface CodexSourceInput {
  label: string
  sshAlias: string
  hostName: string | null
  user: string | null
  port: number | null
  remoteCodexHome: string
  selected: boolean
}

export interface CodexSourceDownloadProgress {
  sourceId: string
  stage: string
  progress: number | null
  message: string
}

export interface CodexSourceDownloadResult {
  source: CodexSource
  scanResult: ScanResult
}

export interface CodexSourceDownloadFailure {
  sourceId: string
  error: string
}

export interface CodexSourceBatchDownloadResult {
  results: CodexSourceDownloadResult[]
  failures: CodexSourceDownloadFailure[]
}

export interface SubscriptionProfile {
  planType: string
  currency: string
  monthlyPrice: number
  billingAnchorDay: number
  updatedAt: string
}

export interface SubscriptionRecord {
  id: number
  paidAt: string
  serviceStart: string
  serviceEnd: string
  amountUsd: number
  planType: string
  note: string | null
  createdAt: string
  updatedAt: string
}

export type SubscriptionRecordInput = Omit<SubscriptionRecord, 'id' | 'createdAt' | 'updatedAt'>

export interface CodexAccountStatus {
  available: boolean
  requiresOpenaiAuth: boolean
  authMode: string | null
  accountType: string | null
  email: string | null
  planType: string | null
  error: string | null
  fetchedAt: string
}

export interface ScanResult {
  codexHome: string
  scannedFiles: number
  importedSessions: number
  updatedSessions: number
  missingSessions: number
  lastCompletedAt: string
}

export interface OverviewStats {
  apiValueUsd: number
  subscriptionCostUsd: number
  payoffRatio: number
  totalTokens: number
  conversationCount: number
}

export interface TrendPoint {
  label: string
  timestamp: string
  apiValueUsd: number
  totalTokens: number
}

export interface RateLimitWindowSnapshot {
  usedPercent: number
  remainingPercent: number
  windowDurationMins: number | null
  resetsAt: string | null
  windowStart: string | null
}

export interface RateLimitCreditsSnapshot {
  hasCredits: boolean | null
  unlimited: boolean | null
  balance: string | null
}

export interface LiveRateLimitSnapshot {
  limitId: string | null
  limitName: string | null
  planType: string | null
  credits: RateLimitCreditsSnapshot | null
  rateLimitReachedType: string | null
  primary: RateLimitWindowSnapshot | null
  secondary: RateLimitWindowSnapshot | null
  fetchedAt: string
}

export interface QuotaTrendPoint {
  label: string
  timestamp: string
  apiValueUsd: number
  cumulativeApiValueUsd: number
  totalTokens: number
  cumulativeTokens: number
  remainingPercent: number | null
  usedPercent: number | null
}

export interface ModelShare {
  modelId: string
  displayName: string
  apiValueUsd: number
  totalTokens: number
  conversationCount: number
  color: string
}

export interface CompositionShare {
  category: string
  label: string
  apiValueUsd: number
  totalTokens: number
  color: string
}

export interface SourceShare {
  sourceId: string
  displayName: string
  apiValueUsd: number
  totalTokens: number
  conversationCount: number
  color: string
}

export interface ShareSlice {
  id: string
  label: string
  secondaryLabel?: string | null
  apiValueUsd: number
  totalTokens: number
  color: string
}

export interface OverviewResponse {
  bucket: OverviewBucket
  anchor: string
  windowStart: string
  windowEnd: string
  liveWindowOffset: number
  liveWindowCount: number
  stats: OverviewStats
  trend: TrendPoint[]
  quotaTrend: QuotaTrendPoint[]
  modelShares: ModelShare[]
  compositionShares: CompositionShare[]
  sourceShares: SourceShare[]
  liveRateLimits: LiveRateLimitSnapshot | null
}

export interface DashboardSnapshot {
  overview: OverviewResponse
  conversations: ConversationListItem[]
  codexSources: CodexSource[]
  syncSettings: SyncSettings
  subscriptionProfile: SubscriptionProfile
  subscriptionRecords: SubscriptionRecord[]
  accountStatus: CodexAccountStatus
  liveRateLimits: LiveRateLimitSnapshot | null
}

export interface ConversationFilters {
  bucket?: OverviewBucket | null
  anchor?: string | null
  customStart?: string | null
  customEnd?: string | null
  search?: string | null
  liveWindowOffset?: number | null
  sourceIds?: string[] | null
}

export interface ConversationListItem {
  rootSessionId: string
  title: string
  startedAt: string | null
  updatedAt: string | null
  modelIds: string[]
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
  totalTokens: number
  turnCount: number
  sessionCount: number
  subagentCount: number
  apiValueUsd: number
  subscriptionShare: number
  sourceStates: string[]
  sourceLabels: string[]
}

export interface ConversationSessionSummary {
  sessionId: string
  parentSessionId: string | null
  agentNickname: string | null
  agentRole: string | null
  modelIds: string[]
  startedAt: string | null
  updatedAt: string | null
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
  totalTokens: number
  apiValueUsd: number
  sourceState: string
  sourcePath: string | null
  isSubagent: boolean
}

export interface ConversationTurnPoint {
  sessionId: string
  turnId: string
  startedAt: string | null
  completedAt: string | null
  lastActivityAt: string
  status: string
  userMessage: string | null
  assistantMessage: string | null
  modelIds: string[]
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
  totalTokens: number
  valueUsd: number
}

export interface ConversationDetail {
  rootSessionId: string
  title: string
  startedAt: string | null
  updatedAt: string | null
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
  totalTokens: number
  apiValueUsd: number
  subscriptionShare: number
  multipleAgent: boolean
  sourceStates: string[]
  sessions: ConversationSessionSummary[]
  turns: ConversationTurnPoint[]
  modelBreakdown: ModelShare[]
  compositionBreakdown: CompositionShare[]
  sourceBreakdown: SourceShare[]
}
