import { mkdir, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { ApiResponseSchemas } from '../src/api.js'

const here = dirname(fileURLToPath(import.meta.url))
const output = resolve(here, '../generated/openapi.json')
const document = {
  openapi: '3.1.0',
  info: { title: 'Luna API', version: '1.0.0' },
  servers: [{ url: '/v1' }],
  paths: {},
  components: { schemas: ApiResponseSchemas },
}

await mkdir(dirname(output), { recursive: true })
await writeFile(output, `${JSON.stringify(document, null, 2)}\n`)
