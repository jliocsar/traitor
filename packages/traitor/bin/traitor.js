#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { constants } from 'node:os'

// As a Claude Code Stop hook, exit 2 would block every stop on an error Claude can't fix, so
// there a traitor that can't run exits 1, a non-blocking error.
const runsAsHook = process.argv.some(
  (argument) => argument === 'claude-code-hook' || argument === '--format=claude-code-hook',
)
const EXIT_COULD_NOT_RUN = runsAsHook ? 1 : 2

const require = createRequire(import.meta.url)
const platformPackage = `@jliocsar/traitor-${process.platform}-${process.arch}`
const executable = process.platform === 'win32' ? 'traitor.exe' : 'traitor'

function findBinary() {
  try {
    return require.resolve(`${platformPackage}/${executable}`)
  } catch {
    console.error(
      `traitor: ${platformPackage} isn't installed, so there's no traitor binary for ${process.platform} ${process.arch}. ` +
        'Reinstall without skipping optional dependencies, or this platform has no build yet.',
    )
    process.exit(EXIT_COULD_NOT_RUN)
  }
}

const result = spawnSync(findBinary(), process.argv.slice(2), { stdio: 'inherit' })
if (result.error) {
  console.error(`traitor: couldn't start ${platformPackage}'s binary: ${result.error.message}`)
  process.exit(EXIT_COULD_NOT_RUN)
}
if (result.signal) {
  process.exit(128 + (constants.signals[result.signal] ?? 0))
}
process.exit(result.status ?? EXIT_COULD_NOT_RUN)
