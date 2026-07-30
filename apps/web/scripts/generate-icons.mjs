import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import sharp from 'sharp'

const root = resolve(import.meta.dirname, '..')
const source = await readFile(resolve(root, 'assets/app-icon.png'))
for (const [name, size] of [
  ['icon-192.png', 192],
  ['icon-512.png', 512],
  ['apple-touch-icon.png', 180],
]) {
  await sharp(source)
    .resize(size, size)
    .png()
    .toFile(resolve(root, 'public', name))
}
