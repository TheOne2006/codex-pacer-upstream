import type { CompositionShare, ModelShare, ShareDimension, ShareSlice, SourceShare } from '../../app/types'

export function buildShareSlices(
  dimension: ShareDimension,
  modelShares: ModelShare[],
  compositionShares: CompositionShare[],
  sourceShares: SourceShare[],
): ShareSlice[] {
  if (dimension === 'composition') {
    return compositionShares.map((item) => ({
      id: item.category,
      label: item.label,
      secondaryLabel:
        item.category === 'input'
          ? 'uncached'
          : item.category === 'cache'
            ? 'cached'
            : 'generated',
      apiValueUsd: item.apiValueUsd,
      totalTokens: item.totalTokens,
      color: item.color,
    }))
  }

  if (dimension === 'source') {
    return sourceShares.map((item) => ({
      id: item.sourceId,
      label: item.displayName,
      secondaryLabel: item.sourceId,
      apiValueUsd: item.apiValueUsd,
      totalTokens: item.totalTokens,
      color: item.color,
    }))
  }

  return modelShares.map((item) => ({
    id: item.modelId,
    label: item.displayName,
    secondaryLabel: item.modelId,
    apiValueUsd: item.apiValueUsd,
    totalTokens: item.totalTokens,
    color: item.color,
  }))
}

export function shareChartTitle(
  dimension: ShareDimension,
  modelTitle: string,
  compositionTitle: string,
  sourceTitle: string,
) {
  if (dimension === 'model') return modelTitle
  if (dimension === 'source') return sourceTitle
  return compositionTitle
}
