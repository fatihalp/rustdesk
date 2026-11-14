import React, { useState } from 'react';
import { MoreHorizontal } from 'lucide-react';
import { primaryButton } from '../styles';
import { timeAgo } from '../time';

function presence(device) {
  if (device.is_blocked) return { dot: 'bg-red-500', label: 'Blocked' };
  if (device.active_connections > 0) return { dot: 'bg-[#30d158]', label: 'In a session' };
  if (device.is_online) return { dot: 'bg-[#30d158]', label: 'Online' };

  return { dot: 'bg-slate-600', label: `Last seen ${timeAgo(device.last_seen_at)}` };
}

function menuItems(device, { onRename, onRelayAction, onRemove }) {
  return [
    { label: 'Rename', run: onRename },
    device.is_online && { label: 'End Session', run: () => onRelayAction('disconnect') },
    device.is_blocked
      ? { label: 'Unblock', run: () => onRelayAction('unblock') }
      : { label: 'Block', run: () => onRelayAction('block') },
    { label: 'Remove', run: onRemove, destructive: true },
  ].filter(Boolean);
}

export default function DeviceRow({ device, onConnect, ...menuHandlers }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const { dot, label } = presence(device);
  const details = [device.username, device.notes].filter((detail) => detail && detail !== '-');

  return (
    <li className="flex items-center gap-4 px-5 py-4">
      <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${dot}`} />

      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{device.hostname}</p>
        <p className="truncate text-sm text-slate-400">{[label, ...details].join(' · ')}</p>
      </div>

      <button onClick={onConnect} disabled={device.is_blocked} className={primaryButton}>
        Connect
      </button>

      <div className="relative">
        <button
          onClick={() => setMenuOpen(true)}
          aria-label={`More actions for ${device.hostname}`}
          aria-expanded={menuOpen}
          className="flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition hover:bg-slate-800 hover:text-white"
        >
          <MoreHorizontal className="h-5 w-5" />
        </button>

        {menuOpen && (
          <>
            <button
              aria-label="Close menu"
              className="fixed inset-0 z-10 cursor-default"
              onClick={() => setMenuOpen(false)}
            />
            <div className="absolute right-0 top-11 z-20 w-44 overflow-hidden rounded-xl border border-slate-700 bg-slate-800 py-1 shadow-2xl">
              {menuItems(device, menuHandlers).map((item) => (
                <button
                  key={item.label}
                  onClick={() => {
                    setMenuOpen(false);
                    item.run();
                  }}
                  className={`block w-full px-4 py-2 text-left text-sm transition hover:bg-slate-700 ${
                    item.destructive ? 'text-red-400' : 'text-white'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </li>
  );
}
