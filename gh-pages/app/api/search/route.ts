import { source } from '@/lib/source';
import { createFromSource } from 'fumadocs-core/search/server';

// `staticGET` writes the search index to out/api/search as a plain file, which
// the browser downloads once and queries locally. There is no server here.
export const revalidate = false;
export const { staticGET: GET } = createFromSource(source, {
  language: 'english',
});
