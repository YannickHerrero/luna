import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import sharp from 'sharp'
import { describe, expect, it } from 'vitest'

const root = resolve(import.meta.dirname, '..')

describe('PWA assets', () => {
  it('publishes installable PNG icons', async () => {
    const manifest = JSON.parse(
      await readFile(resolve(root, 'public/manifest.webmanifest'), 'utf8'),
    ) as { icons: { src: string; sizes: string }[] }
    expect(manifest.icons).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ src: '/icon-192.png', sizes: '192x192' }),
        expect.objectContaining({ src: '/icon-512.png', sizes: '512x512' }),
      ]),
    )
    await expect(sharp(resolve(root, 'assets/app-icon.png')).metadata()).resolves.toMatchObject({
      width: 1254,
      height: 1254,
    })
    await expect(sharp(resolve(root, 'public/icon-192.png')).metadata()).resolves.toMatchObject({
      width: 192,
      height: 192,
    })
    await expect(sharp(resolve(root, 'public/icon-512.png')).metadata()).resolves.toMatchObject({
      width: 512,
      height: 512,
    })
  })

  it('never caches private API responses', async () => {
    const worker = await readFile(resolve(root, 'public/sw.js'), 'utf8')
    expect(worker).toContain("url.pathname.startsWith('/v1/')")
    expect(worker).toContain("'/icon-192.png'")
    expect(worker).toContain('caches.open(CACHE)')
  })
})
