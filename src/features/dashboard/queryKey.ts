import type { OverviewBucket } from '../../app/types'

export function buildQueryKey(
  bucket: OverviewBucket,
  anchor: string | null,
  customStart: string | null,
  customEnd: string | null,
  search: string | null,
  liveWindowOffset: number,
  sourceSelectionKey = '',
) {
  return [
    bucket,
    anchor ?? '',
    customStart ?? '',
    customEnd ?? '',
    search ?? '',
    String(liveWindowOffset),
    sourceSelectionKey,
  ].join('::')
}
