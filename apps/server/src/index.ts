import { buildApp } from './app.js'
import { loadConfig } from './config.js'

const config = loadConfig()
const app = await buildApp(config)

app.log.warn(`Luna pairing code: ${app.luna.pairingCode}`)
await app.listen({ host: config.bindHost, port: config.port })
