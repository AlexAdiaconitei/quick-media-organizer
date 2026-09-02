'use client';

import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * The hero is the product: a queue of files and a row of shortcuts. Press the
 * real key and the real thing happens, minus the filesystem. Messages are the
 * strings the app actually shows (src/lib/i18n.ts).
 */

type Status = 'pending' | 'renamed' | 'moved' | 'trashed';

interface Item {
  original: string;
  suggested: string;
  meta: string;
}

interface Entry {
  status: Status;
  name: string;
  folder: string | null;
}

interface LedgerLine {
  id: number;
  from: string;
  to: string;
  kind: Status;
}

const QUEUE: Item[] = [
  {
    original: 'IMG_4521.HEIC',
    suggested: 'sunset-at-the-beach.heic',
    meta: 'HEIC · Live Photo · 2.3 MB',
  },
  {
    original: 'VID_20240813_101122.mp4',
    suggested: 'algarve-cliffs.mp4',
    meta: 'MP4 · 00:41 · 222.8 MB',
  },
  {
    original: 'IMG_4530.HEIC',
    suggested: 'dinner-in-lagos.heic',
    meta: 'HEIC · 2.7 MB',
  },
  {
    original: 'IMG_4544.jpg',
    suggested: 'boarding-pass.jpg',
    meta: 'JPEG · 3.1 MB',
  },
];

const FOLDERS = ['trips/portugal/algarve', 'paperwork', 'gym'];

const FIRST_INDEX = 2051;
const TOTAL = 2845;

const fresh = (): Entry[] =>
  QUEUE.map((item) => ({ status: 'pending', name: item.original, folder: null }));

type ActionId = 'save' | 'folder' | 'delete' | 'prev' | 'next' | 'undo';

const ACTIONS: { id: ActionId; keys: string[]; label: string; danger?: boolean }[] = [
  { id: 'save', keys: ['Enter'], label: 'Save' },
  { id: 'folder', keys: ['Ctrl+F'], label: 'Folder' },
  { id: 'delete', keys: ['Ctrl+D'], label: 'Delete', danger: true },
  { id: 'prev', keys: ['←'], label: 'Previous' },
  { id: 'next', keys: ['→'], label: 'Next' },
  { id: 'undo', keys: ['Ctrl+Z'], label: 'Undo' },
];

export function HeroRig() {
  const rig = useRef<HTMLDivElement>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const uid = useRef(0);

  const [entries, setEntries] = useState<Entry[]>(fresh);
  const [index, setIndex] = useState(0);
  const [armed, setArmed] = useState<string | null>(null);
  const [folderTurn, setFolderTurn] = useState(0);
  const [status, setStatus] = useState('');
  const [hit, setHit] = useState<ActionId | null>(null);
  const [history, setHistory] = useState<{ index: number; entry: Entry }[]>([]);
  const [ledger, setLedger] = useState<LedgerLine[]>([]);

  const entry = entries[index];
  const item = QUEUE[index];

  const flash = useCallback((id: ActionId) => {
    setHit(id);
    if (flashTimer.current) clearTimeout(flashTimer.current);
    flashTimer.current = setTimeout(() => setHit(null), 220);
  }, []);

  useEffect(
    () => () => {
      if (flashTimer.current) clearTimeout(flashTimer.current);
    },
    [],
  );

  const commit = useCallback(
    (at: number, next: Entry, to: string) => {
      setHistory((past) => [...past.slice(-9), { index: at, entry: entries[at] }]);
      setEntries((all) => all.map((value, i) => (i === at ? next : value)));
      setLedger((lines) =>
        [
          { id: (uid.current += 1), from: QUEUE[at].original, to, kind: next.status },
          ...lines,
        ].slice(0, 3),
      );
    },
    [entries],
  );

  const run = useCallback(
    (id: ActionId) => {
      flash(id);

      switch (id) {
        case 'save': {
          if (armed) {
            commit(
              index,
              { status: 'moved', name: item.suggested, folder: armed },
              `${armed}/${item.suggested}`,
            );
            setStatus('Saved to folder');
            setArmed(null);
          } else {
            commit(
              index,
              { status: 'renamed', name: item.suggested, folder: null },
              item.suggested,
            );
            setStatus('Renamed');
          }
          setIndex((i) => Math.min(i + 1, QUEUE.length - 1));
          break;
        }
        case 'folder': {
          const folder = FOLDERS[folderTurn % FOLDERS.length];
          setFolderTurn((turn) => turn + 1);
          setArmed(folder);
          setStatus('');
          break;
        }
        case 'delete': {
          commit(
            index,
            { status: 'trashed', name: item.original, folder: null },
            `_deleted/${item.original}`,
          );
          setStatus('Moved to _deleted (not system Trash). Press Undo to restore.');
          setArmed(null);
          setIndex((i) => Math.min(i + 1, QUEUE.length - 1));
          break;
        }
        case 'prev': {
          setIndex((i) => Math.max(i - 1, 0));
          setStatus('');
          break;
        }
        case 'next': {
          setIndex((i) => Math.min(i + 1, QUEUE.length - 1));
          setStatus('');
          break;
        }
        case 'undo': {
          const last = history.at(-1);
          if (!last) {
            setStatus('This action can no longer be undone.');
            break;
          }
          setHistory((past) => past.slice(0, -1));
          setEntries((all) => all.map((value, i) => (i === last.index ? last.entry : value)));
          setLedger((lines) => lines.slice(1));
          setIndex(last.index);
          setStatus('Undone');
          break;
        }
      }
    },
    [armed, commit, flash, folderTurn, history, index, item],
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    const mod = event.ctrlKey || event.metaKey;

    if (event.key === 'Escape' && armed) {
      event.preventDefault();
      setArmed(null);
      setStatus('');
      return;
    }
    if (event.key === 'Enter' && !mod) {
      event.preventDefault();
      run('save');
      return;
    }
    if (mod && event.key.toLowerCase() === 'f') {
      event.preventDefault();
      run('folder');
      return;
    }
    if (mod && event.key.toLowerCase() === 'd') {
      event.preventDefault();
      run('delete');
      return;
    }
    if (mod && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      run('undo');
      return;
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      run('prev');
      return;
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      run('next');
    }
  };

  // Land with the demo already listening. Skipped when the visitor arrived
  // partway down the page, so a deep link keeps its place.
  useEffect(() => {
    if (window.scrollY > 0) return;
    rig.current?.focus({ preventScroll: true });
  }, []);

  const nowClass =
    entry.status === 'trashed' ? 'lp-rename-now is-trashed' : 'lp-rename-now';

  const nowText =
    entry.status === 'trashed'
      ? `_deleted/${entry.name}`
      : entry.status === 'moved'
        ? `${entry.folder}/${entry.name}`
        : entry.name;

  return (
    <div
      ref={rig}
      className="lp-rig"
      tabIndex={0}
      role="group"
      aria-label="Keyboard demo. Use Enter, Ctrl+F, Ctrl+D, Ctrl+Z and the arrow keys."
      onKeyDown={onKeyDown}
    >
      <div className="lp-rig-head">
        <span>
          {FIRST_INDEX + index} / {TOTAL}
        </span>
        <span>{item.meta}</span>
      </div>

      {armed ? (
        <p className="lp-rig-banner">Folder mode: {armed} — Esc to cancel</p>
      ) : null}

      <p className="lp-rename">
        <span className={nowClass}>{nowText}</span>
      </p>

      <p className="lp-status" role="status" aria-live="polite">
        {status}
      </p>

      {ledger.length > 0 ? (
        <ul className="lp-ledger">
          {ledger.map((line) => (
            <li key={line.id} className={`is-${line.kind}`}>
              <s>{line.from}</s>
              <span aria-hidden="true">→</span>
              <span>{line.to}</span>
            </li>
          ))}
        </ul>
      ) : null}

      <div className="lp-keys">
        {ACTIONS.map((action) => (
          <button
            type="button"
            key={action.id}
            className={[
              'chip',
              action.danger ? 'is-danger' : '',
              hit === action.id ? 'is-hit' : '',
            ]
              .filter(Boolean)
              .join(' ')}
            onClick={() => run(action.id)}
          >
            {action.keys.map((cap) => (
              <span className="chip-key" key={cap}>
                {cap}
              </span>
            ))}
            <span className="chip-text">{action.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
