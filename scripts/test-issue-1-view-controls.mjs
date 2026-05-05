import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import assert from 'node:assert/strict'

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)))
const chartSource = readFileSync(join(repoRoot, 'src/components/ModelShareChart.tsx'), 'utf8')
const cssSource = readFileSync(join(repoRoot, 'src/styles.css'), 'utf8')
const metricCardSource = readFileSync(join(repoRoot, 'src/shared/ui/MetricCards.tsx'), 'utf8')

assert.match(
  chartSource,
  /className="chart-control-group"[\s\S]*role="group"[\s\S]*aria-label=\{t\.charts\.dimensionControlLabel\}/,
  'distribution chart should expose the dimension controls as their own labelled button group',
)

assert.match(
  chartSource,
  /className="chart-control-group"[\s\S]*role="group"[\s\S]*aria-label=\{t\.charts\.metricControlLabel\}/,
  'distribution chart should expose the metric controls as their own labelled button group',
)

assert.match(
  chartSource,
  /aria-pressed=\{dimension === 'model'\}/,
  'dimension selector should expose selected state with aria-pressed',
)

assert.match(
  chartSource,
  /aria-pressed=\{mode === 'value'\}/,
  'metric selector should expose selected state with aria-pressed',
)

assert.doesNotMatch(
  chartSource,
  /role="radiogroup"|role="radio"|aria-checked=/,
  'distribution controls should not use custom radio semantics without radio keyboard behavior',
)

assert.match(
  chartSource,
  /\{hasRenderableData \? \(\s*<div className="share-center">/,
  'distribution chart should hide the center metric when the empty-state message is shown',
)

assert.match(
  cssSource,
  /\.chart-control-group\s*\{[\s\S]*?border:\s*1px solid var\(--line-subtle\);[\s\S]*?\}/,
  'distribution control groups should have a visible boundary between independent selectors',
)

assert.match(
  cssSource,
  /\.pill-strip button\s*\{[\s\S]*?white-space:\s*nowrap;[\s\S]*?\}/,
  'pill buttons should keep short option labels on one line',
)

assert.match(
  cssSource,
  /\.chart-heading > div:first-child\s*\{[\s\S]*?min-width:\s*0;[\s\S]*?\}/,
  'chart heading copy should be allowed to shrink before controls overflow',
)

assert.doesNotMatch(
  cssSource,
  /\.chart-heading > div:first-child\s*\{[\s\S]*?min-width:\s*max-content;[\s\S]*?\}/,
  'chart heading copy should not force max-content width',
)

assert.match(
  cssSource,
  /@media \(max-width: 760px\)[\s\S]*?\.chart-shell--secondary \.chart-heading\s*\{[\s\S]*?flex-direction:\s*column;[\s\S]*?\}/,
  'distribution chart heading should stack above controls on narrow screens',
)

assert.doesNotMatch(
  cssSource,
  /\.metric-card strong\s*\{[\s\S]*?word-break:\s*break-word;[\s\S]*?\}/,
  'metric values should not break arbitrary words or numeric units',
)

assert.match(
  cssSource,
  /\.metric-card:not\(\.metric-card--featured\) strong\s*\{[\s\S]*?font-size:\s*clamp\(1\.18rem,\s*1\.8vw,\s*1\.72rem\);[\s\S]*?\}/,
  'standard metric cards should use a value size that fits short currency values without wrapping',
)

assert.match(
  chartSource + readFileSync(join(repoRoot, 'src/App.tsx'), 'utf8') + metricCardSource,
  /<strong[^>]*aria-label=\{value\}[^>]*title=\{value\}/,
  'metric values should expose the full value to assistive technology as well as hover',
)
