import { startTransition, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import { isTauri } from '@tauri-apps/api/core'
import { emitTo, listen } from '@tauri-apps/api/event'
import {
  BadgeDollarSign,
  ChartNoAxesCombined,
  ChevronLeft,
  ChevronRight,
  Check,
  Clock3,
  RefreshCw,
  Settings2,
  Sparkles,
  ArrowUpDown,
} from 'lucide-react'

import {
  createSubscriptionRecord,
  deleteCodexSource,
  deleteSubscriptionRecord,
  discoverSshCodexSources,
  downloadCodexSources,
  getScanInProgress,
  getConversationDetail,
  getLiveRateLimits,
  listCodexSources,
  listSubscriptionRecords,
  loadDashboard,
  refreshPricing,
  scanCodexSources,
  scanCodexUsage,
  setCodexSourceSelected,
  upsertCodexSource,
  updateSubscriptionRecord,
  updateSyncSettings,
} from './app/api'
import {
  formatCompactDateTime,
  formatDateTime,
  formatRemainingDuration,
  formatPercent,
  formatShortDate,
  formatTokenCount,
  formatUsd,
  todayInputValue,
} from './app/format'
import { useI18n } from './app/useI18n'
import type {
  AppView,
  CodexSource,
  CodexSourceCandidate,
  CodexSourceInput,
  ConversationDetail,
  ConversationListItem,
  LiveRateLimitSnapshot,
  OverviewBucket,
  OverviewResponse,
  ShareDimension,
  ShareMode,
  CodexAccountStatus,
  SubscriptionRecord,
  SubscriptionRecordInput,
  SyncSettings,
} from './app/types'
import { ModelShareChart } from './components/ModelShareChart'
import { QuotaTrendChart } from './components/QuotaTrendChart'
import { SettingsPanel } from './components/SettingsPanel'
import { TrendChart } from './components/TrendChart'
import {
  anchorForBucket,
  bucketUsesAnchor,
  CALENDAR_BUCKETS,
  createCalendarAnchors,
  formatCustomRangeLabel,
  formatCalendarWindowLabel,
  LIVE_QUOTA_BUCKETS,
  monthInputValue,
  normalizeAnchorForBucket,
  shiftAnchorForBucket,
  type CalendarAnchorBucket,
} from './features/dashboard/dateWindows'
import { selectActiveRateLimitWindow } from './features/dashboard/liveQuota'
import { buildQueryKey } from './features/dashboard/queryKey'
import { buildShareSlices, shareChartTitle } from './features/dashboard/shareSlices'
import {
  formatModelSummary,
  formatSessionLabel,
  formatTurnHeadline,
  formatTurnStatus,
} from './features/conversations/turnFormat'
import {
  SidebarSourceManager,
  SourceAddModal,
  SourceDeleteDialog,
  SourceManagerModal,
  SourceSelectorPanel,
  upsertSourceInList,
} from './features/sources'
import { MetricCard, MiniCard } from './shared/ui/MetricCards'
import { isErrorLikeStatus } from './shared/lib/status'

const MENU_BAR_POPUP_WINDOW_LABEL = 'menu-bar-popup'
const MENU_BAR_POPUP_REFRESH_EVENT = 'codex-counter://menu-bar-popup-refresh'
const MENU_BAR_POPUP_LANGUAGE_EVENT = 'codex-counter://language-changed'
const PRODUCT_NAME = 'Codex Pacer'

type StatusMessageKey = 'waitingForFirstScan' | 'fetchingLiveQuotaWindow'
type ConversationSortField = 'value' | 'tokens' | 'updatedAt' | 'startedAt' | 'sessions'
type ConversationSortDirection = 'desc' | 'asc'

const conversationSortFields: ConversationSortField[] = ['value', 'tokens', 'updatedAt', 'startedAt', 'sessions']
const conversationSortDirections: ConversationSortDirection[] = ['desc', 'asc']

function statusMessageForKey(key: StatusMessageKey, t: ReturnType<typeof useI18n>['t']) {
  return key === 'waitingForFirstScan' ? t.status.waitingForFirstScan : t.status.fetchingLiveQuotaWindow
}

function App() {
  const { language, setLanguage, t } = useI18n()
  const [view, setView] = useState<AppView>('overview')
  const [bucket, setBucket] = useState<OverviewBucket>('month')
  const [liveWindowOffset, setLiveWindowOffset] = useState(0)
  const [appLaunchDate] = useState(() => todayInputValue())
  const [calendarAnchors, setCalendarAnchors] = useState<Record<CalendarAnchorBucket, string>>(() =>
    createCalendarAnchors(appLaunchDate),
  )
  const [customStart, setCustomStart] = useState(todayInputValue())
  const [customEnd, setCustomEnd] = useState(todayInputValue())
  const [yearDraft, setYearDraft] = useState(() => new Date().getFullYear().toString())
  const [shareMode, setShareMode] = useState<ShareMode>('value')
  const [shareDimension, setShareDimension] = useState<ShareDimension>('model')
  const [overview, setOverview] = useState<OverviewResponse | null>(null)
  const [conversations, setConversations] = useState<ConversationListItem[]>([])
  const [selectedRootSessionId, setSelectedRootSessionId] = useState<string | null>(null)
  const [detail, setDetail] = useState<ConversationDetail | null>(null)
  const [syncSettings, setSyncSettings] = useState<SyncSettings | null>(null)
  const [codexSources, setCodexSources] = useState<CodexSource[]>([])
  const [sourcePanelOpen, setSourcePanelOpen] = useState(false)
  const [sourceManagerOpen, setSourceManagerOpen] = useState(false)
  const [sourceModalOpen, setSourceModalOpen] = useState(false)
  const [sourceCandidates, setSourceCandidates] = useState<CodexSourceCandidate[]>([])
  const [sourceSelectionMessage, setSourceSelectionMessage] = useState('')
  const [sourceManagerMessage, setSourceManagerMessage] = useState('')
  const [sourceModalMessage, setSourceModalMessage] = useState('')
  const [downloadingSourceIds, setDownloadingSourceIds] = useState<Set<string>>(() => new Set())
  const [deletingSourceIds, setDeletingSourceIds] = useState<Set<string>>(() => new Set())
  const [pendingDeleteSource, setPendingDeleteSource] = useState<CodexSource | null>(null)
  const [subscriptionRecords, setSubscriptionRecords] = useState<SubscriptionRecord[]>([])
  const [accountStatus, setAccountStatus] = useState<CodexAccountStatus | null>(null)
  const [liveRateLimits, setLiveRateLimits] = useState<LiveRateLimitSnapshot | null>(null)
  const [loadedQueryKey, setLoadedQueryKey] = useState<string | null>(null)
  const [dashboardRevision, setDashboardRevision] = useState(0)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [statusMessageKey, setStatusMessageKey] = useState<StatusMessageKey | null>('waitingForFirstScan')
  const [statusMessageText, setStatusMessageText] = useState('')
  const [isBusy, setIsBusy] = useState(false)
  const [search, setSearch] = useState('')
  const [conversationSortField, setConversationSortField] = useState<ConversationSortField>('value')
  const [conversationSortDirection, setConversationSortDirection] =
    useState<ConversationSortDirection>('desc')
  const [conversationSortOpen, setConversationSortOpen] = useState(false)
  const deferredSearch = useDeferredValue(search)
  const syncSettingsRef = useRef<SyncSettings | null>(null)
  const loadShellRef = useRef<(requestScan?: boolean) => Promise<void>>(async () => {})
  const lastRequestedQueryKeyRef = useRef<string | null>(null)
  const latestLoadRequestIdRef = useRef(0)
  const detailCacheRef = useRef(new Map<string, ConversationDetail>())
  const latestDetailRequestIdRef = useRef(0)
  const [hasBootstrapped, setHasBootstrapped] = useState(false)
  const anchor = anchorForBucket(bucket, calendarAnchors)
  const selectedSourceIds = codexSources.filter((source) => source.selected).map((source) => source.id)
  const sourceSelectionKey = selectedSourceIds.join(',')
  const dashboardSourceIds = selectedSourceIds.length > 0 ? selectedSourceIds : null
  const statusMessage = statusMessageKey ? statusMessageForKey(statusMessageKey, t) : statusMessageText
  const setStatusMessage = useCallback((next: string | ((current: string) => string)) => {
    setStatusMessageKey(null)
    setStatusMessageText((current) => (typeof next === 'function' ? next(current) : next))
  }, [])
  const setStatusMessageByKey = useCallback((key: StatusMessageKey) => {
    setStatusMessageKey(key)
    setStatusMessageText('')
  }, [])

  const setCalendarAnchor = useCallback((targetBucket: CalendarAnchorBucket, value: string) => {
    setCalendarAnchors((current) => ({
      ...current,
      [targetBucket]: normalizeAnchorForBucket(targetBucket, value, current[targetBucket]),
    }))
  }, [])

  const shiftCalendarAnchor = useCallback((targetBucket: CalendarAnchorBucket, amount: number) => {
    setCalendarAnchors((current) => ({
      ...current,
      [targetBucket]: shiftAnchorForBucket(targetBucket, current[targetBucket], amount),
    }))
  }, [])

  const waitForScanToSettle = useCallback(async () => {
    const startedAt = Date.now()
    while (Date.now() - startedAt < 15000) {
      if (!(await getScanInProgress())) {
        return
      }
      await new Promise((resolve) => window.setTimeout(resolve, 250))
    }
  }, [])

  useEffect(() => {
    syncSettingsRef.current = syncSettings
  }, [syncSettings])

  useEffect(() => {
    if (bucket === 'year') {
      setYearDraft(calendarAnchors.year.slice(0, 4))
    }
  }, [bucket, calendarAnchors.year])

  const loadShell = useCallback(async (requestScan = false) => {
    const requestId = latestLoadRequestIdRef.current + 1
    latestLoadRequestIdRef.current = requestId
    const requestBucket = bucket
    const requestAnchor = bucketUsesAnchor(requestBucket) ? anchor : null
    const requestCustomStart = requestBucket === 'custom' ? customStart : null
    const requestCustomEnd = requestBucket === 'custom' ? customEnd : null
    const requestSearch = deferredSearch || null
    const requestLiveWindowOffset = requestBucket === 'five_hour' || requestBucket === 'seven_day' ? liveWindowOffset : 0
    lastRequestedQueryKeyRef.current = buildQueryKey(
      requestBucket,
      requestAnchor,
      requestCustomStart,
      requestCustomEnd,
      requestSearch,
      requestLiveWindowOffset,
      sourceSelectionKey,
    )
    setIsBusy(true)
    if ((requestBucket === 'five_hour' || requestBucket === 'seven_day') && requestLiveWindowOffset === 0) {
      setStatusMessageByKey('fetchingLiveQuotaWindow')
    }
    try {
      if (requestScan) {
        try {
          const scans = codexSources.length > 0
            ? await scanCodexSources(dashboardSourceIds)
            : [await scanCodexUsage(syncSettingsRef.current?.codexHome ?? null)]
          const scannedFiles = scans.reduce((sum, scan) => sum + scan.scannedFiles, 0)
          const updatedSessions = scans.reduce((sum, scan) => sum + scan.updatedSessions, 0)
          setStatusMessage(t.status.scannedFiles(scannedFiles, updatedSessions))
        } catch (error) {
          const message = String(error)
          if (!message.includes('already running')) {
            throw error
          }
          setStatusMessage(t.status.backgroundScanAlreadyRunning)
          await waitForScanToSettle()
        }
      }

      const snapshot = await loadDashboard(
        requestBucket,
        requestAnchor,
        requestSearch,
        requestLiveWindowOffset,
        dashboardSourceIds,
        requestCustomStart,
        requestCustomEnd,
      )
      if (requestId !== latestLoadRequestIdRef.current) {
        return
      }

      startTransition(() => {
        const nextDetailCache = new Map<string, ConversationDetail>()
        for (const conversation of snapshot.conversations) {
          const cachedDetail = detailCacheRef.current.get(conversation.rootSessionId)
          if (cachedDetail && cachedDetail.updatedAt === conversation.updatedAt) {
            nextDetailCache.set(conversation.rootSessionId, cachedDetail)
          }
        }

        setOverview(snapshot.overview)
        setConversations(snapshot.conversations)
        setSyncSettings(snapshot.syncSettings)
        setCodexSources(snapshot.codexSources)
        setSubscriptionRecords(snapshot.subscriptionRecords)
        setAccountStatus(snapshot.accountStatus)
        setLiveRateLimits(snapshot.liveRateLimits)
        detailCacheRef.current = nextDetailCache
        setDashboardRevision((current) => current + 1)
        setLiveWindowOffset(snapshot.overview.liveWindowOffset)
        setLoadedQueryKey(
          buildQueryKey(
            requestBucket,
            requestAnchor,
            requestCustomStart,
            requestCustomEnd,
            requestSearch,
            snapshot.overview.liveWindowOffset,
            sourceSelectionKey,
          ),
        )
        setStatusMessage((current) => (isErrorLikeStatus(current) ? '' : current))
        setSelectedRootSessionId((current) =>
          current && snapshot.conversations.some((item) => item.rootSessionId === current)
            ? current
            : snapshot.conversations[0]?.rootSessionId ?? null,
        )
      })
    } catch (error) {
      if (requestId !== latestLoadRequestIdRef.current) {
        return
      }
      startTransition(() => {
        setLoadedQueryKey(null)
      })
      setStatusMessage(t.status.failedToLoad(t.buckets[requestBucket], String(error)))
    } finally {
      if (requestId === latestLoadRequestIdRef.current) {
        setIsBusy(false)
      }
    }
  }, [
    anchor,
    bucket,
    codexSources.length,
    customEnd,
    customStart,
    dashboardSourceIds,
    deferredSearch,
    liveWindowOffset,
    setStatusMessage,
    setStatusMessageByKey,
    sourceSelectionKey,
    t,
    waitForScanToSettle,
  ])

  const currentQueryKey = buildQueryKey(
    bucket,
    bucketUsesAnchor(bucket) ? anchor : null,
    bucket === 'custom' ? customStart : null,
    bucket === 'custom' ? customEnd : null,
    deferredSearch || null,
    bucket === 'five_hour' || bucket === 'seven_day' ? liveWindowOffset : 0,
    sourceSelectionKey,
  )

  useEffect(() => {
    loadShellRef.current = loadShell
  }, [loadShell])

  useEffect(() => {
    if (!isTauri()) return

    let dispose: (() => void) | undefined
    void listen('codex-counter://open-settings', () => {
      setSettingsOpen(true)
    }).then((unlisten) => {
      dispose = unlisten
    })

    return () => {
      dispose?.()
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    void emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_LANGUAGE_EVENT, { language }).catch(() => {})
  }, [language])

  const loadDetail = useCallback(async (rootSessionId: string | null) => {
    const requestId = latestDetailRequestIdRef.current + 1
    latestDetailRequestIdRef.current = requestId
    if (!rootSessionId) {
      setDetail(null)
      return
    }

    const cachedDetail = detailCacheRef.current.get(rootSessionId)
    if (cachedDetail) {
      setDetail(cachedDetail)
      return
    }

    setDetail(null)
    try {
      const nextDetail = await getConversationDetail(rootSessionId)
      if (requestId !== latestDetailRequestIdRef.current) {
        return
      }
      detailCacheRef.current.set(rootSessionId, nextDetail)
      startTransition(() => {
        setDetail(nextDetail)
      })
    } catch (error) {
      if (requestId !== latestDetailRequestIdRef.current) {
        return
      }
      setStatusMessage(String(error))
    }
  }, [setStatusMessage])

  useEffect(() => {
    let cancelled = false

    const bootstrap = async () => {
      await loadShellRef.current(true)
      if (!cancelled) {
        setHasBootstrapped(true)
      }
    }

    void bootstrap()

    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!hasBootstrapped) return
    if (lastRequestedQueryKeyRef.current === currentQueryKey) return
    void loadShell(false)
  }, [currentQueryKey, hasBootstrapped, loadShell])

  useEffect(() => {
    if (!loadedQueryKey && selectedRootSessionId) return
    void loadDetail(selectedRootSessionId)
  }, [dashboardRevision, loadDetail, loadedQueryKey, selectedRootSessionId])

  useEffect(() => {
    if (!hasBootstrapped) return
    const refreshMs =
      bucket === 'five_hour' || bucket === 'seven_day'
        ? (syncSettings?.liveQuotaRefreshIntervalSeconds ?? 300) * 1000
        : 60000
    const interval = window.setInterval(() => {
      void loadShell(false)
    }, Math.max(5000, refreshMs))
    return () => window.clearInterval(interval)
  }, [bucket, hasBootstrapped, loadShell, syncSettings?.liveQuotaRefreshIntervalSeconds])

  useEffect(() => {
    if (!settingsOpen) return
    let cancelled = false
    const refresh = () =>
      void getLiveRateLimits()
        .then((snapshot) => {
          if (!cancelled) {
            setLiveRateLimits(snapshot)
          }
        })
        .catch(() => {})
    refresh()
    const interval = window.setInterval(
      refresh,
      Math.max(60000, (syncSettings?.liveQuotaRefreshIntervalSeconds ?? 300) * 1000),
    )
    return () => {
      cancelled = true
      window.clearInterval(interval)
    }
  }, [settingsOpen, syncSettings?.liveQuotaRefreshIntervalSeconds])

  async function handleRescan() {
    await loadShell(true)
  }

  async function handleRefreshPricing() {
    setIsBusy(true)
    try {
      detailCacheRef.current.clear()
      setDetail(null)
      try {
        await refreshPricing()
      } catch (error) {
        const message = String(error)
        if (!message.includes('already running')) {
          throw error
        }
        setStatusMessage(t.status.backgroundScanAlreadyRunning)
        await waitForScanToSettle()
        await refreshPricing()
      }
      detailCacheRef.current.clear()
      await loadShell(false)
      if (isTauri()) {
        await emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_REFRESH_EVENT, {}).catch(() => {})
      }
      setStatusMessage(t.status.pricingRefreshed)
    } catch (error) {
      setStatusMessage(String(error))
    } finally {
      setIsBusy(false)
    }
  }

  async function refreshCodexSources() {
    try {
      const nextSources = await listCodexSources()
      setCodexSources(nextSources)
    } catch (error) {
      setSourceManagerMessage(String(error))
    }
  }

  async function handleToggleSource(source: CodexSource, selected: boolean) {
    if (!selected && codexSources.filter((item) => item.selected).length <= 1) {
      setSourceSelectionMessage(t.sources.keepOneSelected)
      return
    }
    try {
      const updated = await setCodexSourceSelected(source.id, selected)
      setCodexSources((current) => current.map((item) => (item.id === updated.id ? updated : item)))
      setSourceSelectionMessage('')
      detailCacheRef.current.clear()
      await loadShell(false)
    } catch (error) {
      setSourceSelectionMessage(String(error))
    }
  }

  async function handleOpenSourceModal() {
    setSourceModalOpen(true)
    setSourceModalMessage('')
    try {
      const [candidates, sources] = await Promise.all([discoverSshCodexSources(), listCodexSources()])
      setSourceCandidates(candidates)
      setCodexSources(sources)
    } catch (error) {
      setSourceModalMessage(String(error))
    }
  }

  async function handleDiscoverSources() {
    setSourceModalMessage('')
    try {
      setSourceCandidates(await discoverSshCodexSources())
    } catch (error) {
      setSourceModalMessage(String(error))
    }
  }

  async function handleAddSource(candidate: CodexSourceCandidate) {
    const payload: CodexSourceInput = {
      label: candidate.label,
      sshAlias: candidate.sshAlias,
      hostName: candidate.hostName,
      user: candidate.user,
      port: candidate.port,
      remoteCodexHome: candidate.remoteCodexHome || '~/.codex',
      selected: true,
    }
    try {
      const saved = await upsertCodexSource(payload)
      setCodexSources((current) => upsertSourceInList(current, saved))
      setSourceModalMessage(t.sources.addedSource(saved.label))
    } catch (error) {
      setSourceModalMessage(String(error))
    }
  }

  async function handleDownloadSources(sourceIds: string[]) {
    const uniqueSourceIds = Array.from(new Set(sourceIds))
    if (uniqueSourceIds.length === 0) {
      return
    }
    setDownloadingSourceIds((current) => {
      const next = new Set(current)
      uniqueSourceIds.forEach((sourceId) => next.add(sourceId))
      return next
    })
    setSourceManagerMessage(t.sources.downloadingRemoteCache)
    try {
      const result = await downloadCodexSources(uniqueSourceIds)
      setCodexSources((current) =>
        result.results.reduce((nextSources, item) => upsertSourceInList(nextSources, item.source), current),
      )
      const scannedFiles = result.results.reduce((sum, item) => sum + item.scanResult.scannedFiles, 0)
      if (result.results.length > 0) {
        detailCacheRef.current.clear()
        await loadShell(false)
      } else {
        await refreshCodexSources()
      }
      const messages = []
      if (result.results.length === 1 && uniqueSourceIds.length === 1) {
        messages.push(t.sources.downloadedAndImported(scannedFiles))
      } else if (result.results.length > 0) {
        messages.push(t.sources.selectedRemoteSourcesSynced(result.results.length, scannedFiles))
      }
      if (result.failures.length > 0) {
        messages.push(t.sources.remoteSourcesSyncFailed(result.failures.length))
      }
      setSourceManagerMessage(messages.join('；') || t.sources.remoteSourcesSyncFailed(uniqueSourceIds.length))
    } catch (error) {
      setSourceManagerMessage(String(error))
      await refreshCodexSources()
    } finally {
      setDownloadingSourceIds((current) => {
        const next = new Set(current)
        uniqueSourceIds.forEach((sourceId) => next.delete(sourceId))
        return next
      })
    }
  }

  async function handleDownloadSource(sourceId: string) {
    await handleDownloadSources([sourceId])
  }

  async function handleDownloadAllSources() {
    const remoteSources = codexSources.filter((source) => source.kind === 'ssh')
    if (remoteSources.length === 0) {
      setSourceManagerMessage(t.sources.noSshServersAdded)
      await handleOpenSourceModal()
      return
    }

    await handleDownloadSources(remoteSources.map((source) => source.id))
  }

  async function handleDownloadSelectedSources() {
    const selectedRemoteSources = codexSources.filter((source) => source.kind === 'ssh' && source.selected)
    if (selectedRemoteSources.length === 0) {
      setSourceManagerMessage(t.sources.noSelectedSshServers)
      return
    }

    await handleDownloadSources(selectedRemoteSources.map((source) => source.id))
  }

  async function handleDeleteSource(source: CodexSource) {
    if (source.kind !== 'ssh') {
      return
    }
    if (source.selected && codexSources.filter((item) => item.selected && item.id !== source.id).length === 0) {
      setSourceSelectionMessage(t.sources.keepOneSelected)
      setSourcePanelOpen(true)
      return
    }
    setSourceManagerMessage('')
    setPendingDeleteSource(source)
  }

  async function confirmDeleteSource() {
    const source = pendingDeleteSource
    if (!source || source.kind !== 'ssh') return

    setDeletingSourceIds((current) => new Set(current).add(source.id))
    setSourceManagerMessage(t.sources.deletingSource(source.label))
    try {
      const nextSources = await deleteCodexSource(source.id)
      setCodexSources(nextSources)
      detailCacheRef.current.clear()
      setSourceSelectionMessage('')
      setPendingDeleteSource(null)
      await loadShell(false)
      if (isTauri()) {
        await emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_REFRESH_EVENT, {}).catch(() => {})
      }
      setSourceManagerMessage(t.sources.deletedSource(source.label))
    } catch (error) {
      setSourceManagerMessage(String(error))
      await refreshCodexSources()
    } finally {
      setDeletingSourceIds((current) => {
        const next = new Set(current)
        next.delete(source.id)
        return next
      })
    }
  }

  async function handleSaveSettings(payload: {
    syncSettings: SyncSettings
  }) {
    const nextSyncSettings = await updateSyncSettings(payload.syncSettings)
    setSyncSettings(nextSyncSettings)
    await loadShell(false)
    if (isTauri()) {
      await emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_REFRESH_EVENT, {}).catch(() => {})
    }
    setStatusMessage(t.status.settingsSaved)
  }

  async function handleSaveSubscriptionRecord(payload: SubscriptionRecordInput, id?: number | null) {
    if (id && id > 0) {
      await updateSubscriptionRecord(id, payload)
    } else {
      await createSubscriptionRecord(payload)
    }
    const nextSubscriptionRecords = await listSubscriptionRecords()
    setSubscriptionRecords(nextSubscriptionRecords)
    await loadShell(false)
    if (isTauri()) {
      await emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_REFRESH_EVENT, {}).catch(() => {})
    }
    setStatusMessage(t.status.subscriptionRecordSaved)
  }

  async function handleDeleteSubscriptionRecord(id: number) {
    await deleteSubscriptionRecord(id)
    const nextSubscriptionRecords = await listSubscriptionRecords()
    setSubscriptionRecords(nextSubscriptionRecords)
    await loadShell(false)
    if (isTauri()) {
      await emitTo(MENU_BAR_POPUP_WINDOW_LABEL, MENU_BAR_POPUP_REFRESH_EVENT, {}).catch(() => {})
    }
    setStatusMessage(t.status.subscriptionRecordDeleted)
  }

  const snapshotIsCurrent = loadedQueryKey === currentQueryKey
  const activeOverview = snapshotIsCurrent ? overview : null
  const activeConversations = snapshotIsCurrent ? conversations : []
  const sortedActiveConversations = useMemo(() => {
    const valueFor = (conversation: ConversationListItem) => {
      switch (conversationSortField) {
        case 'tokens':
          return conversation.totalTokens
        case 'updatedAt': {
          const updatedAt = conversation.updatedAt ? Date.parse(conversation.updatedAt) : 0
          return Number.isFinite(updatedAt) ? updatedAt : 0
        }
        case 'startedAt': {
          const startedAt = conversation.startedAt ? Date.parse(conversation.startedAt) : 0
          return Number.isFinite(startedAt) ? startedAt : 0
        }
        case 'sessions':
          return conversation.sessionCount
        case 'value':
        default:
          return conversation.apiValueUsd
      }
    }

    return [...activeConversations].sort((left, right) => {
      const leftValue = valueFor(left)
      const rightValue = valueFor(right)
      const primary =
        conversationSortDirection === 'desc' ? rightValue - leftValue : leftValue - rightValue

      if (primary !== 0) {
        return primary
      }

      const parsedLeftUpdatedAt = left.updatedAt ? Date.parse(left.updatedAt) : 0
      const parsedRightUpdatedAt = right.updatedAt ? Date.parse(right.updatedAt) : 0
      const leftUpdatedAt = Number.isFinite(parsedLeftUpdatedAt) ? parsedLeftUpdatedAt : 0
      const rightUpdatedAt = Number.isFinite(parsedRightUpdatedAt) ? parsedRightUpdatedAt : 0
      return rightUpdatedAt - leftUpdatedAt || left.rootSessionId.localeCompare(right.rootSessionId)
    })
  }, [activeConversations, conversationSortDirection, conversationSortField])
  const conversationSortDirectionLabel =
    conversationSortField === 'updatedAt' || conversationSortField === 'startedAt'
      ? t.conversationList.sortDirections.time[conversationSortDirection]
      : conversationSortField === 'sessions'
        ? t.conversationList.sortDirections.count[conversationSortDirection]
        : t.conversationList.sortDirections.numeric[conversationSortDirection]
  const conversationSortSummary = `${t.conversationList.sortFields[conversationSortField]} · ${conversationSortDirectionLabel}`
  const isLiveBucket = bucket === 'five_hour' || bucket === 'seven_day'
  const activeLiveWindowOffset = activeOverview?.liveWindowOffset ?? liveWindowOffset
  const activeLiveWindowCount = activeOverview?.liveWindowCount ?? (isLiveBucket ? 1 : 0)
  const isHistoricalLiveWindow = isLiveBucket && activeLiveWindowOffset > 0
  const calendarWindowLabel = formatCalendarWindowLabel(bucket, anchor, language)
  const currentBucketLabel =
    activeOverview?.bucket === 'custom'
      ? formatCustomRangeLabel(activeOverview.windowStart, activeOverview.windowEnd, language)
      : bucket === 'five_hour'
          ? isHistoricalLiveWindow
            ? t.bucketDescriptions.historicalFiveHourWindow
            : t.bucketDescriptions.currentFiveHourWindow
          : bucket === 'seven_day'
            ? isHistoricalLiveWindow
              ? t.bucketDescriptions.historicalSevenDayWindow
              : t.bucketDescriptions.currentSevenDayWindow
            : calendarWindowLabel ?? t.buckets[bucket]
  const shouldShowStatusNotice = isErrorLikeStatus(statusMessage)
  const shouldShowLoadingNotice =
    isBusy && !shouldShowStatusNotice && isLiveBucket && liveWindowOffset === 0
  const resolvedBucket = bucket
  const resolvedLiveRateLimits = activeOverview?.liveRateLimits ?? liveRateLimits
  const activeRateLimitWindow = selectActiveRateLimitWindow(resolvedLiveRateLimits, resolvedBucket)
  const overviewShareData = buildShareSlices(
    shareDimension,
    activeOverview?.modelShares ?? [],
    activeOverview?.compositionShares ?? [],
    activeOverview?.sourceShares ?? [],
  )
  const activeDetail = snapshotIsCurrent ? detail : null
  const detailShareData = buildShareSlices(
    shareDimension,
    activeDetail?.modelBreakdown ?? [],
    activeDetail?.compositionBreakdown ?? [],
    activeDetail?.sourceBreakdown ?? [],
  )
  const sessionSummariesById = new Map(
    (activeDetail?.sessions ?? []).map((session) => [session.sessionId, session] as const),
  )
  const latestQuotaPoint = [...(activeOverview?.quotaTrend ?? [])]
    .reverse()
    .find((point) => point.remainingPercent !== null)
  const currentRemainingPercent = activeRateLimitWindow?.remainingPercent ?? latestQuotaPoint?.remainingPercent ?? 0
  const currentUsedPercent = activeRateLimitWindow?.usedPercent ?? latestQuotaPoint?.usedPercent ?? 0
  const wastedPercent = Math.max(0, 100 - Math.max(...(activeOverview?.quotaTrend ?? []).map((point) => point.usedPercent ?? 0), 0))
  const liveWindowRangeLabel =
    activeOverview && isLiveBucket
      ? `${formatCompactDateTime(activeOverview.windowStart, language)} → ${formatCompactDateTime(activeOverview.windowEnd, language)}`
      : null
  const anchorInputLabel = t.common.bucketAnchor(t.buckets[bucket])

  return (
    <div className="app-shell">
      <div className="app-frame">
        <aside className="sidebar">
          <div className="brand">
            <h1>{PRODUCT_NAME}</h1>
          </div>

          <div className="nav-stack">
            <button
              className={`nav-button ${view === 'overview' ? 'active' : ''}`}
              onClick={() => setView('overview')}
              type="button"
            >
              <ChartNoAxesCombined size={18} /> {t.nav.overview}
            </button>
            <button
              className={`nav-button ${view === 'conversations' ? 'active' : ''}`}
              onClick={() => setView('conversations')}
              type="button"
            >
              <Sparkles size={18} /> {t.nav.conversations}
            </button>
            <SidebarSourceManager
              onOpenManager={() => setSourceManagerOpen(true)}
            />
          </div>

          <div className="sidebar-footer">
            <div className="status-panel sidebar-status-panel">
              <span className="eyebrow">{t.common.autoScan}</span>
              <strong>
                {syncSettings?.autoScanEnabled
                  ? t.common.everyMinutes(syncSettings.autoScanIntervalMinutes)
                  : t.common.disabled}
              </strong>
              <Clock3 size={18} />
            </div>

            <div className="sidebar-utility-row" aria-label={t.sources.maintenanceActions}>
              <button
                className="ghost-button sidebar-utility-button sidebar-utility-button--scan"
                disabled={isBusy}
                onClick={handleRescan}
                title={t.actions.rescanNow}
                type="button"
              >
                <RefreshCw size={15} />
                <span>{t.actions.rescanNow}</span>
              </button>
              <button
                className="ghost-button sidebar-utility-button sidebar-utility-button--pricing"
                disabled={isBusy}
                onClick={handleRefreshPricing}
                title={t.actions.refreshPricing}
                type="button"
              >
                <BadgeDollarSign size={15} />
                <span>{t.actions.refreshPricing}</span>
              </button>
            </div>

            <button className="ghost-button" onClick={() => setSettingsOpen(true)} type="button">
              <Settings2 size={16} /> {t.actions.settings}
            </button>
          </div>
        </aside>

        <main className="main-panel">
          <section className="hero-panel hero-panel-filters hero-panel-filters--controls-only">
            <div className="hero-filter-region hero-filter-region--standalone">
              <div className="hero-filter-controls">
                <div className="hero-filter-left">
	                  <SourceSelectorPanel
	                    isOpen={sourcePanelOpen}
	                    message={sourceSelectionMessage}
	                    onToggleOpen={() => setSourcePanelOpen((current) => !current)}
	                    onToggleSource={handleToggleSource}
	                    sources={codexSources}
	                  />
                  <div className="time-filter-panel">
                  <div className="time-filter-group">
                    <span className="time-filter-label">{t.common.timeRange}</span>
                    <div className="pill-strip pill-strip--calendar" role="group" aria-label={t.common.timeRange}>
                      {CALENDAR_BUCKETS.map((option) => (
                        <button
                          key={option}
                          className={option === bucket ? 'active' : ''}
                          onClick={() => {
                            setBucket(option)
                            setLiveWindowOffset(0)
                          }}
                          type="button"
                        >
                          {t.buckets[option]}
                        </button>
                      ))}
                    </div>
                  </div>
                  <div className="time-filter-group">
                    <span className="time-filter-label">{t.charts.liveQuotaEyebrow}</span>
                    <div className="pill-strip pill-strip--live" role="group" aria-label={t.charts.liveQuotaEyebrow}>
                      {LIVE_QUOTA_BUCKETS.map((option) => (
                        <button
                          key={option}
                          className={option === bucket ? 'active' : ''}
                          onClick={() => {
                            setBucket(option)
                            setLiveWindowOffset(0)
                          }}
                          type="button"
                        >
                          {t.buckets[option]}
                        </button>
                      ))}
                    </div>
                  </div>
                  </div>
                </div>
                {bucket === 'custom' ? (
                  <div className="custom-range-controls">
                    <label className="custom-range-field">
                      <span>{t.common.start}</span>
                      <input
                        aria-label={t.common.customRangeStartDate}
                        className="anchor-input custom-range-input"
                        max={customEnd}
                        name="customRangeStartDate"
                        onChange={(event) => setCustomStart(event.target.value)}
                        type="date"
                        value={customStart}
                      />
                    </label>
                    <span aria-hidden="true" className="range-separator">
                      →
                    </span>
                    <label className="custom-range-field">
                      <span>{t.common.end}</span>
                      <input
                        aria-label={t.common.customRangeEndDate}
                        className="anchor-input custom-range-input"
                        min={customStart}
                        name="customRangeEndDate"
                        onChange={(event) => setCustomEnd(event.target.value)}
                        type="date"
                        value={customEnd}
                      />
                    </label>
                  </div>
                ) : bucket === 'day' ? (
                  <div className="period-nav period-nav--editable">
                    <button
                      aria-label={t.common.earlier}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('day', -1)}
                      title={t.common.earlier}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" size={16} />
                    </button>
                    <input
                      aria-label={anchorInputLabel}
                      className="anchor-input anchor-input-inline period-nav-input"
                      onChange={(event) => setCalendarAnchor('day', event.target.value)}
                      name="bucketAnchorDate"
                      type="date"
                      value={calendarAnchors.day}
                    />
                    <button
                      aria-label={t.common.newer}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('day', 1)}
                      title={t.common.newer}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" size={16} />
                    </button>
                  </div>
                ) : bucket === 'week' ? (
                  <div className="period-nav period-nav--editable period-nav--week">
                    <button
                      aria-label={t.common.earlier}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('week', -1)}
                      title={t.common.earlier}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" size={16} />
                    </button>
                    <div className="period-nav-copy period-nav-copy--week-range">
                      <strong>{calendarWindowLabel}</strong>
                    </div>
                    <button
                      aria-label={t.common.newer}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('week', 1)}
                      title={t.common.newer}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" size={16} />
                    </button>
                  </div>
                ) : bucket === 'month' ? (
                  <div className="period-nav period-nav--editable">
                    <button
                      aria-label={t.common.earlier}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('month', -1)}
                      title={t.common.earlier}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" size={16} />
                    </button>
                    <input
                      aria-label={anchorInputLabel}
                      className="anchor-input anchor-input-inline anchor-input-month period-nav-input"
                      onChange={(event) => setCalendarAnchor('month', `${event.target.value}-01`)}
                      name="bucketAnchorMonth"
                      type="month"
                      value={monthInputValue(calendarAnchors.month)}
                    />
                    <button
                      aria-label={t.common.newer}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('month', 1)}
                      title={t.common.newer}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" size={16} />
                    </button>
                  </div>
                ) : bucket === 'year' ? (
                  <div className="period-nav period-nav--editable">
                    <button
                      aria-label={t.common.earlier}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('year', -1)}
                      title={t.common.earlier}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" size={16} />
                    </button>
                    <input
                      aria-label={anchorInputLabel}
                      className="anchor-input anchor-input-inline anchor-input-year period-nav-input"
                      inputMode="numeric"
                      name="bucketAnchorYear"
                      onBlur={() => setYearDraft(calendarAnchors.year.slice(0, 4))}
                      onChange={(event) => {
                        const nextDraft = event.target.value.replace(/\D/g, '').slice(0, 4)
                        setYearDraft(nextDraft)
                        if (nextDraft.length === 4) {
                          setCalendarAnchor('year', `${nextDraft}-01-01`)
                        }
                      }}
                      pattern="[0-9]*"
                      type="text"
                      value={yearDraft}
                    />
                    <button
                      aria-label={t.common.newer}
                      className="ghost-button live-window-nav-button"
                      onClick={() => shiftCalendarAnchor('year', 1)}
                      title={t.common.newer}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" size={16} />
                    </button>
                  </div>
                ) : isLiveBucket ? (
                  <div className="live-window-nav">
                    <button
                      aria-label={t.common.earlier}
                      className="ghost-button live-window-nav-button"
                      disabled={isBusy || activeLiveWindowOffset >= Math.max(activeLiveWindowCount - 1, 0)}
                      onClick={() => setLiveWindowOffset((current) => current + 1)}
                      title={t.common.earlier}
                      type="button"
                    >
                      <ChevronLeft aria-hidden="true" size={16} />
                    </button>
                    <div className="live-window-nav-copy">
                      <strong>{liveWindowRangeLabel ?? currentBucketLabel}</strong>
                    </div>
                    <button
                      aria-label={t.common.newer}
                      className="ghost-button live-window-nav-button"
                      disabled={isBusy || activeLiveWindowOffset <= 0}
                      onClick={() => setLiveWindowOffset((current) => Math.max(0, current - 1))}
                      title={t.common.newer}
                      type="button"
                    >
                      <ChevronRight aria-hidden="true" size={16} />
                    </button>
                  </div>
                ) : null}
              </div>
            </div>
          </section>

          {shouldShowLoadingNotice ? (
            <div aria-live="polite" className="status-inline status-inline-muted" role="status">
              {t.status.fetchingLiveQuotaWindow}
            </div>
          ) : null}
          {shouldShowStatusNotice ? (
            <div aria-live="polite" className="status-inline" role="status">
              {statusMessage}
            </div>
          ) : null}

          {view === 'overview' ? (
            <>
              <section className="metric-grid">
                {isLiveBucket ? (
                  <>
                    <MetricCard
                      caption={t.metrics.apiValue}
                      featured
                      tone="primary"
                      value={formatUsd(activeOverview?.stats.apiValueUsd ?? 0, language)}
                      note={currentBucketLabel}
                    />
                    <MetricCard
                      caption={isHistoricalLiveWindow ? t.metrics.wastedQuota : t.metrics.remainingQuota}
                      tone="secondary"
                      value={formatPercent(
                        (isHistoricalLiveWindow ? wastedPercent : currentRemainingPercent) / 100,
                        language,
                      )}
                      note={
                        isHistoricalLiveWindow
                          ? t.metrics.peakUsed(formatPercent((100 - wastedPercent) / 100, language))
                          : latestQuotaPoint
                            ? t.metrics.used(formatPercent(currentUsedPercent / 100, language))
                            : t.status.waitingLiveQuota
                      }
                    />
                    <MetricCard
                      caption={isHistoricalLiveWindow ? t.metrics.windowStart : t.metrics.remainingTime}
                      tone="accent"
                      value={
                        isHistoricalLiveWindow
                          ? formatCompactDateTime(activeOverview?.windowStart ?? null, language)
                          : formatRemainingDuration(activeOverview?.windowEnd ?? null, language)
                      }
                      compact
                      note={
                        isHistoricalLiveWindow
                          ? `${t.common.reset} ${formatCompactDateTime(activeOverview?.windowEnd ?? null, language)}`
                          : activeOverview?.windowEnd
                            ? `${t.common.reset} ${formatCompactDateTime(activeOverview.windowEnd, language)}`
                            : undefined
                      }
                    />
                    <MetricCard
                      caption={t.metrics.totalTokens}
                      tone="neutral"
                      value={formatTokenCount(activeOverview?.stats.totalTokens ?? 0, language)}
                    />
                    <MetricCard
                      caption={t.metrics.conversationCount}
                      tone="neutral"
                      value={String(activeOverview?.stats.conversationCount ?? 0)}
                    />
                  </>
                ) : (
                  <>
                    <MetricCard
                      caption={t.metrics.apiValue}
                      featured
                      tone="primary"
                      value={formatUsd(activeOverview?.stats.apiValueUsd ?? 0, language)}
                      note={currentBucketLabel}
                    />
                    <MetricCard
                      caption={t.metrics.subscriptionCost}
                      tone="secondary"
                      value={formatUsd(activeOverview?.stats.subscriptionCostUsd ?? 0, language)}
                      note={
                        subscriptionRecords.length > 0
                          ? t.metrics.subscriptionLedgerNote(subscriptionRecords.length)
                          : t.metrics.noSubscriptionRecords
                      }
                    />
                    <MetricCard
                      caption={t.metrics.payoffRatio}
                      tone="accent"
                      value={formatPercent(activeOverview?.stats.payoffRatio ?? 0, language)}
                    />
                    <MetricCard
                      caption={t.metrics.totalTokens}
                      tone="neutral"
                      value={formatTokenCount(activeOverview?.stats.totalTokens ?? 0, language)}
                    />
                    <MetricCard
                      caption={t.metrics.conversationCount}
                      tone="neutral"
                      value={String(activeOverview?.stats.conversationCount ?? 0)}
                    />
                  </>
                )}
              </section>

              <section className="overview-grid">
                {isLiveBucket ? (
                  <QuotaTrendChart
                    bucket={resolvedBucket}
                    data={activeOverview?.quotaTrend ?? []}
                    liveRateLimits={resolvedLiveRateLimits}
                    windowStart={activeOverview?.windowStart ?? new Date().toISOString()}
                    windowEnd={activeOverview?.windowEnd ?? new Date().toISOString()}
                    isHistorical={isHistoricalLiveWindow}
                  />
                ) : (
                  <TrendChart data={activeOverview?.trend ?? []} />
                )}
                <ModelShareChart
                  data={overviewShareData}
                  mode={shareMode}
                  onModeChange={setShareMode}
                  dimension={shareDimension}
                  onDimensionChange={setShareDimension}
                  title={shareChartTitle(
                    shareDimension,
                    t.overview.modelShare,
                    t.overview.costStructure,
                    t.charts.sourceDistribution,
                  )}
                  eyebrow={t.overview.distribution}
                />
              </section>
            </>
          ) : (
            <section className="conversation-layout">
              <aside className="list-panel">
                <div className="list-toolbar">
                  <div className="list-toolbar-head">
                    <div className="list-toolbar-copy">
                      <p className="eyebrow">{t.conversationList.eyebrow}</p>
                      <h3>{t.conversationList.title}</h3>
                    </div>
                    <div className="list-toolbar-actions">
                      <div className="conversation-sort">
                        <button
                          aria-expanded={conversationSortOpen}
                          aria-label={t.conversationList.sort}
                          className="conversation-sort-button"
                          onClick={() => setConversationSortOpen((open) => !open)}
                          type="button"
                        >
                          <ArrowUpDown aria-hidden="true" size={15} />
                          <span>{conversationSortSummary}</span>
                        </button>
                        {conversationSortOpen ? (
                          <div className="conversation-sort-menu">
                            <p className="conversation-sort-heading">{t.conversationList.sortField}</p>
                            {conversationSortFields.map((field) => (
                              <button
                                className={`conversation-sort-option ${
                                  conversationSortField === field ? 'active' : ''
                                }`}
                                key={field}
                                onClick={() => setConversationSortField(field)}
                                type="button"
                              >
                                <span>{t.conversationList.sortFields[field]}</span>
                                {conversationSortField === field ? <Check aria-hidden="true" size={14} /> : null}
                              </button>
                            ))}
                            <div className="conversation-sort-divider" />
                            <p className="conversation-sort-heading">{t.conversationList.sortDirection}</p>
                            {conversationSortDirections.map((direction) => (
                              <button
                                className={`conversation-sort-option ${
                                  conversationSortDirection === direction ? 'active' : ''
                                }`}
                                key={direction}
                                onClick={() => setConversationSortDirection(direction)}
                                type="button"
                              >
                                <span>
                                  {conversationSortField === 'updatedAt' || conversationSortField === 'startedAt'
                                    ? t.conversationList.sortDirections.time[direction]
                                    : conversationSortField === 'sessions'
                                      ? t.conversationList.sortDirections.count[direction]
                                      : t.conversationList.sortDirections.numeric[direction]}
                                </span>
                                {conversationSortDirection === direction ? (
                                  <Check aria-hidden="true" size={14} />
                                ) : null}
                              </button>
                            ))}
                          </div>
                        ) : null}
                      </div>
                      <span className="detail-micro list-toolbar-count">
                        {t.conversationList.shown(sortedActiveConversations.length)}
                      </span>
                    </div>
                  </div>
                  <input
                    aria-label={t.common.searchTitleOrSession}
                    className="search-input"
                    name="conversationSearch"
                    onChange={(event) => setSearch(event.target.value)}
                    placeholder={t.common.searchTitleOrSession}
                    value={search}
                  />
                </div>

                <div className="conversation-list">
                  {sortedActiveConversations.length === 0 ? (
                    <div className="empty-state">{t.conversationList.empty}</div>
                  ) : (
                    sortedActiveConversations.map((conversation) => (
                      <button
                        key={conversation.rootSessionId}
                        className={`conversation-card ${
                          conversation.rootSessionId === selectedRootSessionId ? 'active' : ''
                        }`}
                        onClick={() => setSelectedRootSessionId(conversation.rootSessionId)}
                        type="button"
                        >
                        <div className="card-topline">
                          <span className="card-id">{conversation.rootSessionId.slice(0, 12)}</span>
                          <span>{formatUsd(conversation.apiValueUsd, language)}</span>
                        </div>
                        <h4>{conversation.title}</h4>
                        <div className="badge-row">
                          {conversation.modelIds.map((modelId) => (
                            <span className="model-chip" key={modelId}>
                              {modelId}
                            </span>
                          ))}
                        </div>
                        {conversation.sourceLabels.length > 0 ? (
                          <div className="source-chip-row">
                            {conversation.sourceLabels.slice(0, 2).map((sourceLabel) => (
                              <span className="source-chip" key={sourceLabel} title={sourceLabel}>
                                {sourceLabel}
                              </span>
                            ))}
                            {conversation.sourceLabels.length > 2 ? (
                              <span className="source-chip source-chip--more">
                                +{conversation.sourceLabels.length - 2}
                              </span>
                            ) : null}
                          </div>
                        ) : null}
                        <div className="token-row">
                          <span>
                            {formatTokenCount(conversation.totalTokens, language)} {t.common.tokens}
                          </span>
                          <span>
                            {conversation.sessionCount} {t.common.sessions}
                          </span>
                        </div>
                        <div className="meta-row">
                          <span>{formatShortDate(conversation.updatedAt, language)}</span>
                        </div>
                      </button>
                    ))
                  )}
                </div>
              </aside>

              <section className="detail-panel">
                {activeDetail ? (
                  <>
                    <div className="detail-topline">
                      <div>
                        <p className="eyebrow">{t.detail.eyebrow}</p>
                        <h3>{activeDetail.title}</h3>
                        <p className="subtitle mono">{activeDetail.rootSessionId}</p>
                      </div>
                      <div className="badge-row">
                        {activeDetail.sourceStates.map((sourceState) => (
                          <span className="state-chip" key={sourceState}>
                            {sourceState}
                          </span>
                        ))}
                      </div>
                    </div>

                    <div className="detail-metrics">
                      <MiniCard
                        label={t.metrics.apiValue}
                        tone="primary"
                        value={formatUsd(activeDetail.apiValueUsd, language)}
                      />
                      <MiniCard
                        label={t.charts.tokens}
                        tone="secondary"
                        value={formatTokenCount(activeDetail.totalTokens, language)}
                      />
                    </div>

                    <ModelShareChart
                      data={detailShareData}
                      mode={shareMode}
                      onModeChange={setShareMode}
                      dimension={shareDimension}
                      onDimensionChange={setShareDimension}
                      title={shareChartTitle(
                        shareDimension,
                        t.detail.conversationModelBreakdown,
                        t.detail.conversationCostBreakdown,
                        t.charts.sourceDistribution,
                      )}
                      eyebrow={t.detail.modelBreakdown}
                    />

                    <section className="section-card">
                      <div className="chart-heading">
                        <div>
                          <p className="eyebrow">{t.detail.turnTimelineEyebrow}</p>
                          <h3>{t.detail.turnUsage}</h3>
                        </div>
                        <p className="chart-note">
                          {t.detail.latestTurns(Math.min(activeDetail.turns.length, 40))}
                        </p>
                      </div>
                      <div className="timeline-grid">
                        {activeDetail.turns.length === 0 ? (
                          <div className="empty-state">{t.detail.emptyTurns}</div>
                        ) : (
                          activeDetail.turns.slice(-40).reverse().map((turn) => {
                            const session = sessionSummariesById.get(turn.sessionId)
                            const modelSummary = formatModelSummary(turn.modelIds, t)
                            const sessionLabel = formatSessionLabel(session, turn.sessionId, t)
                            const statusLabel = formatTurnStatus(turn.status)
                            const title = turn.userMessage ?? turn.assistantMessage ?? turn.turnId

                            return (
                              <article className="timeline-row" key={`${turn.sessionId}-${turn.turnId}`}>
                                <div className="timeline-head">
                                  <strong title={title}>{formatTurnHeadline(turn, t)}</strong>
                                  <div className="timeline-value-group">
                                    <span className="timeline-metric">
                                      {formatTokenCount(turn.totalTokens, language)} {t.common.tokens}
                                    </span>
                                    <span className="timeline-value">
                                      {formatUsd(turn.valueUsd, language)}
                                    </span>
                                  </div>
                                </div>
                                <div className="timeline-meta">
                                  <span className="timeline-pill">
                                    {formatDateTime(turn.lastActivityAt, language)}
                                  </span>
                                  <span className="timeline-pill">{sessionLabel}</span>
                                  <span className="timeline-pill">{modelSummary}</span>
                                  {statusLabel ? <span className="timeline-pill">{statusLabel}</span> : null}
                                </div>
                              </article>
                            )
                          })
                        )}
                      </div>
                    </section>
                  </>
                ) : (
                  <div className="empty-state">{t.detail.emptySelection}</div>
                )}
              </section>
            </section>
          )}
        </main>
      </div>

      <SettingsPanel
        isOpen={settingsOpen}
        language={language}
        liveRateLimits={liveRateLimits}
        onClose={() => setSettingsOpen(false)}
        onLanguageChange={setLanguage}
        onDeleteSubscriptionRecord={handleDeleteSubscriptionRecord}
        onSave={handleSaveSettings}
        onSaveSubscriptionRecord={handleSaveSubscriptionRecord}
        subscriptionRecords={subscriptionRecords}
        accountStatus={accountStatus}
        syncSettings={syncSettings}
      />

      <SourceManagerModal
        deletingSourceIds={deletingSourceIds}
        downloadingSourceIds={downloadingSourceIds}
        isOpen={sourceManagerOpen}
        message={sourceManagerMessage}
        onClose={() => setSourceManagerOpen(false)}
        onDeleteSource={handleDeleteSource}
        onDownloadAllSources={handleDownloadAllSources}
        onDownloadSource={handleDownloadSource}
        onDownloadSelectedSources={handleDownloadSelectedSources}
        onOpenAddModal={handleOpenSourceModal}
        onToggleSource={handleToggleSource}
        sources={codexSources}
      />

      <SourceAddModal
        candidates={sourceCandidates}
        isOpen={sourceModalOpen}
        message={sourceModalMessage}
        onAddCandidate={handleAddSource}
        onClose={() => setSourceModalOpen(false)}
        onDiscover={handleDiscoverSources}
        sources={codexSources}
      />

      <SourceDeleteDialog
        isDeleting={pendingDeleteSource ? deletingSourceIds.has(pendingDeleteSource.id) : false}
        onCancel={() => setPendingDeleteSource(null)}
        onConfirm={confirmDeleteSource}
        source={pendingDeleteSource}
      />
    </div>
  )
}

export default App
