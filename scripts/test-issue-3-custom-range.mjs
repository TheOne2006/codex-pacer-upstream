import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import assert from 'node:assert/strict'

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)))
const appSource = readFileSync(join(repoRoot, 'src/App.tsx'), 'utf8')
const apiSource = readFileSync(join(repoRoot, 'src/app/api.ts'), 'utf8')
const typeSource = readFileSync(join(repoRoot, 'src/app/types.ts'), 'utf8')
const i18nSource = readFileSync(join(repoRoot, 'src/app/i18n.ts'), 'utf8')

assert.match(
  typeSource,
  /\|\s*'custom'/,
  'overview buckets should include a dashboard custom range option',
)

assert.match(
  apiSource,
  /customStart\?:\s*string\s*\|\s*null[\s\S]*customEnd\?:\s*string\s*\|\s*null/,
  'dashboard API should accept custom range start and end dates',
)

assert.match(
  apiSource,
  /customStart:\s*bucket === 'custom' \? customStart \?\? null : null/,
  'dashboard API should send customStart only for the custom bucket',
)

assert.match(
  apiSource,
  /customEnd:\s*bucket === 'custom' \? customEnd \?\? null : null/,
  'dashboard API should send customEnd only for the custom bucket',
)

assert.match(
  appSource,
  /const \[customStart,\s*setCustomStart\] = useState\(todayInputValue\(\)\)/,
  'dashboard should keep custom range start in local state',
)

assert.match(
  appSource,
  /const \[customEnd,\s*setCustomEnd\] = useState\(todayInputValue\(\)\)/,
  'dashboard should keep custom range end in local state',
)

assert.match(
  appSource,
  /name="customRangeStartDate"[\s\S]*type="date"[\s\S]*value=\{customStart\}/,
  'custom range controls should expose a labelled start date input',
)

assert.match(
  appSource,
  /name="customRangeEndDate"[\s\S]*type="date"[\s\S]*value=\{customEnd\}/,
  'custom range controls should expose a labelled end date input',
)

assert.match(
  appSource,
  /loadDashboard\([\s\S]*(?:customStart|requestCustomStart),[\s\S]*(?:customEnd|requestCustomEnd),[\s\S]*\)/,
  'dashboard load should pass the custom range to the backend',
)

assert.match(
  i18nSource,
  /custom:\s*'自定义'/,
  'Chinese bucket labels should include custom range',
)

assert.match(
  i18nSource,
  /custom:\s*'Custom'/,
  'English bucket labels should include custom range',
)
