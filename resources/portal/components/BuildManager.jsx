import React, { useState, useEffect } from 'react';
import { api } from '../api';
import { useAuth } from '../context/AuthContext';
import {
  Cpu,
  Download,
  Check,
  AlertTriangle,
  RefreshCw,
  Sparkles,
  Apple,
  Layers
} from 'lucide-react';

export default function BuildManager() {
  const { clientApp } = useAuth();
  const [builds, setBuilds] = useState([]);
  const [loading, setLoading] = useState(true);
  const [buildingPlatform, setBuildingPlatform] = useState(null);
  const [errorMsg, setErrorMsg] = useState(null);
  const [successMsg, setSuccessMsg] = useState(null);

  const fetchBuilds = async () => {
    try {
      const res = await api.get('/api/portal/builds');
      setBuilds(res.builds || []);
    } catch (err) {
      console.error('Failed to fetch builds:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchBuilds();
  }, []);

  const handleTriggerBuild = async (platform) => {
    setBuildingPlatform(platform);
    setErrorMsg(null);
    setSuccessMsg(null);
    try {
      await api.post('/api/portal/builds/trigger', { platform });
      setSuccessMsg(`Custom branded ${platform.toUpperCase()} package has been built and is ready for download!`);
      await fetchBuilds();
    } catch (err) {
      setErrorMsg(err.message || 'Failed to start build.');
    } finally {
      setBuildingPlatform(null);
    }
  };

  return (
    <div className="space-y-6">
      <div className="rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-xl">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div>
            <h2 className="text-xl font-bold text-white flex items-center gap-2">
              <Layers className="h-5 w-5 text-blue-500" />
              Custom Branded Client Builder
            </h2>
            <p className="text-xs text-slate-400 mt-1 max-w-2xl">
              Build a custom client package pre-configured with your company logo, name, and relay server settings.
              As soon as your customer launches the client, it registers directly with your dashboard.
            </p>
          </div>

          <button
            onClick={() => { setLoading(true); fetchBuilds(); }}
            disabled={loading}
            className="flex items-center gap-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 px-3.5 py-2 text-xs font-medium text-slate-200 transition border border-slate-700/60"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </button>
        </div>

        {successMsg && (
          <div className="mt-4 rounded-xl bg-emerald-500/10 border border-emerald-500/30 p-3.5 text-xs text-emerald-300 flex items-center gap-2">
            <Check className="h-4 w-4 text-emerald-400" />
            <span>{successMsg}</span>
          </div>
        )}

        {errorMsg && (
          <div className="mt-4 rounded-xl bg-red-500/10 border border-red-500/30 p-3.5 text-xs text-red-400">
            {errorMsg}
          </div>
        )}

        <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="rounded-xl bg-slate-950 border border-slate-800 p-5 flex flex-col justify-between hover:border-slate-700 transition">
            <div>
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2.5">
                  <div className="h-10 w-10 rounded-xl bg-slate-900 border border-slate-800 flex items-center justify-center text-white">
                    <Apple className="h-5 w-5" />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-white">macOS Client</h3>
                    <p className="text-[11px] text-slate-400">Apple Silicon (M1/M2/M3/M4) & Intel</p>
                  </div>
                </div>
                <span className="text-[10px] uppercase font-bold text-blue-400 bg-blue-500/10 px-2.5 py-1 rounded-full border border-blue-500/20">
                  .dmg (Apple Disk Image)
                </span>
              </div>
              <p className="text-xs text-slate-400">
                Ready-to-run macOS application bundled with your logo and "{clientApp?.app_name || 'Remote Support'}" title.
              </p>
            </div>

            <div className="mt-5 pt-4 border-t border-slate-800/80 flex items-center justify-between">
              <span className="text-[11px] text-slate-500 font-mono">v1.5.1 Standalone</span>
              <button
                onClick={() => handleTriggerBuild('macos')}
                disabled={buildingPlatform !== null}
                className="flex items-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-500 px-4 py-2 text-xs font-bold text-white shadow-md shadow-blue-500/20 transition disabled:opacity-50"
              >
                {buildingPlatform === 'macos' ? (
                  <>
                    <RefreshCw className="h-3.5 w-3.5 animate-spin" />
                    <span>Packaging...</span>
                  </>
                ) : (
                  <>
                    <Sparkles className="h-3.5 w-3.5" />
                    <span>Build macOS</span>
                  </>
                )}
              </button>
            </div>
          </div>

          <div className="rounded-xl bg-slate-950 border border-slate-800 p-5 flex flex-col justify-between hover:border-slate-700 transition">
            <div>
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2.5">
                  <div className="h-10 w-10 rounded-xl bg-slate-900 border border-slate-800 flex items-center justify-center text-white">
                    <Cpu className="h-5 w-5 text-blue-400" />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-white">Windows Client</h3>
                    <p className="text-[11px] text-slate-400">Windows 10 / 11 (64-bit)</p>
                  </div>
                </div>
                <span className="text-[10px] uppercase font-bold text-blue-400 bg-blue-500/10 px-2.5 py-1 rounded-full border border-blue-500/20">
                  .exe / .zip
                </span>
              </div>
              <p className="text-xs text-slate-400">
                Portable Windows support client pre-connected to your company's relay server with zero installation required.
              </p>
            </div>

            <div className="mt-5 pt-4 border-t border-slate-800/80 flex items-center justify-between">
              <span className="text-[11px] text-slate-500 font-mono">v1.5.1 Portable</span>
              <button
                onClick={() => handleTriggerBuild('windows')}
                disabled={buildingPlatform !== null}
                className="flex items-center gap-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 px-4 py-2 text-xs font-bold text-white shadow-md shadow-indigo-500/20 transition disabled:opacity-50"
              >
                {buildingPlatform === 'windows' ? (
                  <>
                    <RefreshCw className="h-3.5 w-3.5 animate-spin" />
                    <span>Packaging...</span>
                  </>
                ) : (
                  <>
                    <Sparkles className="h-3.5 w-3.5" />
                    <span>Build Windows</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-xl">
        <h3 className="text-base font-bold text-white mb-4">Build History</h3>

        {loading && builds.length === 0 ? (
          <p className="text-xs text-slate-400 text-center py-8">Loading...</p>
        ) : builds.length === 0 ? (
          <div className="text-center py-8 text-slate-400">
            <p className="text-xs">No builds have been created yet.</p>
            <p className="text-[11px] text-slate-500 mt-1">Use the buttons above to build your first branded client package.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-slate-800 text-slate-400">
                  <th className="pb-3 font-semibold">Platform</th>
                  <th className="pb-3 font-semibold">Status</th>
                  <th className="pb-3 font-semibold">Artifact</th>
                  <th className="pb-3 font-semibold">Size</th>
                  <th className="pb-3 font-semibold">SHA256</th>
                  <th className="pb-3 font-semibold">Date</th>
                  <th className="pb-3 font-semibold text-right">Action</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/60">
                {builds.map((b) => (
                  <tr key={b.id} className="hover:bg-slate-800/30 transition">
                    <td className="py-3 font-bold text-white capitalize flex items-center gap-2">
                      {b.platform === 'macos' ? <Apple className="h-4 w-4" /> : <Cpu className="h-4 w-4 text-blue-400" />}
                      <span>{b.platform}</span>
                    </td>
                    <td className="py-3">
                      {b.status === 'ready' ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-semibold text-emerald-400 border border-emerald-500/20">
                          <Check className="h-3 w-3" />
                          Ready
                        </span>
                      ) : b.status === 'building' ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-blue-500/10 px-2 py-0.5 text-[10px] font-semibold text-blue-400 border border-blue-500/20">
                          <RefreshCw className="h-3 w-3 animate-spin" />
                          Building
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-red-500/10 px-2 py-0.5 text-[10px] font-semibold text-red-400 border border-red-500/20">
                          <AlertTriangle className="h-3 w-3" />
                          Failed
                        </span>
                      )}
                    </td>
                    <td className="py-3 font-mono text-slate-300">{b.artifact_name || '-'}</td>
                    <td className="py-3 text-slate-400">{b.size_formatted || '-'}</td>
                    <td className="py-3 font-mono text-slate-500 truncate max-w-[100px]" title={b.sha256}>
                      {b.sha256 ? `${b.sha256.substring(0, 8)}...` : '-'}
                    </td>
                    <td className="py-3 text-slate-400">
                      {b.created_at ? new Date(b.created_at).toLocaleDateString('en-US') : '-'}
                    </td>
                    <td className="py-3 text-right">
                      {b.status === 'ready' && (
                        <a
                          href={`/s/${clientApp?.slug}`}
                          target="_blank"
                          rel="noreferrer"
                          className="inline-flex items-center gap-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 px-3 py-1.5 text-xs font-semibold text-white transition border border-slate-700"
                        >
                          <Download className="h-3 w-3 text-blue-400" />
                          <span>Download</span>
                        </a>
                      )}
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
