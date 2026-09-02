import { getContributors } from '@/lib/contributors';
import { site } from '@/lib/site';

/** Server component: the fetch happens during `next build`, not in the browser. */
export async function Contributors() {
  const people = await getContributors();

  return (
    <section className="lp-wrap lp-section">
      <div className="lp-section-head">
        <p className="lp-eyebrow">Contributors</p>
        <h2>
          {people.length > 0
            ? `Built by ${people.length} ${people.length === 1 ? 'person' : 'people'}`
            : 'Built in the open'}
        </h2>
      </div>

      {people.length > 0 ? (
        <ul className="lp-people">
          {people.map((person) => (
            <li key={person.login}>
              <a href={person.profileUrl} title={`${person.login} on GitHub`}>
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={person.avatarUrl}
                  alt=""
                  width={80}
                  height={80}
                  loading="lazy"
                  decoding="async"
                />
                <span>{person.login}</span>
              </a>
            </li>
          ))}
        </ul>
      ) : null}

      <p className="lp-section-link">
        <a href={`${site.repo}/graphs/contributors`}>Full contributor graph →</a>
      </p>
    </section>
  );
}
