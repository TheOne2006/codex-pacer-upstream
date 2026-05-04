import { useI18n } from '../../app/useI18n'
import type { CodexSource } from '../../app/types'
import { formatSourceSubtitle } from './sourceUtils'

interface SourceDeleteDialogProps {
  source: CodexSource | null
  isDeleting: boolean
  onCancel: () => void
  onConfirm: () => void
}

export function SourceDeleteDialog({
  source,
  isDeleting,
  onCancel,
  onConfirm,
}: SourceDeleteDialogProps) {
  const { t } = useI18n()
  if (!source) return null

  return (
    <div className="modal-backdrop" onClick={isDeleting ? undefined : onCancel}>
      <section className="modal-panel source-delete-panel" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">{t.sources.deleteServer}</p>
            <h3>{t.sources.deleteServerTitle(source.label)}</h3>
          </div>
        </div>
        <p className="source-delete-copy">{t.sources.deleteServerDescription}</p>
        <p className="source-delete-address">{formatSourceSubtitle(source, t)}</p>
        <div className="modal-actions">
          <button className="ghost-button" disabled={isDeleting} onClick={onCancel} type="button">
            {t.actions.cancel}
          </button>
          <button className="danger-button" disabled={isDeleting} onClick={onConfirm} type="button">
            {isDeleting ? t.sources.deleting : t.sources.confirmDelete}
          </button>
        </div>
      </section>
    </div>
  )
}
