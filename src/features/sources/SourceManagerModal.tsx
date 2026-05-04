import { DatabaseZap, Trash2 } from 'lucide-react'

import { useI18n } from '../../app/useI18n'
import type { CodexSource } from '../../app/types'
import { formatSourceStatus, formatSourceSubtitle } from './sourceUtils'

interface SourceManagerModalProps {
  sources: CodexSource[]
  downloadingSourceIds: Set<string>
  deletingSourceIds: Set<string>
  isOpen: boolean
  message: string
  onClose: () => void
  onOpenAddModal: () => void
  onDownloadSource: (sourceId: string) => void
  onDownloadAllSources: () => void
  onDeleteSource: (source: CodexSource) => void
}

export function SourceManagerModal({
  sources,
  downloadingSourceIds,
  deletingSourceIds,
  isOpen,
  message,
  onClose,
  onOpenAddModal,
  onDownloadSource,
  onDownloadAllSources,
  onDeleteSource,
}: SourceManagerModalProps) {
  const { t } = useI18n()
  if (!isOpen) return null

  const remoteSources = sources.filter((source) => source.kind === 'ssh')
  const hasRemoteSources = remoteSources.length > 0
  const anyBusy = remoteSources.some(
    (source) => downloadingSourceIds.has(source.id) || deletingSourceIds.has(source.id),
  )

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <section className="modal-panel source-manager-modal" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header source-manager-modal-header">
          <div className="source-manager-title">
            <div className="sidebar-source-trigger-icon source-manager-title-icon" aria-hidden="true">
              <DatabaseZap size={20} />
            </div>
            <div>
              <p className="eyebrow">{t.sources.remoteSources}</p>
              <h3>SSH Codex</h3>
            </div>
          </div>
          <button className="ghost-button" onClick={onClose} type="button">
            {t.actions.close}
          </button>
        </div>

        <div className="source-manager-actions">
          <button className="ghost-button sidebar-source-action" onClick={onOpenAddModal} type="button">
            {t.sources.addSsh}
          </button>
          <button
            className="ghost-button sidebar-source-action sidebar-source-action--update"
            disabled={!hasRemoteSources || anyBusy}
            onClick={onDownloadAllSources}
            type="button"
          >
            {t.sources.updateAll}
          </button>
        </div>

        <div className="source-manager-list">
          {remoteSources.length === 0 ? (
            <p className="sidebar-source-empty">{t.sources.noRemoteSources}</p>
          ) : (
            remoteSources.map((source) => {
              const downloading = downloadingSourceIds.has(source.id)
              const deleting = deletingSourceIds.has(source.id)
              const busy = downloading || deleting
              return (
                <article className="source-manager-item" key={source.id}>
                  <div className="sidebar-source-title-row">
                    <strong title={source.label}>{source.label}</strong>
                    <span className={`source-status source-status--${source.status}`}>{formatSourceStatus(source, t)}</span>
                  </div>
                  <small title={formatSourceSubtitle(source, t)}>{formatSourceSubtitle(source, t)}</small>
                  <div className="sidebar-source-row-actions">
                    <button
                      className="ghost-button source-mini-button"
                      disabled={busy}
                      onClick={() => onDownloadSource(source.id)}
                      type="button"
                    >
                      {downloading ? t.sources.updating : t.sources.update}
                    </button>
                    <button
                      aria-label={t.sources.deleteSourceLabel(source.label)}
                      className="ghost-button source-icon-button source-danger-button"
                      disabled={busy}
                      onClick={() => onDeleteSource(source)}
                      title={deleting ? t.sources.deleting : t.sources.deleteSource}
                      type="button"
                    >
                      <Trash2 aria-hidden="true" size={14} />
                    </button>
                  </div>
                  {busy ? <div className="source-progress"><span /></div> : null}
                  {source.lastError ? <p className="source-error">{source.lastError}</p> : null}
                </article>
              )
            })
          )}
        </div>
        {message ? <p className="source-message sidebar-source-message source-manager-message">{message}</p> : null}
      </section>
    </div>
  )
}
