import Link from 'next/link';
import { Contributors } from '@/components/contributors';
import { DownloadButtons } from '@/components/download-buttons';
import { HeroRig } from '@/components/hero-rig';
import { Shortcut } from '@/components/key';
import { Shot } from '@/components/shot';
import { asset, site } from '@/lib/site';

export default function HomePage() {
  return (
    <main className="lp">
      <section className="lp-wrap lp-hero">
        <div className="lp-identity">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            className="lp-appicon"
            src={asset('/icon.png')}
            alt=""
            width={256}
            height={256}
          />
          <h1>{site.name}</h1>
        </div>

        <p className="lp-lede">
          Organize thousands of phone photos and videos with your keyboard.{' '}
          <span>
            A desktop app for the folder of IMG_1234 files you have been avoiding
            since 2019.
          </span>
        </p>

        <HeroRig />

        <DownloadButtons />
      </section>

      {/* Rename and file ------------------------------------------------- */}
      <section className="lp-wrap lp-section">
        <div className="lp-section-head">
          <div className="lp-trigger">
            <Shortcut keys="Enter" action="Save" />
            <Shortcut keys="Ctrl+F" action="Folder" />
          </div>
          <h2>Name it and file it in the same keystroke</h2>
          <p>
            One file fills the screen. Type a name, press <strong>Enter</strong>,
            and the next one loads. <strong>Ctrl+F</strong> arms a subfolder, so
            every following Enter renames <em>and</em> moves until you press Esc.
            Live Photo pairs travel together: rename the <code>.heic</code> and
            the <code>.mov</code> beside it gets the same name.
          </p>
        </div>

        <Shot
          src="/screenshots/workspace.png"
          alt="The photo workspace with the rename field and the shortcut list on the right"
          caption="The shortcut list never leaves the screen. That is the whole interface."
          width={2640}
          height={2240}
          narrow
        />

        <div className="lp-panel">
          <b>2024-portugal/</b>
          <br />
          ├─ <span className="in">trips/portugal/algarve/</span> sunset-at-the-beach.heic
          <br />
          │&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; sunset-at-the-beach.mov{' '}
          <span className="in">← the Live Photo pair, moved with it</span>
          <br />
          ├─ <span className="in">paperwork/</span> boarding-pass.jpg
          <br />
          └─ <span className="gone">_deleted/</span> IMG_4519.HEIC
        </div>
      </section>

      {/* Trim -------------------------------------------------------------- */}
      <section className="lp-wrap lp-section">
        <div className="lp-section-head">
          <div className="lp-trigger">
            <Shortcut keys={['[', ']']} action="Trim start / end" />
            <Shortcut keys="Enter" action="Apply and save" />
          </div>
          <h2>Cut the dead ten seconds off a clip without re-encoding it</h2>
          <p>
            Scrub to where the video should start, press <strong>[</strong>, scrub
            to the end, press <strong>]</strong>. Saving runs an FFmpeg stream
            copy: the frames that survive are the original frames, so a 40-second
            clip is written in about a second and the quality is untouched.
          </p>
        </div>

        <Shot
          src="/screenshots/workspace-video.png"
          alt="The video workspace showing the trim range markers under the player"
          caption="Trim markers sit under the player. Enter applies the trim and saves."
          width={2640}
          height={2240}
          narrow
        />
      </section>

      {/* Batch ------------------------------------------------------------- */}
      <section className="lp-wrap lp-section">
        <div className="lp-section-head">
          <div className="lp-trigger">
            <Shortcut keys="Ctrl+B" action="Batch optimize" />
          </div>
          <h2>Shrink a whole folder in one pass</h2>
          <p>
            Pick the files, pick a codec and a resolution, and let it run into a
            new folder. Originals stay where they are unless you say otherwise.
          </p>
        </div>

        <div className="lp-shots-2">
          <Shot
            src="/screenshots/batch-select.png"
            alt="Batch panel with a folder of clips and photos selected"
            caption="1 · Select — clips and photos, or just one of the two."
            width={2200}
            height={1514}
          />
          <Shot
            src="/screenshots/batch-settings.png"
            alt="Batch settings showing codec, quality, resolution, audio and output options"
            caption="2 · Settings — H.265, H.264, AV1, or JPEG / WebP / AVIF for photos."
            width={2200}
            height={1514}
          />
        </div>

        <div className="lp-panel">
          <b>What it does while you are away</b>
          <br />
          <span className="out">GPU</span> NVENC, Quick Sync, AMF, VideoToolbox and
          VAAPI are used when the device has them. A job that fails on the GPU
          retries on the CPU and says so in the report.
          <br />
          <span className="out">Estimate</span> Three-second samples are encoded
          first, so the predicted output size comes from your files, not a table.
          <br />
          <span className="out">Resume</span> Every finished file is checkpointed.
          Close the app mid-run and the next launch picks up the pending ones.
          <br />
          <span className="out">Dates</span> The EXIF block is carried into the
          JPEG, PNG or WebP that comes out. AVIF has nowhere to put it, and the
          settings panel says so before you start.
        </div>

        <Shot
          src="/screenshots/batch-done.png"
          alt="Finished batch job listing the space saved for each file"
          caption="3 · Run — 927.3 MB down to 243.9 MB across twelve files, with the one skip named."
          width={2200}
          height={1514}
        />
      </section>

      {/* Safety ------------------------------------------------------------ */}
      <section className="lp-wrap lp-section">
        <div className="lp-section-head">
          <div className="lp-trigger">
            <Shortcut keys="Ctrl+D" action="Delete" danger />
            <Shortcut keys="Ctrl+Z" action="Undo" />
          </div>
          <h2>Delete is a move, not a deletion</h2>
          <p>
            <strong>Ctrl+D</strong> moves the file to <code>_deleted/</code> inside
            the folder you opened. Not the system Trash, not gone. Review it
            whenever you like, or press <strong>Ctrl+Z</strong> and put it back.
            Renaming and moving never rewrite file contents, so capture dates and
            EXIF survive the whole session.
          </p>
        </div>
      </section>

      {/* Download ---------------------------------------------------------- */}
      <section className="lp-wrap lp-section">
        <div className="lp-section-head">
          <p className="lp-eyebrow">Download</p>
          <h2>Two builds, same app</h2>
          <p>
            Trimming and batch optimizing need FFmpeg. The choice is whether it
            ships with the installer or you already have it.
          </p>
        </div>

        <div className="lp-editions">
          <div className="lp-edition">
            <h3>
              Standard <span>FFmpeg included</span>
            </h3>
            <p>
              FFmpeg is bundled. Install it, open it, everything works. This is
              the one to pick if you are not sure.
            </p>
          </div>
          <div className="lp-edition">
            <h3>
              Lite <span>~50 MB smaller</span>
            </h3>
            <p>
              Uses the FFmpeg already on your PATH. Renaming and organizing work
              without it; trimming and batch need{' '}
              <code>winget install Gyan.FFmpeg</code> or{' '}
              <code>brew install ffmpeg</code> once.
            </p>
          </div>
        </div>

        <DownloadButtons variant="plain" />

        <p className="lp-section-link">
          <Link href="/docs/install">
            First-launch warnings and FFmpeg setup →
          </Link>
        </p>
      </section>

      <Contributors />

      <footer className="lp-wrap lp-footer">
        <span>
          MIT licensed. Built with Tauri 2 by{' '}
          <a href="https://github.com/FerranVidalBelles">Ferran Vidal Bellés</a>.
        </span>
        <span>
          <a href={site.repo}>Source</a> · <a href={site.coffee}>Buy me a coffee</a>
        </span>
      </footer>
    </main>
  );
}
