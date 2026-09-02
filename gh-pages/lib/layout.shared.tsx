import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { site } from '@/lib/site';
import { Wordmark } from '@/components/wordmark';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: <Wordmark />,
      transparentMode: 'top',
    },
    githubUrl: site.repo,
    links: [
      { text: 'Docs', url: '/docs', active: 'nested-url' },
      { text: 'Download', url: site.releases, external: true },
    ],
  };
}
