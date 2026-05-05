import { useEffect, useRef, useState } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'

import { formatPercent, formatRemainingDuration, formatUsd, todayInputValue } from '../app/format'
import { SUPPORTED_LANGUAGES, type AppLanguage } from '../app/i18n'
import { useI18n } from '../app/useI18n'
import type {
  CodexAccountStatus,
  LiveRateLimitSnapshot,
  MenuBarPopupModuleId,
  OverviewBucket,
  SubscriptionRecord,
  SubscriptionRecordInput,
  SyncSettings,
} from '../app/types'

interface SettingsPanelProps {
  isOpen: boolean
  language: AppLanguage
  liveRateLimits: LiveRateLimitSnapshot | null
  syncSettings: SyncSettings | null
  subscriptionRecords: SubscriptionRecord[]
  accountStatus: CodexAccountStatus | null
  onClose: () => void
  onLanguageChange: (language: AppLanguage) => void
  onSave: (payload: {
    syncSettings: SyncSettings
  }) => Promise<void>
  onSaveSubscriptionRecord: (payload: SubscriptionRecordInput, id?: number | null) => Promise<void>
  onDeleteSubscriptionRecord: (id: number) => Promise<void>
}

interface SwitchFieldProps {
  label: string
  checked: boolean
  className?: string
  disabled?: boolean
  onChange: (checked: boolean) => void
}

function SwitchField({ label, checked, className = '', disabled = false, onChange }: SwitchFieldProps) {
  return (
    <label className={`switch-field${disabled ? ' is-disabled' : ''}${className ? ` ${className}` : ''}`}>
      <span>{label}</span>
      <input
        checked={checked}
        className="switch-input"
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        role="switch"
        type="checkbox"
      />
      <span className="switch-track" aria-hidden="true">
        <span />
      </span>
    </label>
  )
}

function addOneMonth(value: string) {
  const date = new Date(`${value}T00:00:00`)
  if (Number.isNaN(date.getTime())) return value
  date.setMonth(date.getMonth() + 1)
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

type SubscriptionPlanOption = 'plus' | 'pro_x5' | 'pro_x10'

interface SubscriptionRecordFormState {
  serviceStart: string
  serviceEnd: string
  amountUsd: number
  planType: SubscriptionPlanOption
  accountEmail: string
}

const SUBSCRIPTION_PLAN_AMOUNTS: Record<SubscriptionPlanOption, number> = {
  plus: 19.99,
  pro_x5: 100,
  pro_x10: 200,
}

function normalizePlanOption(planType?: string | null): SubscriptionPlanOption {
  const normalized = planType?.toLowerCase().replace(/[×*]/g, 'x').replace(/[\s-]+/g, '_') ?? ''
  if (normalized.includes('pro') && normalized.includes('5')) return 'pro_x5'
  if (normalized.includes('pro')) return 'pro_x10'
  return 'plus'
}

function defaultAmountForPlan(planType?: string | null) {
  return SUBSCRIPTION_PLAN_AMOUNTS[normalizePlanOption(planType)]
}

function createDefaultRecordForm(accountStatus: CodexAccountStatus | null): SubscriptionRecordFormState {
  const serviceStart = todayInputValue()
  const planType = normalizePlanOption(accountStatus?.planType)
  return {
    serviceStart,
    serviceEnd: addOneMonth(serviceStart),
    amountUsd: defaultAmountForPlan(planType),
    planType,
    accountEmail: accountStatus?.email ?? '',
  }
}

function recordToForm(record: SubscriptionRecord): SubscriptionRecordFormState {
  const planType = normalizePlanOption(record.planType)
  return {
    serviceStart: record.serviceStart,
    serviceEnd: record.serviceEnd,
    amountUsd: record.amountUsd,
    planType,
    accountEmail: record.note ?? '',
  }
}

function formatPlanLabel(
  planType: string,
  labels: {
    planPlus: string
    planPro5: string
    planPro10: string
  },
) {
  const normalized = normalizePlanOption(planType)
  if (normalized === 'pro_x5') return labels.planPro5
  if (normalized === 'pro_x10') return labels.planPro10
  return labels.planPlus
}

function maskEmail(email: string) {
  const [name, domain] = email.split('@')
  if (!name || !domain) return email
  const visible = name.slice(0, 2)
  return `${visible}${'•'.repeat(Math.max(2, name.length - visible.length))}@${domain}`
}

function formatAccountStatus(
  accountStatus: CodexAccountStatus | null,
  labels: {
    accountUnavailable: string
    accountRequiresLogin: string
    accountApiKey: string
    accountUnknown: string
  },
) {
  if (!accountStatus) return labels.accountUnavailable
  if (!accountStatus.available) return accountStatus.error || labels.accountUnavailable
  if (accountStatus.requiresOpenaiAuth && !accountStatus.accountType) return labels.accountRequiresLogin
  if (accountStatus.accountType === 'apiKey') return labels.accountApiKey
  const parts = [
    accountStatus.accountType ?? labels.accountUnknown,
    accountStatus.planType,
    accountStatus.email ? maskEmail(accountStatus.email) : null,
  ].filter(Boolean)
  return parts.join(' / ') || labels.accountUnknown
}

function refreshSecondsToMinutes(seconds: number) {
  return Math.max(1, Math.round(seconds / 60))
}

function refreshMinutesToSeconds(minutes: number) {
  return Math.min(60, Math.max(1, minutes)) * 60
}

function isMacOs() {
  return typeof navigator !== 'undefined' && navigator.platform.toLowerCase().includes('mac')
}

export function SettingsPanel({
  isOpen,
  language,
  liveRateLimits,
  syncSettings,
  subscriptionRecords,
  accountStatus,
  onClose,
  onLanguageChange,
  onSave,
  onSaveSubscriptionRecord,
  onDeleteSubscriptionRecord,
}: SettingsPanelProps) {
  const { t } = useI18n()
  const [draftSync, setDraftSync] = useState<SyncSettings | null>(syncSettings)
  const [recordForm, setRecordForm] = useState<SubscriptionRecordFormState>(() =>
    createDefaultRecordForm(accountStatus),
  )
  const [editingRecordId, setEditingRecordId] = useState<number | null>(null)
  const [saving, setSaving] = useState(false)
  const [savingRecord, setSavingRecord] = useState(false)
  const [deletingRecordId, setDeletingRecordId] = useState<number | null>(null)
  const wasOpenRef = useRef(false)
  const showDockSettings = isMacOs()

  useEffect(() => {
    const justOpened = isOpen && !wasOpenRef.current

    if (justOpened || (!draftSync && syncSettings)) {
      setDraftSync(syncSettings)
      setRecordForm(createDefaultRecordForm(accountStatus))
      setEditingRecordId(null)
      setSaving(false)
      setSavingRecord(false)
      setDeletingRecordId(null)
    } else if (!isOpen && wasOpenRef.current) {
      setDraftSync(syncSettings)
      setRecordForm(createDefaultRecordForm(accountStatus))
      setEditingRecordId(null)
      setSaving(false)
      setSavingRecord(false)
      setDeletingRecordId(null)
    }

    wasOpenRef.current = isOpen
  }, [accountStatus, draftSync, isOpen, syncSettings])

  if (!isOpen || !draftSync) return null

  const menuBarBucketOptions: Array<{ value: OverviewBucket; label: string }> = [
    { value: 'five_hour', label: t.buckets.five_hour },
    { value: 'day', label: t.buckets.day },
    { value: 'seven_day', label: t.buckets.seven_day },
    { value: 'week', label: t.buckets.week },
    { value: 'month', label: t.buckets.month },
    { value: 'year', label: t.buckets.year },
    { value: 'total', label: t.buckets.total },
  ]

  const menuBarLiveQuotaOptions = [
    { value: 'five_hour', label: t.buckets.five_hour },
    { value: 'seven_day', label: t.buckets.seven_day },
  ] as const

  const menuBarLiveQuotaMetricOptions = [
    { value: 'remaining_percent', label: t.settings.sections.menuBar.liveMetricRemainingPercent },
    {
      value: 'suggested_usage_speed',
      label: t.settings.sections.menuBar.liveMetricSuggestedUsageSpeed,
    },
  ] as const

  const popupModuleOptions: Array<{ value: MenuBarPopupModuleId; label: string }> = [
    { value: 'api_value', label: t.popup.modules.apiValue },
    { value: 'token_count', label: t.popup.modules.tokenCount },
    { value: 'scan_freshness', label: t.popup.modules.scanFreshness },
    { value: 'live_quota_freshness', label: t.popup.modules.liveQuotaFreshness },
    { value: 'payoff_ratio', label: t.popup.modules.payoffRatio },
    { value: 'conversation_count', label: t.popup.modules.conversationCount },
  ]

  function togglePopupModule(moduleId: MenuBarPopupModuleId, enabled: boolean) {
    setDraftSync((current) => {
      if (!current) return current
      const nextModules = enabled
        ? current.menuBarPopupModules.includes(moduleId)
          ? current.menuBarPopupModules
          : [...current.menuBarPopupModules, moduleId]
        : current.menuBarPopupModules.filter((item) => item !== moduleId)

      return {
        ...current,
        menuBarPopupModules: nextModules,
      }
    })
  }

  function movePopupModule(moduleId: MenuBarPopupModuleId, direction: -1 | 1) {
    setDraftSync((current) => {
      if (!current) return current
      const index = current.menuBarPopupModules.indexOf(moduleId)
      if (index < 0) return current

      const nextIndex = index + direction
      if (nextIndex < 0 || nextIndex >= current.menuBarPopupModules.length) return current

      const nextModules = [...current.menuBarPopupModules]
      const [item] = nextModules.splice(index, 1)
      nextModules.splice(nextIndex, 0, item)

      return {
        ...current,
        menuBarPopupModules: nextModules,
      }
    })
  }

  const planOptions: Array<{ value: SubscriptionPlanOption; label: string; amountUsd: number }> = [
    {
      value: 'plus',
      label: t.settings.sections.subscription.planPlus,
      amountUsd: SUBSCRIPTION_PLAN_AMOUNTS.plus,
    },
    {
      value: 'pro_x5',
      label: t.settings.sections.subscription.planPro5,
      amountUsd: SUBSCRIPTION_PLAN_AMOUNTS.pro_x5,
    },
    {
      value: 'pro_x10',
      label: t.settings.sections.subscription.planPro10,
      amountUsd: SUBSCRIPTION_PLAN_AMOUNTS.pro_x10,
    },
  ]

  const settingsGroupLabels = t.settings.sections.menuBar.groups

  function resetRecordForm() {
    setRecordForm(createDefaultRecordForm(accountStatus))
    setEditingRecordId(null)
  }

  function updateRecordForm(patch: Partial<SubscriptionRecordFormState>) {
    setRecordForm((current) => ({ ...current, ...patch }))
  }

  function updateRecordPlan(planType: SubscriptionPlanOption) {
    setRecordForm((current) => ({
      ...current,
      planType,
      amountUsd: SUBSCRIPTION_PLAN_AMOUNTS[planType],
    }))
  }

  function updateRecordServiceStart(serviceStart: string) {
    setRecordForm((current) => ({
      ...current,
      serviceStart,
      serviceEnd: current.serviceEnd <= serviceStart ? addOneMonth(serviceStart) : current.serviceEnd,
    }))
  }

  function editSubscriptionRecord(record: SubscriptionRecord) {
    setEditingRecordId(record.id)
    setRecordForm(recordToForm(record))
  }

  async function saveSubscriptionRecordForm() {
    setSavingRecord(true)
    try {
      const accountEmail = recordForm.accountEmail.trim()
      await onSaveSubscriptionRecord(
        {
          paidAt: recordForm.serviceStart,
          serviceStart: recordForm.serviceStart,
          serviceEnd: recordForm.serviceEnd,
          amountUsd: Math.max(0, Number(recordForm.amountUsd || 0)),
          planType: recordForm.planType,
          note: accountEmail || null,
        },
        editingRecordId,
      )
      resetRecordForm()
    } finally {
      setSavingRecord(false)
    }
  }

  async function removeSubscriptionRecord(id: number) {
    setDeletingRecordId(id)
    try {
      await onDeleteSubscriptionRecord(id)
      if (editingRecordId === id) {
        resetRecordForm()
      }
    } finally {
      setDeletingRecordId(null)
    }
  }

  async function handleSubmit() {
    if (!draftSync) return
    setSaving(true)
    try {
      const nextSync = draftSync
      await onSave({
        syncSettings: nextSync,
      })
      onClose()
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel settings-modal-panel" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">{t.settings.appSettings}</p>
            <h3>{t.settings.syncAndSubscriptionProfile}</h3>
          </div>
          <button className="ghost-button" onClick={onClose} type="button">
            {t.actions.close}
          </button>
        </div>

        <div className="modal-scroll">
          <div className="settings-sections">
            <section className="settings-section">
              <div className="settings-section-head">
                <p className="eyebrow">{t.settings.sections.language.eyebrow}</p>
                <h4>{t.settings.sections.language.title}</h4>
              </div>

              <div className="settings-grid">
                <label className="field">
                  <span>{t.settings.sections.language.label}</span>
                  <select
                    value={language}
                    onChange={(event) => onLanguageChange(event.target.value as AppLanguage)}
                  >
                    {SUPPORTED_LANGUAGES.map((option) => (
                      <option key={option.code} value={option.code}>
                        {option.nativeLabel} · {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </section>

            <section className="settings-section">
              <div className="settings-section-head">
                <p className="eyebrow">{t.settings.sections.sync.eyebrow}</p>
                <h4>{t.settings.sections.sync.title}</h4>
              </div>

              <div className="settings-grid settings-grid--two settings-grid--compact">
                <label className="field field-span-2">
                  <span>{t.settings.sections.sync.codexHome}</span>
                  <input
                    value={draftSync.codexHome ?? ''}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              codexHome: event.target.value.trim() || null,
                            }
                          : current,
                      )
                    }
                    placeholder={t.settings.sections.sync.codexHomePlaceholder}
                  />
                </label>

                <SwitchField
                  checked={draftSync.autoScanEnabled}
                  label={t.settings.sections.sync.autoScanEnabled}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            autoScanEnabled: checked,
                          }
                        : current,
                    )
                  }
                />

                <label className="field">
                  <span>{t.settings.sections.sync.autoScanIntervalMinutes}</span>
                  <input
                    min={1}
                    step={1}
                    type="number"
                    value={draftSync.autoScanIntervalMinutes}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              autoScanIntervalMinutes: Math.max(1, Number(event.target.value || 5)),
                            }
                          : current,
                      )
                    }
                  />
                </label>

                <label className="field">
                  <span>{t.settings.sections.sync.liveQuotaRefreshIntervalSeconds}</span>
                  <input
                    min={1}
                    max={60}
                    step={1}
                    type="number"
                    value={refreshSecondsToMinutes(draftSync.liveQuotaRefreshIntervalSeconds)}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              liveQuotaRefreshIntervalSeconds: refreshMinutesToSeconds(
                                Number(event.target.value || 5),
                              ),
                            }
                          : current,
                      )
                    }
                  />
                </label>
              </div>
            </section>

            <section className="settings-section">
              <div className="settings-section-head">
                <p className="eyebrow">{t.settings.sections.menuBar.eyebrow}</p>
                <h4>{t.settings.sections.menuBar.title}</h4>
              </div>

              <div className="settings-group-grid">
                <div className="settings-card-group settings-card-group--span-2">
                  <span className="settings-group-label">{settingsGroupLabels.display}</span>
                  <div className="settings-grid settings-grid--four settings-grid--compact">
                {showDockSettings ? (
                  <SwitchField
                    checked={draftSync.hideDockIconWhenMenuBarVisible}
                    label={t.settings.sections.menuBar.hideDockIcon}
                    onChange={(checked) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              hideDockIconWhenMenuBarVisible: checked,
                            }
                          : current,
                      )
                    }
                  />
                ) : null}

                <SwitchField
                  checked={draftSync.showMenuBarLogo}
                  label={t.settings.sections.menuBar.showLogo}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            showMenuBarLogo: checked,
                          }
                        : current,
                    )
                  }
                />

                <SwitchField
                  checked={draftSync.showMenuBarDailyApiValue}
                  label={t.settings.sections.menuBar.showApiValue}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            showMenuBarDailyApiValue: checked,
                          }
                        : current,
                    )
                  }
                />

                <SwitchField
                  checked={draftSync.showMenuBarLiveQuotaPercent}
                  label={t.settings.sections.menuBar.showLiveQuotaMetric}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            showMenuBarLiveQuotaPercent: checked,
                          }
                        : current,
                    )
                  }
                />

                  </div>
                </div>

                <div className="settings-card-group">
                  <span className="settings-group-label">{settingsGroupLabels.valueSource}</span>
                  <div className="settings-grid settings-grid--compact">
                <label className="field">
                  <span>{t.settings.sections.menuBar.range}</span>
                  <select
                    value={draftSync.menuBarBucket === 'subscription_month' ? 'month' : draftSync.menuBarBucket}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarBucket: event.target.value as OverviewBucket,
                            }
                          : current,
                      )
                    }
                  >
                    {menuBarBucketOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.liveMetric}</span>
                  <select
                    disabled={!draftSync.showMenuBarLiveQuotaPercent}
                    value={draftSync.menuBarLiveQuotaMetric}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarLiveQuotaMetric: event.target.value as
                                | 'remaining_percent'
                                | 'suggested_usage_speed',
                            }
                          : current,
                      )
                    }
                  >
                    {menuBarLiveQuotaMetricOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.quotaSource}</span>
                  <select
                    disabled={!draftSync.showMenuBarLiveQuotaPercent}
                    value={draftSync.menuBarLiveQuotaBucket}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarLiveQuotaBucket: event.target.value as 'five_hour' | 'seven_day',
                            }
                          : current,
                      )
                    }
                  >
                    {menuBarLiveQuotaOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>

                  </div>
                </div>

                <div className="settings-card-group">
                  <span className="settings-group-label">{settingsGroupLabels.pace}</span>
                  <div className="settings-grid settings-grid--three settings-grid--compact">
                <SwitchField
                  checked={draftSync.menuBarSpeedShowEmoji}
                  disabled={
                    !draftSync.showMenuBarLiveQuotaPercent ||
                    draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                  }
                  label={t.settings.sections.menuBar.speedShowEmoji}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            menuBarSpeedShowEmoji: checked,
                          }
                        : current,
                    )
                  }
                />

                <label className="field">
                  <span>{t.settings.sections.menuBar.speedFastThreshold}</span>
                  <input
                    disabled={
                      !draftSync.showMenuBarLiveQuotaPercent ||
                      draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                    }
                    min={0}
                    max={1000}
                    step={1}
                    type="number"
                    value={draftSync.menuBarSpeedFastThresholdPercent}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarSpeedFastThresholdPercent: Math.max(
                                0,
                                Math.min(1000, Number(event.target.value || 0)),
                              ),
                            }
                          : current,
                      )
                    }
                  />
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.speedSlowThreshold}</span>
                  <input
                    disabled={
                      !draftSync.showMenuBarLiveQuotaPercent ||
                      draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                    }
                    min={0}
                    max={1000}
                    step={1}
                    type="number"
                    value={draftSync.menuBarSpeedSlowThresholdPercent}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarSpeedSlowThresholdPercent: Math.max(
                                0,
                                Math.min(1000, Number(event.target.value || 0)),
                              ),
                            }
                          : current,
                      )
                    }
                  />
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.speedHealthyEmoji}</span>
                  <input
                    disabled={
                      !draftSync.showMenuBarLiveQuotaPercent ||
                      draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                    }
                    value={draftSync.menuBarSpeedHealthyEmoji}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarSpeedHealthyEmoji: event.target.value,
                            }
                          : current,
                      )
                    }
                  />
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.speedFastEmoji}</span>
                  <input
                    disabled={
                      !draftSync.showMenuBarLiveQuotaPercent ||
                      draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                    }
                    value={draftSync.menuBarSpeedFastEmoji}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarSpeedFastEmoji: event.target.value,
                            }
                          : current,
                      )
                    }
                  />
                </label>

                <label className="field">
                  <span>{t.settings.sections.menuBar.speedSlowEmoji}</span>
                  <input
                    disabled={
                      !draftSync.showMenuBarLiveQuotaPercent ||
                      draftSync.menuBarLiveQuotaMetric !== 'suggested_usage_speed'
                    }
                    value={draftSync.menuBarSpeedSlowEmoji}
                    onChange={(event) =>
                      setDraftSync((current) =>
                        current
                          ? {
                              ...current,
                              menuBarSpeedSlowEmoji: event.target.value,
                            }
                          : current,
                      )
                    }
                  />
                </label>

                  </div>
                </div>

                <div className="settings-card-group settings-card-group--span-2">
                  <span className="settings-group-label">{settingsGroupLabels.popup}</span>
                  <div className="settings-grid settings-grid--two settings-grid--compact">
                <SwitchField
                  checked={draftSync.menuBarPopupEnabled}
                  label={t.settings.sections.menuBar.popupEnabled}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            menuBarPopupEnabled: checked,
                          }
                        : current,
                    )
                  }
                />

                <SwitchField
                  checked={draftSync.menuBarPopupShowResetTimeline}
                  label={t.settings.sections.menuBar.popupShowResetTimeline}
                  onChange={(checked) =>
                    setDraftSync((current) =>
                      current
                        ? {
                            ...current,
                            menuBarPopupShowResetTimeline: checked,
                          }
                        : current,
                    )
                  }
                />

                <div className="field field-span-2 popup-module-editor">
                  <span>{t.settings.sections.menuBar.popupModules}</span>
                  <div className="popup-module-list">
                    {popupModuleOptions.map((option) => {
                      const enabled = draftSync.menuBarPopupModules.includes(option.value)
                      const index = draftSync.menuBarPopupModules.indexOf(option.value)
                      return (
                        <div className="popup-module-item" key={option.value}>
                          <label className="popup-module-toggle">
                            <input
                              checked={enabled}
                              className="switch-input"
                              onChange={(event) => togglePopupModule(option.value, event.target.checked)}
                              role="switch"
                              type="checkbox"
                            />
                            <span className="switch-track" aria-hidden="true">
                              <span />
                            </span>
                            <span>{option.label}</span>
                          </label>
                          <div className="popup-module-actions">
                            <button
                              aria-label={t.settings.sections.menuBar.moveUp}
                              className="ghost-button popup-icon-button"
                              disabled={!enabled || index <= 0}
                              onClick={() => movePopupModule(option.value, -1)}
                              type="button"
                            >
                              <ChevronUp aria-hidden="true" size={15} />
                            </button>
                            <button
                              aria-label={t.settings.sections.menuBar.moveDown}
                              className="ghost-button popup-icon-button"
                              disabled={!enabled || index < 0 || index >= draftSync.menuBarPopupModules.length - 1}
                              onClick={() => movePopupModule(option.value, 1)}
                              type="button"
                            >
                              <ChevronDown aria-hidden="true" size={15} />
                            </button>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
                  </div>
                </div>
              </div>
            </section>

            <section className="settings-section">
              <div className="settings-section-head">
                <p className="eyebrow">{t.settings.sections.subscription.eyebrow}</p>
                <h4>{t.settings.sections.subscription.title}</h4>
              </div>

              <div className="subscription-account-card">
                <span className="metric-label">{t.settings.sections.subscription.accountStatus}</span>
                <strong>{formatAccountStatus(accountStatus, t.settings.sections.subscription)}</strong>
              </div>

              <div className="subscription-record-list">
                {subscriptionRecords.length === 0 ? (
                  <div className="empty-state subscription-empty-state">
                    {t.settings.sections.subscription.emptyRecords}
                  </div>
                ) : (
                  subscriptionRecords.map((record) => (
                    <div className="subscription-record-card subscription-record-summary-card" key={record.id}>
                      <div className="subscription-record-summary-main">
                        <span className="subscription-record-plan-badge">
                          {formatPlanLabel(record.planType, t.settings.sections.subscription)}
                        </span>
                        <strong className="subscription-record-amount">{formatUsd(record.amountUsd, language)}</strong>
                        <span className="field-note">
                          {record.serviceStart} → {record.serviceEnd}
                        </span>
                      </div>
                      <div className="subscription-record-summary-meta">
                        {record.note ? (
                          <span>{record.note}</span>
                        ) : (
                          <span>{t.settings.sections.subscription.accountEmailPlaceholder}</span>
                        )}
                      </div>
                      <div className="subscription-record-actions">
                        <button
                          className="ghost-button"
                          disabled={savingRecord || deletingRecordId === record.id}
                          onClick={() => editSubscriptionRecord(record)}
                          type="button"
                        >
                          {t.settings.sections.subscription.editRecord}
                        </button>
                        <button
                          className="ghost-button"
                          disabled={savingRecord || deletingRecordId === record.id}
                          onClick={() => removeSubscriptionRecord(record.id)}
                          type="button"
                        >
                          {deletingRecordId === record.id
                            ? t.actions.saving
                            : t.settings.sections.subscription.removeRecord}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>

              <div className="subscription-record-editor">
                <div className="settings-section-head">
                  <p className="eyebrow">
                    {editingRecordId ? t.settings.sections.subscription.editRecordTitle : t.settings.sections.subscription.addRecordTitle}
                  </p>
                  <h4>
                    {editingRecordId ? t.settings.sections.subscription.updateRecord : t.settings.sections.subscription.addRecord}
                  </h4>
                </div>

                <div className="subscription-record-grid">
                  <label className="field">
                    <span>{t.settings.sections.subscription.planType}</span>
                    <select
                      value={recordForm.planType}
                      onChange={(event) => updateRecordPlan(event.target.value as SubscriptionPlanOption)}
                    >
                      {planOptions.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label} · {formatUsd(option.amountUsd, language)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>{t.settings.sections.subscription.amountUsd}</span>
                    <input
                      min={0}
                      step={0.01}
                      type="number"
                      value={recordForm.amountUsd}
                      onChange={(event) =>
                        updateRecordForm({ amountUsd: Math.max(0, Number(event.target.value || 0)) })
                      }
                    />
                  </label>
                  <label className="field">
                    <span>{t.settings.sections.subscription.accountEmail}</span>
                    <input
                      placeholder={t.settings.sections.subscription.accountEmailPlaceholder}
                      type="email"
                      value={recordForm.accountEmail}
                      onChange={(event) => updateRecordForm({ accountEmail: event.target.value })}
                    />
                  </label>
                  <label className="field">
                    <span>{t.settings.sections.subscription.serviceStart}</span>
                    <input
                      type="date"
                      value={recordForm.serviceStart}
                      onChange={(event) => updateRecordServiceStart(event.target.value)}
                    />
                  </label>
                  <label className="field">
                    <span>{t.settings.sections.subscription.serviceEnd}</span>
                    <input
                      type="date"
                      value={recordForm.serviceEnd}
                      onChange={(event) => updateRecordForm({ serviceEnd: event.target.value })}
                    />
                  </label>
                  <div className="subscription-record-form-actions">
                    {editingRecordId ? (
                      <button
                        className="ghost-button"
                        disabled={savingRecord}
                        onClick={resetRecordForm}
                        type="button"
                      >
                        {t.settings.sections.subscription.cancelEditRecord}
                      </button>
                    ) : null}
                    <button
                      className="accent-button"
                      disabled={
                        savingRecord ||
                        !recordForm.serviceStart ||
                        !recordForm.serviceEnd ||
                        recordForm.serviceEnd <= recordForm.serviceStart
                      }
                      onClick={saveSubscriptionRecordForm}
                      type="button"
                    >
                      {savingRecord
                        ? t.actions.saving
                        : editingRecordId
                          ? t.settings.sections.subscription.updateRecord
                          : t.settings.sections.subscription.saveRecord}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            {liveRateLimits ? (
              <section className="settings-section">
                <div className="settings-section-head">
                  <p className="eyebrow">{t.settings.sections.liveQuota.eyebrow}</p>
                  <h4>{t.settings.sections.liveQuota.title}</h4>
                </div>

                <div className="field field-readonly live-quota-field">
                  <div className="live-quota-grid">
                    <div className="live-quota-row">
                      <strong>{t.buckets.five_hour}</strong>
                      <span>
                        {t.settings.sections.liveQuota.remaining(
                          formatPercent((liveRateLimits.primary?.remainingPercent ?? 0) / 100, language),
                        )}
                      </span>
                      <span>
                        {t.settings.sections.liveQuota.timeLeft(
                          formatRemainingDuration(liveRateLimits.primary?.resetsAt ?? null, language),
                        )}
                      </span>
                    </div>
                    <div className="live-quota-row">
                      <strong>{t.buckets.seven_day}</strong>
                      <span>
                        {t.settings.sections.liveQuota.remaining(
                          formatPercent(
                            (liveRateLimits.secondary?.remainingPercent ?? 0) / 100,
                            language,
                          ),
                        )}
                      </span>
                      <span>
                        {t.settings.sections.liveQuota.timeLeft(
                          formatRemainingDuration(liveRateLimits.secondary?.resetsAt ?? null, language),
                        )}
                      </span>
                    </div>
                  </div>
                </div>
              </section>
            ) : null}
          </div>
        </div>

        <div className="modal-actions">
          <button className="ghost-button" onClick={onClose} type="button">
            {t.actions.cancel}
          </button>
          <button className="accent-button" disabled={saving} onClick={handleSubmit} type="button">
            {saving ? t.actions.saving : t.actions.saveSettings}
          </button>
        </div>
      </div>
    </div>
  )
}
