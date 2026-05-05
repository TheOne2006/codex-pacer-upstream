import type { TranslationSet } from '../../app/i18n'
import type { ConversationSessionSummary, ConversationTurnPoint } from '../../app/types'

export function formatTurnHeadline(turn: ConversationTurnPoint, t: TranslationSet) {
  const content = [turn.userMessage, turn.assistantMessage, turn.turnId]
    .filter((value): value is string => Boolean(value && value.trim()))
    .map((value) => value.replace(/\s+/g, ' ').trim())[0]

  if (!content) return t.detail.untitledTurn
  if (content.length <= 110) return content
  return `${content.slice(0, 109).trimEnd()}…`
}

export function formatModelSummary(modelIds: string[], t: TranslationSet) {
  if (modelIds.length === 0) return t.detail.unknownModel
  if (modelIds.length === 1) return modelIds[0]
  return `${modelIds[0]} +${modelIds.length - 1}`
}

export function formatSessionLabel(
  session: ConversationSessionSummary | undefined,
  sessionId: string,
  t: TranslationSet,
) {
  if (!session) return t.detail.sessionLabel(sessionId)
  if (session.agentNickname) return session.agentNickname
  return session.isSubagent ? t.detail.subagent : t.detail.mainSession
}

export function formatTurnStatus(status: string) {
  if (!status) return null
  if (status === 'completed') return null
  return status.replace(/_/g, ' ')
}
