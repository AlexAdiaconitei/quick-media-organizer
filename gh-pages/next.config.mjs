import { createMDX } from 'fumadocs-mdx/next';

// GitHub Pages serves a project site from https://<user>.github.io/<repo>, so
// every link and asset needs that prefix. The workflow passes the repository
// name; a fork gets its own prefix without editing this file. Local `next dev`
// leaves it empty and the site lives at the root.
// A user site or a custom domain yields '/' here, which Next rejects.
const basePath = (process.env.NEXT_PUBLIC_BASE_PATH ?? '').replace(/\/$/, '');

/** @type {import('next').NextConfig} */
const config = {
  // No Node server on Pages: `next build` writes a plain HTML tree into out/.
  output: 'export',
  basePath,
  // Pages has no image optimizer.
  images: { unoptimized: true },
  // Emits /docs/index.html instead of /docs.html, which is what Pages resolves
  // when someone opens a directory URL.
  trailingSlash: true,
  reactStrictMode: true,
};

const withMDX = createMDX();

export default withMDX(config);
