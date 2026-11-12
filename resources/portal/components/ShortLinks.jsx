import React, { useState, useEffect } from 'react';
import { api } from '../api';
import { useAuth } from '../context/AuthContext';
import {
  Link as LinkIcon,
  Copy,
  Check,
  Plus,
  Trash2,
  ExternalLink,
  Sparkles
} from 'lucide-react';

export default function ShortLinks() {
  const { clientApp } = useAuth();
  const [links, setLinks] = useState([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [copiedId, setCopiedId] = useState(null);
  const [copiedPrimary, setCopiedPrimary] = useState(false);

  const [label, setLabel] = useState('');
  const [maxDownloads, setMaxDownloads] = useState('');
  const [expiresDays, setExpiresDays] = useState('');

  const primaryShortUrl = clientApp?.short_url || `${window.location.origin}/s/${clientApp?.slug || 'client'}`;

  const fetchLinks = async () => {
    try {
      const res = await api.get('/api/portal/links');
      setLinks(res.links || []);
    } catch (err) {
      console.error('Failed to fetch links:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLinks();
  }, []);

  const handleCreate = async (e) => {
    e.preventDefault();
    try {
      await api.post('/api/portal/links', {
        label: label || 'Custom Download Link',
        max_downloads: maxDownloads ? parseInt(maxDownloads) : null,
        expires_days: expiresDays ? parseInt(expiresDays) : null,
      });
      setLabel('');
      setMaxDownloads('');
      setExpiresDays('');
      setCreating(false);
      await fetchLinks();
    } catch (err) {
      alert('Failed to create link: ' + err.message);
    }
  };

  const handleDelete = async (linkId) => {
    if (!window.confirm('Are you sure you want to delete this download link?')) return;
    try {
      await api.delete(`/api/portal/links/${linkId}`);
      setLinks((prev) => prev.filter((l) => l.id !== linkId));
    } catch (err) {
      alert('Failed to delete link: ' + err.message);
    }
  };

  const copyUrl = (url, id) => {
    navigator.clipboard.writeText(url);
    if (id === 'primary') {
      setCopiedPrimary(true);
      setTimeout(() => setCopiedPrimary(false), 2000);
    } else {
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    }
  };

  return (
    <div className="space-y-6">
      <div className="rounded-2xl bg-gradient-to-br from-slate-900 via-slate-900 to-blue-950/40 border border-slate-800 p-6 shadow-xl relative overflow-hidden">
        <div className="absolute top-0 right-0 -mr-16 -mt-16 w-64 h-64 bg-blue-500/10 rounded-full blur-3xl pointer-events-none" />

        <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-6 relative z-10">
          <div>
            <span className="inline-flex items-center gap-1.5 rounded-full bg-blue-500/10 px-3 py-1 text-xs font-semibold text-blue-400 border border-blue-500/20 mb-3">
              <Sparkles className="h-3.5 w-3.5" />
              Permanent Customer Short Link
            </span>
            <h2 className="text-xl font-extrabold text-white">
              {clientApp?.app_name || 'Remote Support'} Download Link
            </h2>
            <p className="text-xs text-slate-400 mt-1 max-w-xl">
              Share this short link on your website, chat, or email. When your customer opens it, they immediately get the right installer for their OS.
            </p>
          </div>

          <div className="w-full md:w-auto flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
            <div className="rounded-xl bg-slate-950 border border-slate-800 px-4 py-2.5 font-mono text-sm text-blue-400 font-semibold select-all truncate">
              {primaryShortUrl}
            </div>
            <button
              onClick={() => copyUrl(primaryShortUrl, 'primary')}
              className="flex items-center justify-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-500 px-4 py-2.5 text-xs font-bold text-black shadow-lg transition active:scale-[0.98] shrink-0"
            >
              {copiedPrimary ? <Check className="h-4 w-4 text-emerald-300" /> : <Copy className="h-4 w-4" />}
              <span>{copiedPrimary ? 'Copied!' : 'Copy Link'}</span>
            </button>
            <a
              href={primaryShortUrl}
              target="_blank"
              rel="noreferrer"
              className="flex items-center justify-center rounded-xl bg-slate-800 hover:bg-slate-700 px-3 py-2.5 text-slate-300 hover:text-white transition border border-slate-700 shrink-0"
              title="Open in New Tab"
            >
              <ExternalLink className="h-4 w-4" />
            </a>
          </div>
        </div>
      </div>

      <div className="rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-xl">
        <div className="flex items-center justify-between pb-4 border-b border-slate-800 mb-6">
          <div>
            <h3 className="text-base font-bold text-white flex items-center gap-2">
              <LinkIcon className="h-4 w-4 text-blue-400" />
              Custom & Expiring Download Links
            </h3>
            <p className="text-xs text-slate-400 mt-0.5">
              Create single-use or time-limited download links for specific customers.
            </p>
          </div>

          <button
            onClick={() => setCreating(!creating)}
            className="flex items-center gap-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 px-3.5 py-2 text-xs font-bold text-black shadow-md transition"
          >
            <Plus className="h-3.5 w-3.5" />
            <span>New Custom Link</span>
          </button>
        </div>

        {creating && (
          <form onSubmit={handleCreate} className="mb-6 rounded-xl bg-slate-950 border border-slate-800 p-4 space-y-4">
            <h4 className="text-xs font-bold text-white">New Custom Link Details</h4>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div>
                <label className="block text-[11px] font-medium text-slate-400 mb-1">Label / Note</label>
                <input
                  type="text"
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                  placeholder="e.g. Acme Corp Support"
                  className="w-full rounded-lg bg-slate-900 border border-slate-700 px-3 py-2 text-xs text-white focus:border-blue-500 focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-[11px] font-medium text-slate-400 mb-1">Max Downloads</label>
                <input
                  type="number"
                  min="1"
                  value={maxDownloads}
                  onChange={(e) => setMaxDownloads(e.target.value)}
                  placeholder="Leave empty for unlimited"
                  className="w-full rounded-lg bg-slate-900 border border-slate-700 px-3 py-2 text-xs text-white focus:border-blue-500 focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-[11px] font-medium text-slate-400 mb-1">Validity (Days)</label>
                <input
                  type="number"
                  min="1"
                  max="365"
                  value={expiresDays}
                  onChange={(e) => setExpiresDays(e.target.value)}
                  placeholder="Leave empty for unlimited"
                  className="w-full rounded-lg bg-slate-900 border border-slate-700 px-3 py-2 text-xs text-white focus:border-blue-500 focus:outline-none"
                />
              </div>
            </div>
            <div className="flex justify-end gap-2 pt-2 border-t border-slate-800">
              <button
                type="button"
                onClick={() => setCreating(false)}
                className="rounded-lg bg-slate-800 px-3 py-1.5 text-xs text-slate-300 hover:bg-slate-700"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-semibold text-black hover:bg-blue-500"
              >
                Create
              </button>
            </div>
          </form>
        )}

        {loading && links.length === 0 ? (
          <p className="text-xs text-slate-400 text-center py-6">Loading...</p>
        ) : links.length === 0 ? (
          <p className="text-xs text-slate-400 text-center py-6">No custom download links created yet.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-slate-800 text-slate-400">
                  <th className="pb-3 font-semibold">Label</th>
                  <th className="pb-3 font-semibold">URL</th>
                  <th className="pb-3 font-semibold">Downloads</th>
                  <th className="pb-3 font-semibold">Last Downloaded</th>
                  <th className="pb-3 font-semibold">Status</th>
                  <th className="pb-3 font-semibold text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/60">
                {links.map((link) => (
                  <tr key={link.id} className="hover:bg-slate-800/30 transition">
                    <td className="py-3 font-semibold text-white">{link.label}</td>
                    <td className="py-3 font-mono text-blue-400 truncate max-w-[200px]">
                      {link.url}
                    </td>
                    <td className="py-3 text-slate-300">
                      {link.download_count} {link.max_downloads ? `/ ${link.max_downloads}` : ''}
                    </td>
                    <td className="py-3 text-slate-400">
                      {link.last_downloaded_at ? new Date(link.last_downloaded_at).toLocaleDateString('en-US') : '-'}
                    </td>
                    <td className="py-3">
                      {link.is_usable ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-semibold text-emerald-400 border border-emerald-500/20">
                          Active
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-slate-800 px-2 py-0.5 text-[10px] text-slate-400">
                          Expired
                        </span>
                      )}
                    </td>
                    <td className="py-3 text-right">
                      <div className="inline-flex items-center gap-1.5">
                        <button
                          onClick={() => copyUrl(link.url, link.id)}
                          className="rounded-lg bg-slate-800 hover:bg-slate-700 p-1.5 text-slate-300 hover:text-white transition"
                          title="Copy Link"
                        >
                          {copiedId === link.id ? <Check className="h-3.5 w-3.5 text-emerald-400" /> : <Copy className="h-3.5 w-3.5" />}
                        </button>
                        <a
                          href={link.url}
                          target="_blank"
                          rel="noreferrer"
                          className="rounded-lg bg-slate-800 hover:bg-slate-700 p-1.5 text-slate-300 hover:text-white transition"
                          title="Open"
                        >
                          <ExternalLink className="h-3.5 w-3.5" />
                        </a>
                        <button
                          onClick={() => handleDelete(link.id)}
                          className="rounded-lg p-1.5 text-slate-500 hover:bg-red-500/10 hover:text-red-400 transition"
                          title="Delete"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
