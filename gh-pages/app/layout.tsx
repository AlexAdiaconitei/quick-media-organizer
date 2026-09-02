import './global.css';
import { RootProvider } from 'fumadocs-ui/provider/next';
import { IBM_Plex_Mono } from 'next/font/google';
import type { Metadata } from 'next';
import type { ReactNode } from 'react';
import StaticSearchDialog from '@/components/search';
import { asset, site } from '@/lib/site';

// Filenames and keycaps are what this product is about, so the mono face does
// the display work. Body copy keeps the app's own system stack.
const plexMono = IBM_Plex_Mono({
  subsets: ['latin'],
  weight: ['400', '500'],
  variable: '--font-plex-mono',
  display: 'swap',
});

export const metadata: Metadata = {
  title: {
    default: `${site.name} — keyboard-first photo and video sorting`,
    template: `%s — ${site.name}`,
  },
  description: site.tagline,
  icons: { icon: asset('/favicon.png') },
  openGraph: {
    title: site.name,
    description: site.tagline,
    type: 'website',
  },
};

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${plexMono.variable} dark`} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <RootProvider
          theme={{ forcedTheme: 'dark', defaultTheme: 'dark', enabled: true }}
          search={{ SearchDialog: StaticSearchDialog }}
        >
          {children}
        </RootProvider>
      </body>
    </html>
  );
}
