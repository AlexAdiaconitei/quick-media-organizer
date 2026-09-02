import type { ReactNode } from 'react';

/** A single keycap, for use inside prose. */
export function Key({ children }: { children: ReactNode }) {
  return <kbd className="keycap">{children}</kbd>;
}

/**
 * The app's shortcut chip: the key on the left, what it does on the right.
 * It is the one control that stays on screen in every screenshot, so it also
 * carries the section headings here.
 */
export function Shortcut({
  keys,
  action,
  danger = false,
}: {
  keys: string | string[];
  action: string;
  danger?: boolean;
}) {
  const caps = Array.isArray(keys) ? keys : [keys];

  return (
    <span className={danger ? 'chip is-danger' : 'chip'}>
      {caps.map((cap) => (
        <span className="chip-key" key={cap}>
          {cap}
        </span>
      ))}
      <span className="chip-text">{action}</span>
    </span>
  );
}
