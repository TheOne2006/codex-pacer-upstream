import type { OverviewBucket } from '../app/types'

export type AppLanguage = 'zh-CN' | 'en'

export interface SupportedLanguage {
  code: AppLanguage
  label: string
  nativeLabel: string
}

export interface I18nShape {
  language: AppLanguage
  languages: SupportedLanguage[]
  setLanguage: (language: AppLanguage) => void
  t: TranslationSet
}

export type TranslationSet = {
  appTitle: string
  appSubtitle: string
  nav: {
    overview: string
    conversations: string
  }
  actions: {
    rescanNow: string
    refreshPricing: string
    settings: string
    close: string
    cancel: string
    saveSettings: string
    saving: string
  }
  common: {
    autoScan: string
    disabled: string
    everyMinutes: (minutes: number) => string
    quotaWindow: string
    earlier: string
    newer: string
    start: string
    end: string
    reset: string
    updated: string
    tokens: string
    sessions: string
    session: string
    fast: string
    noData: string
    searchTitleOrSession: string
    customRangeStartDate: string
    customRangeEndDate: string
  }
  status: {
    waitingForFirstScan: string
    fetchingLiveQuotaWindow: string
    scannedFiles: (files: number, sessions: number) => string
    backgroundScanAlreadyRunning: string
    failedToLoad: (bucketLabel: string, error: string) => string
    pricingRefreshed: string
    settingsSaved: string
    subscriptionRecordSaved: string
    subscriptionRecordDeleted: string
    waitingLiveQuota: string
  }
  buckets: Record<OverviewBucket, string>
  bucketDescriptions: {
    subscriptionMonth: (day: number) => string
    currentFiveHourWindow: string
    historicalFiveHourWindow: string
    currentSevenDayWindow: string
    historicalSevenDayWindow: string
  }
  metrics: {
    apiValue: string
    wastedQuota: string
    remainingQuota: string
    peakUsed: (value: string) => string
    used: (value: string) => string
    windowStart: string
    remainingTime: string
    totalTokens: string
    conversationCount: string
    subscriptionCost: string
    payoffRatio: string
    planPerMonth: (planType: string, price: string) => string
    subscriptionLedgerNote: (count: number) => string
    noSubscriptionRecords: string
    monthlyFeeShare: string
  }
  overview: {
    distribution: string
    modelShare: string
    costStructure: string
    sourceShare: string
  }
  conversationList: {
    eyebrow: string
    title: string
    shown: (count: number) => string
    empty: string
  }
  detail: {
    eyebrow: string
    modelBreakdown: string
    conversationModelBreakdown: string
    conversationCostBreakdown: string
    conversationSourceBreakdown: string
    turnTimelineEyebrow: string
    turnUsage: string
    latestTurns: (count: number) => string
    emptyTurns: string
    emptySelection: string
    untitledTurn: string
    unknownModel: string
    subagent: string
    mainSession: string
    sessionLabel: (sessionId: string) => string
  }
  charts: {
    trendEyebrow: string
    valueAndTokens: string
    valueVsTokens: string
    apiValue: string
    tokens: string
    noShareData: string
    valueUnavailableTokenFallback: string
    dimensionControlLabel: string
    metricControlLabel: string
    byModel: string
    byStructure: string
    bySource: string
    byValue: string
    byTokens: string
    liveQuotaEyebrow: string
    fiveHourQuotaTrend: string
    sevenDayQuotaTrend: string
    historicalQuotaNote: string
    currentQuotaNote: string
    noLiveQuotaHistory: string
    remaining: string
    cumulativeValue: string
    windowValue: string
  }
  sources: {
    label: string
    remoteSources: string
    chooseSources: string
    noneSelected: string
    listSeparator: string
    local: string
    localCodexHome: string
    cached: string
    added: string
    failed: string
    downloading: string
    notDownloaded: string
    addSsh: string
    updateAll: string
    noRemoteSources: string
    update: string
    updating: string
    deleteSource: string
    deleteSourceLabel: (label: string) => string
    deleting: string
    deleteServer: string
    deleteServerTitle: (label: string) => string
    deleteServerDescription: string
    confirmDelete: string
    addCodexServer: string
    refreshSshList: string
    filteredHostsNote: string
    noSshServersDiscovered: string
    add: string
    keepOneSelected: string
    addedSource: (label: string) => string
    downloadingRemoteCache: string
    downloadedAndImported: (files: number) => string
    noSshServersAdded: string
    deletingSource: (label: string) => string
    deletedSource: (label: string) => string
    maintenanceActions: string
  }
  popup: {
    title: string
    updated: (value: string) => string
    loading: string
    failedTitle: string
    resetIn: (value: string) => string
    speedTitle: string
    speedHint: string
    sevenDayUsageChart: string
    chartLegendRemaining: string
    chartLegendReference: string
    chartLegendCurrent: string
    chartValueBadge: string
    speedStatus: {
      fast: string
      healthy: string
      slow: string
    }
    resetTimeline5h: string
    resetTimeline7d: string
    modules: {
      apiValue: string
      tokenCount: string
      scanFreshness: string
      liveQuotaFreshness: string
      payoffRatio: string
      conversationCount: string
    }
    actions: {
      openDashboard: string
      refresh: string
      settings: string
    }
  }
  settings: {
    appSettings: string
    syncAndSubscriptionProfile: string
    sections: {
      language: {
        eyebrow: string
        title: string
        description: string
        label: string
        note: string
      }
      sync: {
        eyebrow: string
        title: string
        description: string
        codexHome: string
        codexHomePlaceholder: string
        autoScanEnabled: string
        autoScanEnabledNote: string
        autoScanIntervalMinutes: string
        liveQuotaRefreshIntervalSeconds: string
        liveQuotaRefreshNote: string
      }
      menuBar: {
        eyebrow: string
        title: string
        description: string
        hideDockIcon: string
        hideDockIconNote: string
        showLogo: string
        showLogoNote: string
        showApiValue: string
        showApiValueNote: string
        showLiveQuotaMetric: string
        showLiveQuotaMetricNote: string
        range: string
        rangeNote: string
        liveMetric: string
        liveMetricRemainingPercent: string
        liveMetricSuggestedUsageSpeed: string
        liveMetricNote: string
        quotaSource: string
        quotaSourceNote: string
        speedEmojiSection: string
        speedShowEmoji: string
        speedShowEmojiNote: string
        speedFastThreshold: string
        speedSlowThreshold: string
        speedThresholdNote: string
        speedHealthyEmoji: string
        speedFastEmoji: string
        speedSlowEmoji: string
        speedEmojiNote: string
        popupEnabled: string
        popupEnabledNote: string
        popupModules: string
        popupModulesNote: string
        popupShowResetTimeline: string
        popupShowActions: string
        moveUp: string
        moveDown: string
      }
      subscription: {
        eyebrow: string
        title: string
        description: string
        planType: string
        currency: string
        currencyNote: string
        monthlyPrice: string
        billingAnchorDay: string
        billingAnchorDayNote: string
        accountStatus: string
        accountUnavailable: string
        accountRequiresLogin: string
        accountApiKey: string
        accountUnknown: string
        planPlus: string
        planPro5: string
        planPro10: string
        amountUsd: string
        serviceStart: string
        serviceEnd: string
        addRecordTitle: string
        editRecordTitle: string
        addRecord: string
        updateRecord: string
        saveRecord: string
        editRecord: string
        removeRecord: string
        cancelEditRecord: string
        emptyRecords: string
        accountEmail: string
        accountEmailPlaceholder: string
      }
      liveQuota: {
        eyebrow: string
        title: string
        description: string
        remaining: (value: string) => string
        timeLeft: (value: string) => string
      }
    }
  }
}
