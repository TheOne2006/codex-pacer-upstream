interface MetricCardProps {
  caption: string
  value: string
  note?: string
  compact?: boolean
  tone?: 'primary' | 'secondary' | 'accent' | 'neutral'
  featured?: boolean
}

interface MiniCardProps {
  label: string
  value: string
  tone?: 'primary' | 'secondary' | 'accent'
}

export function MetricCard({
  caption,
  value,
  note,
  compact = false,
  tone = 'neutral',
  featured = false,
}: MetricCardProps) {
  const density = metricValueDensity(value)
  return (
    <section
      className={`metric-card metric-card--${tone} metric-card--${density} ${
        compact ? 'metric-card--compact' : ''
      } ${featured ? 'metric-card--featured' : ''}`}
    >
      <span className="metric-label">{caption}</span>
      <strong aria-label={value} title={value}>
        {value}
      </strong>
      {note ? <span className="stat-caption">{note}</span> : null}
    </section>
  )
}

export function MiniCard({ label, value, tone = 'secondary' }: MiniCardProps) {
  return (
    <div className={`detail-mini-card detail-mini-card--${tone}`}>
      <span className="metric-label">{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function metricValueDensity(value: string) {
  const compactLength = Array.from(value.replace(/\s+/g, '')).length
  if (compactLength >= 11) return 'value-very-long'
  if (compactLength >= 7) return 'value-long'
  return 'value-normal'
}
