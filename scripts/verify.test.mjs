import assert from 'node:assert/strict'
import test from 'node:test'

import { runVerification, verificationLanes } from './verify.mjs'

const lane = (id, dependencies = []) => ({ id, command: id, args: [], dependencies })

test('runs independent verification lanes concurrently after their dependencies', async () => {
  const events = []
  let active = 0
  let maximumActive = 0
  const report = await runVerification({
    profile: 'test',
    lanes: [lane('prepare'), lane('one', ['prepare']), lane('two', ['prepare'])],
    concurrency: 2,
    onLaneStart: null,
    onLaneFinish: null,
    execute: async (item) => {
      events.push(`start:${item.id}`)
      active += 1
      maximumActive = Math.max(maximumActive, active)
      await new Promise((resolveDelay) => setTimeout(resolveDelay, item.id === 'prepare' ? 1 : 10))
      active -= 1
      events.push(`finish:${item.id}`)
      return 0
    },
  })

  assert.equal(report.status, 'passed')
  assert.equal(maximumActive, 2)
  assert.deepEqual(events.slice(0, 2), ['start:prepare', 'finish:prepare'])
  assert.deepEqual(new Set(events.slice(2, 4)), new Set(['start:one', 'start:two']))
})

test('skips dependent lanes after a failure while retaining independent results', async () => {
  const executed = []
  const report = await runVerification({
    profile: 'test',
    lanes: [lane('failed'), lane('blocked', ['failed']), lane('independent')],
    concurrency: 2,
    onLaneStart: null,
    onLaneFinish: null,
    execute: async (item) => {
      executed.push(item.id)
      return item.id === 'failed' ? 7 : 0
    },
  })

  assert.equal(report.status, 'failed')
  assert.deepEqual(new Set(executed), new Set(['failed', 'independent']))
  assert.equal(report.lanes.find((item) => item.id === 'failed').exitCode, 7)
  assert.equal(report.lanes.find((item) => item.id === 'blocked').status, 'skipped')
  assert.equal(report.lanes.find((item) => item.id === 'independent').status, 'passed')
})

test('marks running verification as cancelled after abort', async () => {
  const controller = new AbortController()
  const reportPromise = runVerification({
    profile: 'test',
    lanes: [lane('long-running')],
    onLaneStart: null,
    onLaneFinish: null,
    signal: controller.signal,
    execute: async (_item, { signal }) =>
      new Promise((resolveExecution) => {
        signal.addEventListener('abort', () => resolveExecution(130), { once: true })
      }),
  })

  controller.abort()
  const report = await reportPromise
  assert.equal(report.status, 'cancelled')
  assert.equal(report.lanes[0].status, 'cancelled')
})

test('defines a conservative full graph with exclusive browser and release builds', () => {
  const lanes = verificationLanes('full')
  const browser = lanes.find((item) => item.id === 'browser-e2e')
  const release = lanes.find((item) => item.id === 'release-build')

  assert.deepEqual(
    new Set(browser.dependencies),
    new Set(['web-tests', 'rust-tests', 'typecheck', 'lint']),
  )
  assert.deepEqual(lanes.find((item) => item.id === 'rust-tests').dependencies, ['runner-tests'])
  assert.deepEqual(lanes.find((item) => item.id === 'lint').dependencies, ['rust-tests'])
  assert.deepEqual(release.dependencies, ['browser-e2e'])
  assert.equal(
    verificationLanes('code').some((item) => item.id === 'browser-e2e'),
    false,
  )
})

test('rejects verification graphs with missing dependencies', async () => {
  await assert.rejects(
    runVerification({
      profile: 'test',
      lanes: [lane('broken', ['missing'])],
      execute: async () => 0,
    }),
    /unknown dependency missing/,
  )
})
