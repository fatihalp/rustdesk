import React from 'react';
import { LogOut, Monitor } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const PAGES = [
  { id: 'devices', label: 'Devices' },
  { id: 'settings', label: 'Settings' },
];

export default function Header({ page, onNavigate }) {
  const { clientApp, logout } = useAuth();

  return (
    <header className="sticky top-0 z-30 border-b border-slate-800/70 bg-black/70">
      <div className="mx-auto flex h-14 w-full max-w-3xl items-center justify-between gap-3 px-5">
        <div className="flex min-w-0 items-center gap-2.5">
          <div className="flex h-7 w-7 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-slate-800">
            {clientApp?.logo_url ? (
              <img src={clientApp.logo_url} alt="" className="h-full w-full object-cover" />
            ) : (
              <Monitor className="h-4 w-4 text-slate-300" />
            )}
          </div>
          <span className="hidden truncate text-sm font-medium sm:block">{clientApp?.app_name}</span>
        </div>

        <nav className="flex rounded-full bg-slate-900 p-0.5">
          {PAGES.map(({ id, label }) => (
            <button
              key={id}
              onClick={() => onNavigate(id)}
              aria-current={page === id ? 'page' : undefined}
              className={`rounded-full px-4 py-1.5 text-sm transition ${
                page === id ? 'bg-slate-700 font-medium text-white' : 'text-slate-400 hover:text-white'
              }`}
            >
              {label}
            </button>
          ))}
        </nav>

        <button
          onClick={logout}
          className="flex items-center gap-1.5 rounded-full px-2 py-1.5 text-sm text-slate-400 transition hover:text-white"
        >
          <LogOut className="h-4 w-4" />
          <span className="hidden sm:inline">Sign out</span>
        </button>
      </div>
    </header>
  );
}
