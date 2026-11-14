const relativeTime = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

const UNITS = [
  ['day', 86400],
  ['hour', 3600],
  ['minute', 60],
];

export function timeAgo(isoDate) {
  if (!isoDate) return 'never';

  const elapsedSeconds = Math.round((Date.now() - new Date(isoDate).getTime()) / 1000);
  const match = UNITS.find(([, unitSeconds]) => elapsedSeconds >= unitSeconds);

  return match ? relativeTime.format(-Math.floor(elapsedSeconds / match[1]), match[0]) : 'just now';
}
