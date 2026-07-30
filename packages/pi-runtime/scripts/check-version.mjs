import { readFile } from 'node:fs/promises'
import { stdout } from 'node:process'
import { fileURLToPath } from 'node:url'

const entry = fileURLToPath(import.meta.resolve('@earendil-works/pi-coding-agent'))
const packagePath = new URL('../package.json', `file://${entry}`)
const packageJson = /** @type {unknown} */ (JSON.parse(await readFile(packagePath, 'utf8')))
if (
  packageJson === null ||
  typeof packageJson !== 'object' ||
  !('version' in packageJson) ||
  typeof packageJson.version !== 'string'
) {
  throw new Error('Pi package does not expose a string version')
}
const expected = '0.80.7'
if (packageJson.version !== expected) {
  throw new Error(`Expected Pi ${expected}, found ${packageJson.version}`)
}
stdout.write(`Pi ${packageJson.version}\n`)
