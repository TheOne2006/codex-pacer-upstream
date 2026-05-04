import { useI18n } from '../../app/useI18n'
import type { CodexSource, CodexSourceCandidate } from '../../app/types'
import { formatCandidateSubtitle } from './sourceUtils'

interface SourceAddModalProps {
  sources: CodexSource[]
  candidates: CodexSourceCandidate[]
  isOpen: boolean
  message: string
  onClose: () => void
  onDiscover: () => void
  onAddCandidate: (candidate: CodexSourceCandidate) => void
}

export function SourceAddModal({
  sources,
  candidates,
  isOpen,
  message,
  onClose,
  onDiscover,
  onAddCandidate,
}: SourceAddModalProps) {
  const { t } = useI18n()
  if (!isOpen) return null

  const addedIds = new Set(sources.map((source) => source.id))
  const visibleCandidates = candidates.filter((candidate) => !candidate.ignoredReason)

  return (
    <div className="modal-backdrop">
      <section className="modal-panel source-modal-panel">
        <div className="modal-header">
          <div>
            <p className="eyebrow">SSH</p>
            <h3>{t.sources.addCodexServer}</h3>
          </div>
          <button className="ghost-button" onClick={onClose} type="button">
            {t.actions.close}
          </button>
        </div>
        <div className="source-modal-toolbar">
          <button className="ghost-button" onClick={onDiscover} type="button">
            {t.sources.refreshSshList}
          </button>
          <span>{t.sources.filteredHostsNote}</span>
        </div>
        <div className="source-candidate-list">
          {visibleCandidates.length === 0 ? (
            <div className="empty-state">{t.sources.noSshServersDiscovered}</div>
          ) : (
            visibleCandidates.map((candidate) => {
              const alreadyAdded = addedIds.has(candidate.id)
              return (
                <article className="source-candidate-card" key={candidate.id}>
                  <div>
                    <strong>{candidate.label}</strong>
                    <span>{formatCandidateSubtitle(candidate)}</span>
                  </div>
                  <button
                    className="accent-button source-mini-button"
                    disabled={alreadyAdded}
                    onClick={() => onAddCandidate(candidate)}
                    type="button"
                  >
                    {alreadyAdded ? t.sources.added : t.sources.add}
                  </button>
                </article>
              )
            })
          )}
        </div>
        {message ? <p className="source-message source-message--modal">{message}</p> : null}
      </section>
    </div>
  )
}
