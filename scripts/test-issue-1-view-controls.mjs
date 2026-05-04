import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import assert from 'node:assert/strict'

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)))
const chartSource = readFileSync(join(repoRoot, 'src/components/ModelShareChart.tsx'), 'utf8')
const cssSource = readFileSync(join(repoRoot, 'src/styles.css'), 'utf8')

assert.match(
  chartSource,
  /className="chart-control-group"[\s\S]*aria-label=\{t\.charts\.dimensionControlLabel\}/,
  'distribution chart should expose the dimension controls as their own labelled group',
)

assert.match(
  chartSource,
  /className="chart-control-group"[\s\S]*aria-label=\{t\.charts\.metricControlLabel\}/,
  'distribution chart should expose the metric controls as their own labelled group',
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
  /\.chart-heading h3\s*\{[\s\S]*?white-space:\s*nowrap;[\s\S]*?\}/,
  'chart titles should not wrap short labels like model share',
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
