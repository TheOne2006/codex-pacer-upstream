import type { TranslationSet } from '../../app/i18n'
import type { CodexSource, CodexSourceCandidate } from '../../app/types'

export function sourceSelectionSummary(sources: CodexSource[], t: TranslationSet) {
  if (sources.length === 0) {
    return 'localhost'
  }
  const selected = sources.filter((source) => source.displaySelected)
  if (selected.length === 0) {
    return t.sources.noneSelected
  }
  if (selected.length === sources.length) {
    const remoteCount = sources.filter((source) => source.kind === 'ssh').length
    return remoteCount > 0 ? `localhost + ${remoteCount}` : 'localhost'
  }
  const labels = selected.slice(0, 2).map((source) => source.label).join(t.sources.listSeparator)
  return selected.length > 2 ? `${labels} +${selected.length - 2}` : labels
}

export function upsertSourceInList(sources: CodexSource[], source: CodexSource) {
  const exists = sources.some((item) => item.id === source.id)
  const next = exists
    ? sources.map((item) => (item.id === source.id ? source : item))
    : [...sources, source]
  return next.sort((left, right) => {
    if (left.id === 'local') return -1
    if (right.id === 'local') return 1
    return left.label.localeCompare(right.label)
  })
}

export function formatSourceSubtitle(source: CodexSource, t: TranslationSet) {
  if (source.kind === 'local') {
    return source.localCodexHome || t.sources.localCodexHome
  }
  const address = [source.user, source.hostName || source.sshAlias]
    .filter(Boolean)
    .join('@')
  return `${address}${source.port ? `:${source.port}` : ''} · ${source.remoteCodexHome || '~/.codex'}`
}

export function formatCandidateSubtitle(candidate: CodexSourceCandidate) {
  const address = [candidate.user, candidate.hostName || candidate.sshAlias].filter(Boolean).join('@')
  return `${address}${candidate.port ? `:${candidate.port}` : ''} · ${candidate.remoteCodexHome}`
}

export function formatSourceStatus(source: CodexSource, t: TranslationSet) {
  if (source.kind === 'local') return t.sources.local
  if (source.status === 'ready') {
    return source.lastDownloadedAt ? t.sources.cached : t.sources.added
  }
  if (source.status === 'failed') return t.sources.failed
  if (source.status === 'downloading') return t.sources.downloading
  return t.sources.notDownloaded
}
