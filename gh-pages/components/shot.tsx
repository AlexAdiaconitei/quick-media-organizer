import { asset } from '@/lib/site';

/**
 * A screenshot in a frame that borrows the app's own border, radius and
 * shadow, so the captured window reads as a continuation of the page.
 */
export function Shot({
  src,
  alt,
  caption,
  width,
  height,
  narrow = false,
}: {
  src: string;
  alt: string;
  caption?: string;
  width: number;
  height: number;
  /** Portrait window captures get a narrower column so they do not eat a screen. */
  narrow?: boolean;
}) {
  return (
    <figure className={narrow ? 'lp-shot is-narrow' : 'lp-shot'}>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={asset(src)}
        alt={alt}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
      />
      {caption ? <figcaption>{caption}</figcaption> : null}
    </figure>
  );
}
