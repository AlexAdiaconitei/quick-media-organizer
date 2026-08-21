# Quick Media Organizer

**Organize thousands of phone photos and videos with your keyboard — no mouse required.**

![MIT License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)
![Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange)

[🇪🇸 Leer en español](README.es.md)

<p align="center">
  <img src="docs/screenshots/welcome.png" alt="Welcome screen" width="720" />
</p>

---

## Why I built this

Honestly? **I needed it myself.**

I had a folder full of phone backups — thousands of `IMG_1234…` files mixed with videos — and every tool I tried felt slow, bloated, or wrong for the job. I didn't want a full photo library. I just wanted to **rename**, **sort into folders**, **trim the bad parts of videos**, and **move on** — as fast as possible, with my hands on the keyboard.

So I built **Quick Media Organizer**. It's not a startup pitch; it's a tool I use every day. I'm sharing it open source because I hope it helps someone else stuck with the same mess.

If it saves you time, I'd genuinely appreciate a [coffee ☕](https://buymeacoffee.com/ferran_vidal). It helps me keep improving it in my spare time.

---

## What it does

- **Rename** photos and videos in seconds with `Enter`
- **Move** files into subfolders like `gym/`, `trips/portugal/`, `paperwork/` with `Ctrl+F`
- **Trim videos losslessly** (FFmpeg stream copy — no re-encoding) before saving
- **Delete safely** to `_deleted/` inside your folder — never permanent, always undoable
- **Skip**, **navigate**, and **undo** without touching the mouse
- **Live Photos** (`.heic` + `.mov`) move, rename, and delete together
- Original **EXIF dates** and file timestamps are preserved

<p align="center">
  <img src="docs/screenshots/workspace.png" alt="Photo workspace with keyboard shortcuts" width="660" />
</p>

<p align="center">
  <img src="docs/screenshots/workspace-video.png" alt="Video workspace with lossless trim controls" width="660" />
</p>

---

## Download

Get the latest release for your platform:

**[GitHub Releases →](https://github.com/FerranVidalBelles/quick-media-organizer/releases)**

macOS (`.dmg`) · Windows (`.msi` / `.exe`)

### First launch (unsigned builds)

| OS | What you may see | What to do |
|----|------------------|------------|
| **macOS** | "Unidentified developer" | Right-click the app → **Open** → confirm once |
| **Windows** | SmartScreen warning | Click **More info** → **Run anyway** |

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Rename or save to armed folder *(also applies pending video trim)* |
| `Ctrl+F` / `⌘F` | Choose or create a subfolder |
| `Ctrl+D` / `⌘D` | Move to `_deleted/` *(works while typing)* |
| `Delete` | Move to `_deleted/` *(when not typing)* |
| `⌘⇧Space` / `Ctrl+Space` | Skip |
| `←` `→` | Previous / next |
| `Ctrl+Z` / `⌘Z` | Undo |
| `Ctrl+M` / `⌘M` | Toggle metadata |
| `Ctrl+O` / `⌘O` | Options |
| `?` | Help |
| `[` `]` | Set video trim start / end |
| `Esc` | Cancel armed folder / close modal |

Shortcuts stay **always visible** in the bottom bar.

---

## FAQ

**Does Delete erase files forever?**  
No. Files go to `_deleted/` inside your media folder. Review them anytime.

**Will organizing change capture dates?**  
No. EXIF metadata and original timestamps are preserved.

**Videos and Live Photos?**  
Yes. Videos preview in-app and can be trimmed losslessly. Live Photo pairs stay in sync.

**HEIC on Windows?**  
Organizing works. Preview may fall back to metadata on some setups.

**Standard or Lite download?**  
Every release ships both:

| | Size (Windows setup) | FFmpeg |
|---|---|---|
| Standard | ~57 MB | included, nothing to install |
| Lite (`-lite`) | ~5 MB | you install it yourself |

Renaming and organizing work in both. Trimming and batch optimizing need
FFmpeg, so on Lite install it once with `winget install Gyan.FFmpeg` (Windows),
`brew install ffmpeg` (macOS) or `apt install ffmpeg` (Linux), then reopen the
app. A bundled copy always wins over one found on your PATH, since it is the
build the app was tested against.

Building locally: `pnpm build` produces the Lite installer, `pnpm build:full`
downloads FFmpeg and produces the Standard one.

FFmpeg is GPL-licensed and stays a separate program, invoked as a subprocess;
this app remains MIT. `FFMPEG-LICENSE.txt` travels with the Standard
installer.

---

## Build from source

Requirements: [Node.js](https://nodejs.org/) 20+, [Rust](https://rustup.rs/)

```bash
git clone https://github.com/FerranVidalBelles/quick-media-organizer.git
cd quick-media-organizer
npm install
npm run tauri dev
```

Build installers:

```bash
npm run tauri build
```

---

## Support & contact

This is a personal passion project born from a real need. If you find it useful:

- ☕ **[Buy me a coffee](https://buymeacoffee.com/ferran_vidal)** — helps me maintain and improve it
- ✉️ **Email:** [ferranvidaldev@gmail.com](mailto:ferranvidaldev@gmail.com)
- 💼 **LinkedIn:** [ferran-vidal-belles](https://www.linkedin.com/in/ferran-vidal-belles/)

Issues and PRs welcome on GitHub. I can't promise instant support, but I read everything.

---

## License

MIT — see [LICENSE](LICENSE).

**Author:** [Ferran Vidal Bellés](https://github.com/FerranVidalBelles)
