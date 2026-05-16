import { ChevronDown } from 'lucide-react'

import { useI18n } from '../../app/useI18n'
import type { CodexSource } from '../../app/types'
import { formatSourceStatus, formatSourceSubtitle, sourceSelectionSummary } from './sourceUtils'

interface SourceSelectorPanelProps {
  sources: CodexSource[]
  isOpen: boolean
  message: string
  onToggleOpen: () => void
  onToggleSource: (source: CodexSource, selected: boolean) => void
}

export function SourceSelectorPanel({
  sources,
  isOpen,
  message,
  onToggleOpen,
  onToggleSource,
}: SourceSelectorPanelProps) {
  const { t } = useI18n()
  const selectedCount = sources.filter((source) => source.displaySelected).length
  const remoteCount = sources.filter((source) => source.kind === 'ssh').length
  const selectedSummary = sourceSelectionSummary(sources, t)

  return (
    <section className="source-picker">
      <span className="time-filter-label">{t.sources.label}</span>
      <button className={`source-picker-trigger ${isOpen ? 'active' : ''}`} onClick={onToggleOpen} type="button">
        <span>{selectedSummary}</span>
        <small>
          {selectedCount}/{sources.length || 1}
          {remoteCount > 0 ? ` · SSH ${remoteCount}` : ''}
        </small>
        <ChevronDown aria-hidden="true" size={16} />
      </button>
      {isOpen ? (
        <div className="source-popover">
          <div className="source-popover-header">
            <span>{t.sources.chooseSources}</span>
          </div>
          <div className="source-list">
            {sources.map((source) => (
              <article className="source-row source-row--select" key={source.id}>
                <label className="source-row-main">
                  <input
                    checked={source.displaySelected}
                    onChange={(event) => onToggleSource(source, event.target.checked)}
                    type="checkbox"
                  />
                  <span>
                    <strong>{source.label}</strong>
                    <small>{formatSourceSubtitle(source, t)}</small>
                  </span>
                </label>
                <span className={`source-status source-status--${source.status}`}>{formatSourceStatus(source, t)}</span>
              </article>
            ))}
          </div>
          {message ? <p className="source-message source-message--picker">{message}</p> : null}
        </div>
      ) : null}
    </section>
  )
}
