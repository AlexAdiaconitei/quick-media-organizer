import defaultMdxComponents from 'fumadocs-ui/mdx';
import { Step, Steps } from 'fumadocs-ui/components/steps';
import type { MDXComponents } from 'mdx/types';
import { Key, Shortcut } from '@/components/key';
import { CloneCommand, ReleasesLink, RepoLink } from '@/components/repo';
import { Shot } from '@/components/shot';

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    Step,
    Steps,
    CloneCommand,
    Key,
    ReleasesLink,
    RepoLink,
    Shortcut,
    Shot,
    ...components,
  } satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
