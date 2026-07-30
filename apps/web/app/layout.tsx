import type { Metadata, Viewport } from 'next'
import '@luna/design-tokens/tokens.css'
import './globals.css'

export const metadata: Metadata = {
  title: 'Luna',
  description: 'Persistent Pi conversations from every device.',
  applicationName: 'Luna',
  manifest: '/manifest.webmanifest',
  icons: { icon: '/icon.svg', apple: '/icon.svg' },
}

export const viewport: Viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: light)', color: '#eff1f5' },
    { media: '(prefers-color-scheme: dark)', color: '#11111b' },
  ],
  viewportFit: 'cover',
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>{children}</body>
    </html>
  )
}
