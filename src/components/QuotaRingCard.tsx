interface QuotaRingCardProps {
  label: string
  percent: number
  timePercent: number | null
  subtitle: string
  tone: 'warm' | 'cool'
  available?: boolean
}

const RING_VIEWBOX_SIZE = 136
const RING_CENTER = RING_VIEWBOX_SIZE / 2
const OUTER_RADIUS = 47
const INNER_RADIUS = 34
const OUTER_CIRCUMFERENCE = 2 * Math.PI * OUTER_RADIUS
const INNER_CIRCUMFERENCE = 2 * Math.PI * INNER_RADIUS

export function QuotaRingCard({ label, percent, timePercent, subtitle, tone, available = true }: QuotaRingCardProps) {
  const clampedPercent = available ? clampPercent(percent) : 0
  const clampedTimePercent = available && timePercent !== null ? clampPercent(timePercent) : null
  const outerStrokeOffset = OUTER_CIRCUMFERENCE * (1 - clampedPercent / 100)
  const innerStrokeOffset =
    clampedTimePercent === null ? INNER_CIRCUMFERENCE : INNER_CIRCUMFERENCE * (1 - clampedTimePercent / 100)

  return (
    <section className={`quota-ring-card quota-ring-card--${tone} ${available ? '' : 'quota-ring-card--empty'}`}>
      <div className="quota-ring-visual">
        <svg aria-hidden="true" className="quota-ring-svg" viewBox={`0 0 ${RING_VIEWBOX_SIZE} ${RING_VIEWBOX_SIZE}`}>
          <circle className="quota-ring-track quota-ring-track--outer" cx={RING_CENTER} cy={RING_CENTER} r={OUTER_RADIUS} />
          <circle
            className="quota-ring-progress quota-ring-progress--outer"
            cx={RING_CENTER}
            cy={RING_CENTER}
            r={OUTER_RADIUS}
            strokeDasharray={OUTER_CIRCUMFERENCE}
            strokeDashoffset={outerStrokeOffset}
          />
          <circle className="quota-ring-track quota-ring-track--inner" cx={RING_CENTER} cy={RING_CENTER} r={INNER_RADIUS} />
          <circle
            className="quota-ring-progress quota-ring-progress--inner"
            cx={RING_CENTER}
            cy={RING_CENTER}
            r={INNER_RADIUS}
            strokeDasharray={INNER_CIRCUMFERENCE}
            strokeDashoffset={innerStrokeOffset}
          />
        </svg>
        <div className="quota-ring-copy">
          <strong>{available ? `${clampedPercent}%` : '—'}</strong>
        </div>
      </div>
      <p className="quota-ring-label">{label}</p>
      <p className="quota-ring-subtitle">{subtitle}</p>
    </section>
  )
}

function clampPercent(value: number) {
  if (!Number.isFinite(value)) return 0
  return Math.max(0, Math.min(100, Math.round(value)))
}
