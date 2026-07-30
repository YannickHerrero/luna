import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const entry = fileURLToPath(import.meta.resolve('@earendil-works/pi-coding-agent'))
const packagePath = new URL('../package.json', `file://${entry}`)
const packageJson = JSON.parse(await readFile(packagePath, 'utf8'))
const expected = '0.80.7'
if (packageJson.version !== expected) {
  throw new Error(`Expected Pi ${expected}, found ${packageJson.version}`)
}
console.log(`Pi ${packageJson.version}`)
