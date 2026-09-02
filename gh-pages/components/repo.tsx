import { DynamicCodeBlock } from 'fumadocs-ui/components/dynamic-codeblock';
import type { ReactNode } from 'react';
import { repoSlug, site } from '@/lib/site';

/**
 * MDX shortcuts for anything that names the repository. The docs never spell a
 * slug out, so a fork's site sends people to the fork's own releases and clone
 * URL without a single edit to the content.
 */

export function RepoLink({ children }: { children: ReactNode }) {
  return <a href={site.repo}>{children}</a>;
}

export function ReleasesLink({ children }: { children: ReactNode }) {
  return <a href={site.releases}>{children}</a>;
}

export function CloneCommand() {
  const repo = repoSlug.split('/')[1];

  return (
    <DynamicCodeBlock
      lang="bash"
      code={[
        `git clone https://github.com/${repoSlug}.git`,
        `cd ${repo}`,
        'pnpm install',
        'pnpm dev          # runs the desktop app',
      ].join('\n')}
    />
  );
}
