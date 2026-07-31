import { mkdir, rm, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import {
  Archive,
  ArrowLeft,
  Camera,
  Check,
  ChevronDown,
  Circle,
  CircleStop,
  ListChecks,
  Mic,
  Minus,
  Moon,
  Paperclip,
  Plus,
  Search,
  Send,
  Settings,
  Sun,
  TriangleAlert,
  X,
} from 'lucide-react'
import sharp from 'sharp'

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const assetCatalog = resolve(webRoot, '../ios/Luna/Resources/Assets.xcassets')
const watchAssetCatalog = resolve(webRoot, '../ios/LunaWatch/Resources/Assets.xcassets')

const icons = {
  Archive,
  ArrowLeft,
  Camera,
  Check,
  ChevronDown,
  Circle,
  CircleStop,
  ListChecks,
  Mic,
  Minus,
  Moon,
  Paperclip,
  Plus,
  Search,
  Send,
  Settings,
  Sun,
  TriangleAlert,
  X,
}

await rm(assetCatalog, { recursive: true, force: true })
await mkdir(assetCatalog, { recursive: true })
await writeJSON(resolve(assetCatalog, 'Contents.json'), {
  info: { author: 'xcode', version: 1 },
})

for (const [name, component] of Object.entries(icons)) {
  const imageSet = resolve(assetCatalog, `Lucide${name}.imageset`)
  await mkdir(imageSet, { recursive: true })
  const svg = renderToStaticMarkup(
    createElement(component, {
      xmlns: 'http://www.w3.org/2000/svg',
      width: 24,
      height: 24,
      color: '#000000',
      strokeWidth: 2,
    }),
  )
  const fileName = `${name}.svg`
  await writeFile(resolve(imageSet, fileName), `${svg}\n`)
  await writeJSON(resolve(imageSet, 'Contents.json'), {
    images: [{ filename: fileName, idiom: 'universal' }],
    info: { author: 'xcode', version: 1 },
    properties: {
      'preserves-vector-representation': true,
      'template-rendering-intent': 'template',
    },
  })
}

const appIconSet = resolve(assetCatalog, 'AppIcon.appiconset')
await mkdir(appIconSet, { recursive: true })
await sharp(resolve(webRoot, 'assets/app-icon.png'))
  .resize(1024, 1024)
  .png()
  .toFile(resolve(appIconSet, 'AppIcon.png'))
await writeJSON(resolve(appIconSet, 'Contents.json'), {
  images: [{ filename: 'AppIcon.png', idiom: 'universal', platform: 'ios', size: '1024x1024' }],
  info: { author: 'xcode', version: 1 },
})

await rm(watchAssetCatalog, { recursive: true, force: true })
await mkdir(watchAssetCatalog, { recursive: true })
await writeJSON(resolve(watchAssetCatalog, 'Contents.json'), {
  info: { author: 'xcode', version: 1 },
})
const watchAppIconSet = resolve(watchAssetCatalog, 'AppIcon.appiconset')
await mkdir(watchAppIconSet, { recursive: true })
await sharp(resolve(webRoot, 'assets/app-icon.png'))
  .resize(1024, 1024)
  .png()
  .toFile(resolve(watchAppIconSet, 'AppIcon.png'))
await writeJSON(resolve(watchAppIconSet, 'Contents.json'), {
  images: [{ filename: 'AppIcon.png', idiom: 'universal', platform: 'watchos', size: '1024x1024' }],
  info: { author: 'xcode', version: 1 },
})

/**
 * @param {string} path
 * @param {unknown} value
 */
async function writeJSON(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`)
}
