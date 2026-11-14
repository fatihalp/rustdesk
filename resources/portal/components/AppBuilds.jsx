import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import { card, errorNotice, secondaryButton } from '../styles';
import { timeAgo } from '../time';

const PLATFORMS = [
  { id: 'macos', label: 'Mac' },
  { id: 'windows', label: 'Windows' },
];
const IN_PROGRESS = ['pending', 'building'];
const PROGRESS_POLL_MS = 5000;

function describe(latestBuild, hasReadyBuild) {
  if (!latestBuild) return 'Not built yet';
  if (IN_PROGRESS.includes(latestBuild.status)) return 'Building. This can take a few minutes.';
  if (latestBuild.status === 'ready') return `Ready · built ${timeAgo(latestBuild.finished_at)}`;

  return hasReadyBuild
    ? 'The last build failed. Customers still get the previous version.'
    : 'The last build failed. Please try again.';
}

export default function AppBuilds() {
  const [builds, setBuilds] = useState([]);
  const [starting, setStarting] = useState(null);
  const [error, setError] = useState(null);

  const refresh = useCallback(
    () => api.get('/api/portal/builds').then((response) => setBuilds(response.builds)),
    []
  );

  useEffect(() => {
    refresh().catch((failure) => setError(failure.message));
  }, [refresh]);

  const inProgress = builds.some((build) => IN_PROGRESS.includes(build.status));

  useEffect(() => {
    if (!inProgress) return undefined;

    const interval = setInterval(() => refresh().catch(() => {}), PROGRESS_POLL_MS);

    return () => clearInterval(interval);
  }, [inProgress, refresh]);

  const build = async (platform) => {
    setStarting(platform);
    setError(null);
    try {
      await api.post('/api/portal/builds/trigger', { platform });
    } catch (failure) {
      setError(failure.message);
    } finally {
      await refresh().catch(() => {});
      setStarting(null);
    }
  };

  return (
    <section>
      <h2 className="text-2xl font-semibold">Your app</h2>
      <p className="mb-4 mt-1.5 text-slate-400">
        Build again after changing your brand so customers download the latest version.
      </p>
      {error && <p className={`${errorNotice} mb-4`}>{error}</p>}

      <ul className={`${card} divide-y divide-slate-800`}>
        {PLATFORMS.map(({ id, label }) => {
          const platformBuilds = builds.filter((candidate) => candidate.platform === id);
          const latestBuild = platformBuilds[0];
          const busy = starting === id || IN_PROGRESS.includes(latestBuild?.status);

          return (
            <li key={id} className="flex items-center justify-between gap-4 px-5 py-4">
              <div className="min-w-0">
                <p className="font-medium">{label}</p>
                <p className="text-sm text-slate-400">
                  {starting === id
                    ? 'Building. This can take a few minutes.'
                    : describe(latestBuild, platformBuilds.some((candidate) => candidate.status === 'ready'))}
                </p>
              </div>
              <button onClick={() => build(id)} disabled={busy} className={secondaryButton}>
                {latestBuild ? 'Rebuild' : 'Build'}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
