import type { LiveRateLimitSnapshot, OverviewBucket } from '../../app/types'

export function selectActiveRateLimitWindow(
  liveRateLimits: LiveRateLimitSnapshot | null,
  bucket: OverviewBucket,
) {
  if (!liveRateLimits) return null
  if (bucket === 'five_hour') return liveRateLimits.primary
  if (bucket === 'seven_day') return liveRateLimits.secondary
  return null
}
