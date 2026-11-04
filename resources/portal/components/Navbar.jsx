import React, { useState } from 'react';
import { useAuth } from '../context/AuthContext';
import { Monitor, Share2, LogOut, Check, User } from 'lucide-react';

export default function Navbar({ activeTab, setActiveTab }) {
  const { user, customer, clientApp, logout } = useAuth();
  const [copied, setCopied] = useState(false);

  const shortUrl = clientApp?.short_url || `${window.location.origin}/s/${clientApp?.slug || 'client'}`;

  const copyShortUrl = () => {
    navigator.clipboard.writeText(shortUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const tabs = [
    { id: 'devices', label: 'Devices' },
    { id: 'brand', label: 'Brand & Logo' },
    { id: 'builds', label: 'Builds' },
    { id: 'links', label: 'Short Links' },
  ];

  return (
    <header className="sticky top-0 z-40 border-b border-slate-800/80 bg-slate-900/95 backdrop-blur">
      <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-3 sm:px-6 lg:px-8">
        {/* Brand & Workspace info */}
        <div className="flex items-center gap-4">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-500 shadow-lg shadow-blue-500/20">
            {clientApp?.logo_url ? (
              <img src={clientApp.logo_url} alt="" className="h-full w-full rounded-xl object-cover" />
            ) : (
              <Monitor className="h-5 w-5 text-white" />
            )}
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-bold text-white text-base tracking-tight">
                {clientApp?.app_name || 'SupportDesk'}
              </span>
              <span className="rounded-full bg-blue-500/10 px-2 py-0.5 text-xs font-medium text-blue-400 border border-blue-500/20">
                Multi-Tenant
              </span>
            </div>
            <p className="text-xs text-slate-400">
              {customer?.company || customer?.name || 'Partner Portal'}
            </p>
          </div>
        </div>

        {/* Navigation Tabs */}
        <nav className="hidden md:flex items-center gap-1 rounded-xl bg-slate-950/60 p-1 border border-slate-800">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`rounded-lg px-3.5 py-1.5 text-xs font-semibold transition-all ${
                activeTab === tab.id
                  ? 'bg-blue-600 text-white shadow-sm shadow-blue-500/30'
                  : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </nav>

        {/* Right side actions */}
        <div className="flex items-center gap-3">
          {/* Quick Share Link button */}
          <button
            onClick={copyShortUrl}
            className="flex items-center gap-1.5 rounded-lg bg-slate-800/80 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-700/80 hover:text-white transition border border-slate-700/50"
            title="Copy Customer Download Link"
          >
            {copied ? (
              <>
                <Check className="h-3.5 w-3.5 text-emerald-400" />
                <span className="text-emerald-400 font-semibold">Copied</span>
              </>
            ) : (
              <>
                <Share2 className="h-3.5 w-3.5 text-blue-400" />
                <span>Short Link</span>
              </>
            )}
          </button>

          {/* User profile & Logout */}
          <div className="flex items-center gap-2 pl-2 border-l border-slate-800">
            <div className="h-8 w-8 rounded-full bg-slate-800 flex items-center justify-center overflow-hidden border border-slate-700">
              {user?.avatar_url ? (
                <img src={user.avatar_url} alt="" className="h-full w-full object-cover" />
              ) : (
                <User className="h-4 w-4 text-slate-400" />
              )}
            </div>
            <div className="hidden lg:block text-left">
              <p className="text-xs font-medium text-slate-200 leading-tight">{user?.name}</p>
              <p className="text-[10px] text-slate-400 leading-tight truncate max-w-[120px]">{user?.email}</p>
            </div>
            <button
              onClick={logout}
              className="rounded-lg p-1.5 text-slate-400 hover:bg-red-500/10 hover:text-red-400 transition"
              title="Sign Out"
            >
              <LogOut className="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Mobile nav tabs */}
      <div className="flex md:hidden border-t border-slate-800/80 bg-slate-950/80 px-4 py-2 overflow-x-auto gap-2">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`whitespace-nowrap rounded-lg px-3 py-1 text-xs font-semibold ${
              activeTab === tab.id ? 'bg-blue-600 text-white' : 'text-slate-400'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
    </header>
  );
}
