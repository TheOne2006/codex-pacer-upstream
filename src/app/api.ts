import { invoke, isTauri } from '@tauri-apps/api/core'

import { todayInputValue } from './format'
import type {
  CodexAccountStatus,
  CodexSource,
  CodexSourceBatchDownloadResult,
  CodexSourceCandidate,
  CodexSourceDownloadResult,
  CodexSourceInput,
  ConversationFilters,
  ConversationListItem,
  LiveRateLimitSnapshot,
  MenuBarPopupSnapshot,
  OverviewBucket,
  OverviewResponse,
  QuotaTrendPoint,
  SubscriptionProfile,
  SubscriptionRecord,
  SubscriptionRecordInput,
  SyncSettings,
} from './types'

function bucketUsesAnchor(bucket: OverviewBucket) {
  return !['five_hour', 'seven_day', 'custom', 'total'].includes(bucket)
}

function nowIso() {
  return new Date().toISOString()
}

function createMockSyncSettings(): SyncSettings {
  return {
    codexHome: null,
    autoScanEnabled: false,
    autoScanIntervalMinutes: 5,
    liveQuotaRefreshIntervalSeconds: 300,
    hideDockIconWhenMenuBarVisible: true,
    showMenuBarLogo: true,
    showMenuBarDailyApiValue: true,
    showMenuBarLiveQuotaPercent: false,
    menuBarLiveQuotaMetric: 'remaining_percent',
    menuBarLiveQuotaBucket: 'five_hour',
    menuBarBucket: 'day',
    menuBarSpeedShowEmoji: true,
    menuBarSpeedFastThresholdPercent: 85,
    menuBarSpeedSlowThresholdPercent: 115,
    menuBarSpeedHealthyEmoji: '🟢',
    menuBarSpeedFastEmoji: '🔥',
    menuBarSpeedSlowEmoji: '🐢',
    menuBarPopupEnabled: true,
    menuBarPopupModules: ['api_value', 'scan_freshness'],
    menuBarPopupShowResetTimeline: true,
    menuBarPopupShowActions: true,
    lastScanStartedAt: null,
    lastScanCompletedAt: null,
    updatedAt: nowIso(),
  }
}

function createMockSubscriptionProfile(): SubscriptionProfile {
  return {
    planType: 'plus',
    currency: 'USD',
    monthlyPrice: 20,
    billingAnchorDay: 1,
    updatedAt: nowIso(),
  }
}

function createMockSubscriptionRecords(): SubscriptionRecord[] {
  return []
}

function createMockCodexSources(): CodexSource[] {
  const now = nowIso()
  return [
    {
      id: 'local',
      kind: 'local',
      label: 'localhost',
      sshAlias: null,
      hostName: null,
      user: null,
      port: null,
      remoteCodexHome: null,
      localCodexHome: '~/.codex',
      selected: true,
      status: 'ready',
      lastDiscoveredAt: null,
      lastDownloadedAt: null,
      lastScannedAt: null,
      lastError: null,
      createdAt: now,
      updatedAt: now,
    },
  ]
}

function createMockCodexAccountStatus(): CodexAccountStatus {
  return {
    available: false,
    requiresOpenaiAuth: false,
    authMode: null,
    accountType: null,
    email: null,
    planType: null,
    error: 'Unavailable in browser preview mode.',
    fetchedAt: nowIso(),
  }
}

function createMockLiveRateLimits(): LiveRateLimitSnapshot {
  return {
    limitId: null,
    limitName: null,
    planType: null,
    primary: null,
    secondary: null,
    fetchedAt: nowIso(),
  }
}

function localDateStartIso(value: string | null | undefined) {
  const fallback = todayInputValue()
  return new Date(`${value ?? fallback}T00:00:00`).toISOString()
}

function localDateExclusiveEndIso(value: string | null | undefined) {
  const fallback = todayInputValue()
  const date = new Date(`${value ?? fallback}T00:00:00`)
  date.setDate(date.getDate() + 1)
  return date.toISOString()
}

function createMockOverview(
  bucket: OverviewBucket,
  anchor?: string | null,
  customStart?: string | null,
  customEnd?: string | null,
): OverviewResponse {
  const timestamp = nowIso()
  const resolvedAnchor =
    bucket === 'custom'
      ? customStart ?? timestamp.slice(0, 10)
      : bucketUsesAnchor(bucket)
        ? anchor ?? timestamp.slice(0, 10)
        : timestamp.slice(0, 10)
  return {
    bucket,
    anchor: resolvedAnchor,
    windowStart: bucket === 'custom' ? localDateStartIso(customStart) : timestamp,
    windowEnd: bucket === 'custom' ? localDateExclusiveEndIso(customEnd ?? customStart) : timestamp,
    liveWindowOffset: 0,
    liveWindowCount: 0,
    stats: {
      apiValueUsd: 0,
      subscriptionCostUsd: 0,
      payoffRatio: 0,
      totalTokens: 0,
      conversationCount: 0,
    },
    trend: [],
    quotaTrend: [],
    modelShares: [],
    compositionShares: [],
    sourceShares: [],
    liveRateLimits: createMockLiveRateLimits(),
  }
}

function createMockQuotaTrend7d(windowStart: string, fetchedAt: string): QuotaTrendPoint[] {
  const startTime = new Date(windowStart).getTime()
  const fetchedTime = new Date(fetchedAt).getTime()
  const step = 24 * 60 * 60 * 1000
  const remaining = [100, 93, 86, 79, 72, 68]

  return remaining.map((remainingPercent, index) => {
    const timestamp = new Date(Math.min(startTime + index * step, fetchedTime)).toISOString()
    const usedPercent = 100 - remainingPercent

    return {
      label: timestamp,
      timestamp,
      apiValueUsd: 0,
      cumulativeApiValueUsd: 0,
      totalTokens: 0,
      cumulativeTokens: 0,
      remainingPercent,
      usedPercent,
    }
  })
}

function createMockMenuBarPopupSnapshot(): MenuBarPopupSnapshot {
  const fetchedAt = nowIso()
  const quota7dWindowStart = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString()
  return {
    fetchedAt,
    refreshIntervalSeconds: 300,
    selectedBucket: 'day',
    quota5h: {
      usedPercent: 58,
      remainingPercent: 42,
      windowDurationMins: 300,
      resetsAt: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString(),
      windowStart: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
    },
    quota7d: {
      usedPercent: 32,
      remainingPercent: 68,
      windowDurationMins: 7 * 24 * 60,
      resetsAt: new Date(Date.now() + 4 * 24 * 60 * 60 * 1000).toISOString(),
      windowStart: quota7dWindowStart,
    },
    quotaTrend7d: createMockQuotaTrend7d(quota7dWindowStart, fetchedAt),
    suggestedSpeed7d: {
      percent: 82,
      displayValue: '82%',
      emoji: '🔥',
      status: 'fast',
      remainingTimePercent: 83,
      remainingPercent: 68,
    },
    speedFastThresholdPercent: 85,
    speedSlowThresholdPercent: 115,
    apiValueSelectedBucket: 14.3,
    totalTokensSelectedBucket: 182_400,
    conversationCountSelectedBucket: 9,
    payoffRatio: 0.71,
    lastScanCompletedAt: fetchedAt,
    liveQuotaFetchedAt: fetchedAt,
    visibleModules: ['api_value', 'scan_freshness'],
    showResetTimeline: true,
    showActions: true,
  }
}

async function invokeOrMock<T>(
  command: string,
  args: Record<string, unknown>,
  mockFactory: () => T | Promise<T>,
): Promise<T> {
  if (isTauri()) {
    return invoke<T>(command, args)
  }
  return mockFactory()
}

export async function scanCodexUsage(codexHome?: string | null): Promise<import('./types').ScanResult> {
  return invokeOrMock('scanCodexUsage', { codexHome: codexHome ?? null }, () => ({
    codexHome: codexHome ?? '~/.codex',
    scannedFiles: 0,
    importedSessions: 0,
    updatedSessions: 0,
    missingSessions: 0,
    lastCompletedAt: nowIso(),
  }))
}

export async function scanCodexSources(sourceIds?: string[] | null): Promise<import('./types').ScanResult[]> {
  return invokeOrMock('scanCodexSources', { sourceIds: sourceIds ?? null }, () => [])
}

export async function getScanInProgress() {
  return invokeOrMock('getScanInProgress', {}, () => false)
}

export async function refreshPricing() {
  return invokeOrMock('refreshPricing', {}, () => [])
}

export async function getOverview(
  bucket: OverviewBucket,
  anchor?: string | null,
  customStart?: string | null,
  customEnd?: string | null,
): Promise<OverviewResponse> {
  return invokeOrMock(
    'getOverview',
    {
      bucket,
      anchor: bucketUsesAnchor(bucket) ? anchor ?? null : null,
      customStart: bucket === 'custom' ? customStart ?? null : null,
      customEnd: bucket === 'custom' ? customEnd ?? null : null,
      liveWindowOffset: null,
      sourceIds: null,
    },
    () => createMockOverview(bucket, anchor, customStart, customEnd),
  )
}

export async function loadDashboard(
  bucket: OverviewBucket,
  anchor?: string | null,
  search?: string | null,
  liveWindowOffset?: number | null,
  sourceIds?: string[] | null,
  customStart?: string | null,
  customEnd?: string | null,
): Promise<import('./types').DashboardSnapshot> {
  return invokeOrMock(
    'loadDashboard',
    {
      bucket,
      anchor: bucketUsesAnchor(bucket) ? anchor ?? null : null,
      customStart: bucket === 'custom' ? customStart ?? null : null,
      customEnd: bucket === 'custom' ? customEnd ?? null : null,
      search: search ?? null,
      liveWindowOffset: liveWindowOffset ?? null,
      sourceIds: sourceIds ?? null,
    },
    () => ({
      overview: createMockOverview(bucket, anchor, customStart, customEnd),
      conversations: [] as ConversationListItem[],
      codexSources: createMockCodexSources(),
      syncSettings: createMockSyncSettings(),
      subscriptionProfile: createMockSubscriptionProfile(),
      subscriptionRecords: createMockSubscriptionRecords(),
      accountStatus: createMockCodexAccountStatus(),
      liveRateLimits: createMockLiveRateLimits(),
    }),
  )
}

export async function listConversations(filters: ConversationFilters) {
  return invokeOrMock('listConversations', { filters }, () => [] satisfies ConversationListItem[])
}

export async function discoverSshCodexSources(): Promise<CodexSourceCandidate[]> {
  return invokeOrMock('discoverSshCodexSources', {}, () => [] satisfies CodexSourceCandidate[])
}

export async function listCodexSources(): Promise<CodexSource[]> {
  return invokeOrMock('listCodexSources', {}, createMockCodexSources)
}

export async function upsertCodexSource(payload: CodexSourceInput): Promise<CodexSource> {
  return invokeOrMock('upsertCodexSource', { payload }, () => ({
    id: payload.sshAlias ? `ssh_${payload.sshAlias}` : `ssh_${Date.now()}`,
    kind: 'ssh',
    label: payload.label,
    sshAlias: payload.sshAlias,
    hostName: payload.hostName,
    user: payload.user,
    port: payload.port,
    remoteCodexHome: payload.remoteCodexHome,
    localCodexHome: null,
    selected: payload.selected,
    status: 'idle',
    lastDiscoveredAt: nowIso(),
    lastDownloadedAt: null,
    lastScannedAt: null,
    lastError: null,
    createdAt: nowIso(),
    updatedAt: nowIso(),
  }))
}

export async function setCodexSourceSelected(sourceId: string, selected: boolean): Promise<CodexSource> {
  return invokeOrMock('setCodexSourceSelected', { sourceId, selected }, () => ({
    ...createMockCodexSources()[0],
    id: sourceId,
    selected,
  }))
}

export async function deleteCodexSource(sourceId: string): Promise<CodexSource[]> {
  return invokeOrMock('deleteCodexSource', { sourceId }, () =>
    createMockCodexSources().filter((source) => source.id !== sourceId),
  )
}

export async function downloadCodexSource(sourceId: string): Promise<CodexSourceDownloadResult> {
  return invokeOrMock('downloadCodexSource', { sourceId }, () => {
    const source = { ...createMockCodexSources()[0], id: sourceId, kind: 'ssh', status: 'ready' }
    return {
      source,
      scanResult: {
        codexHome: source.localCodexHome ?? '',
        scannedFiles: 0,
        importedSessions: 0,
        updatedSessions: 0,
        missingSessions: 0,
        lastCompletedAt: nowIso(),
      },
    }
  })
}

export async function downloadCodexSources(sourceIds: string[]): Promise<CodexSourceBatchDownloadResult> {
  return invokeOrMock('downloadCodexSources', { sourceIds }, () => ({
    results: sourceIds.map((sourceId) => {
      const source = { ...createMockCodexSources()[0], id: sourceId, kind: 'ssh', status: 'ready' }
      return {
        source,
        scanResult: {
          codexHome: source.localCodexHome ?? '',
          scannedFiles: 0,
          importedSessions: 0,
          updatedSessions: 0,
          missingSessions: 0,
          lastCompletedAt: nowIso(),
        },
      }
    }),
    failures: [],
  }))
}

export async function getLiveRateLimits(): Promise<LiveRateLimitSnapshot> {
  return invokeOrMock('getLiveRateLimits', {}, createMockLiveRateLimits)
}

export async function getConversationDetail(
  rootSessionId: string,
): Promise<import('./types').ConversationDetail> {
  return invokeOrMock('getConversationDetail', { rootSessionId }, () => {
    throw new Error(`Conversation ${rootSessionId} is unavailable in browser preview mode.`)
  })
}

export async function getMenuBarPopupSnapshot(forceRefresh = false): Promise<MenuBarPopupSnapshot> {
  return invokeOrMock('getMenuBarPopupSnapshot', { forceRefresh }, createMockMenuBarPopupSnapshot)
}

export async function resizeMenuBarPopup(height: number) {
  return invokeOrMock('resizeMenuBarPopup', { height }, () => true)
}

export type MenuBarPopupAction = 'open_dashboard' | 'open_settings' | 'hide' | 'refresh'

export async function handleMenuBarPopupAction(action: MenuBarPopupAction) {
  return invokeOrMock('handleMenuBarPopupAction', { action }, () => true)
}

export async function getSyncSettings() {
  return invokeOrMock('getSyncSettings', {}, createMockSyncSettings)
}

export async function updateSyncSettings(payload: SyncSettings) {
  return invokeOrMock('updateSyncSettings', { payload }, () => payload)
}

export async function getSubscriptionProfile() {
  return invokeOrMock('getSubscriptionProfile', {}, createMockSubscriptionProfile)
}

export async function updateSubscriptionProfile(payload: SubscriptionProfile) {
  return invokeOrMock('updateSubscriptionProfile', { payload }, () => ({
    ...payload,
    currency: 'USD',
  }))
}

export async function listSubscriptionRecords() {
  return invokeOrMock('listSubscriptionRecords', {}, createMockSubscriptionRecords)
}

export async function createSubscriptionRecord(payload: SubscriptionRecordInput) {
  return invokeOrMock('createSubscriptionRecord', { payload }, () => ({
    ...payload,
    id: -Date.now(),
    createdAt: nowIso(),
    updatedAt: nowIso(),
  }))
}

export async function updateSubscriptionRecord(id: number, payload: SubscriptionRecordInput) {
  return invokeOrMock('updateSubscriptionRecord', { id, payload }, () => ({
    ...payload,
    id,
    createdAt: nowIso(),
    updatedAt: nowIso(),
  }))
}

export async function deleteSubscriptionRecord(id: number) {
  return invokeOrMock('deleteSubscriptionRecord', { id }, () => true)
}

export async function getCodexAccountStatus() {
  return invokeOrMock('getCodexAccountStatus', {}, createMockCodexAccountStatus)
}
