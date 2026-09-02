# Landing page and documentation

The public site for Quick Media Organizer, built with
[Fumadocs](https://fumadocs.dev/) on Next.js and exported as static HTML.

Published to GitHub Pages for whichever repository builds it, at
`https://<owner>.github.io/<repo>/`. Nothing here names a repository: the
workflow passes `${{ github.repository }}` as `NEXT_PUBLIC_REPO_SLUG` and the
base path comes from `actions/configure-pages`, so a fork's site links to the
fork's own releases and lists the fork's own contributors.

This is a standalone project. It has its own `package.json`, its own lockfile,
and a `pnpm-workspace.yaml` that stops pnpm from walking up into the desktop
app, so installing here never touches the Tauri build.

## Running it

```bash
cd gh-pages
pnpm install
pnpm dev        # http://localhost:3000
pnpm build      # static export into out/
pnpm typecheck
```

`pnpm build` writes a plain HTML tree with no server behind it. To check the
build the way GitHub Pages serves it, build with the deployment prefix and serve
it from a parent directory:

```bash
NEXT_PUBLIC_BASE_PATH=/quick-media-organizer pnpm build
```

## Deployment

`.github/workflows/docs.yml` runs on pushes to `main` that touch `gh-pages/**`,
and on demand from the Actions tab. It reads the base path from
`actions/configure-pages`, so a fork or a custom domain gets the right prefix
with no edit here.

Enable it once per repository: **Settings → Pages → Build and deployment →
Source → GitHub Actions**.

## Layout

| Path | What it is |
|---|---|
| `app/(home)/page.tsx` | The landing page |
| `components/hero-rig.tsx` | The interactive shortcut demo in the hero |
| `content/docs/*.mdx` | Documentation pages; `meta.json` sets their order |
| `app/global.css` | Design tokens, taken from the app's `src/app.css` |
| `public/screenshots/` | Copies of `docs/screenshots/` from the repository root |

Search is Fumadocs' static index: the browser downloads `out/api/search` once
and queries it locally, since Pages has no server to ask.

## Keeping screenshots in sync

The site serves its own copy of the screenshots. After regenerating them at the
repository root with `pnpm capture-screenshots`, copy them across:

```bash
cp docs/screenshots/*.png gh-pages/public/screenshots/
```
