import React, { useState, useEffect } from 'react';
import { X, ExternalLink, Copy, Check, ShieldCheck, Terminal, AlertCircle } from 'lucide-react';
import { api } from '../api';

export default function ConnectModal({ device, onClose }) {
  const [copiedCmd, setCopiedCmd] = useState(false);
  const [copiedPass, setCopiedPass] = useState(false);
  const [connInfo, setConnInfo] = useState(null);

  useEffect(() => {
    let active = true;
    if (device?.id) {
      api
        .get(`/api/portal/devices/${device.id}/connect`)
        .then((res) => {
          if (active) setConnInfo(res);
        })
        .catch(() => {});
    }
    return () => {
      active = false;
    };
  }, [device?.id]);

  if (!device) return null;

  const password = connInfo?.unattended_password || device.unattended_password || '';
  const connectCommand =
    connInfo?.connect_command ||
    device.connect_command ||
    `supportdesk --connect ${device.supportdesk_id}${password ? ` --password ${password}` : ''}`;
  const connectUri =
    connInfo?.connect_uri ||
    device.connect_uri ||
    `supportdesk://connection/new/${device.supportdesk_id}${
      password ? `?password=${encodeURIComponent(password)}` : ''
    }`;

  const copyCommand = () => {
    navigator.clipboard.writeText(connectCommand);
    setCopiedCmd(true);
    setTimeout(() => setCopiedCmd(false), 2000);
  };

  const copyPassword = () => {
    if (password) {
      navigator.clipboard.writeText(password);
      setCopiedPass(true);
      setTimeout(() => setCopiedPass(false), 2000);
    }
  };

  const launchApp = () => {
    window.location.href = connectUri;
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="relative w-full max-w-lg rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-2xl">
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div>
            <div className="flex items-center gap-2">
              <h3 className="text-lg font-bold text-white">Start Remote Session</h3>
              {device.is_online ? (
                <span className="flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-2.5 py-0.5 text-xs font-semibold text-emerald-400 border border-emerald-500/20">
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
                  Online
                </span>
              ) : (
                <span className="rounded-full bg-slate-800 px-2.5 py-0.5 text-xs text-slate-400">
                  Offline
                </span>
              )}
            </div>
            <p className="text-xs text-slate-400 mt-0.5">
              {device.hostname} &bull; ID: <span className="font-mono text-slate-200">{device.supportdesk_id}</span>
            </p>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1.5 text-slate-400 hover:bg-slate-800 hover:text-white transition"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="mt-5 space-y-4">
          {device.has_unattended_password ? (
            <div className="flex items-start gap-3 rounded-xl bg-emerald-500/10 border border-emerald-500/20 p-3.5 text-xs text-emerald-300">
              <ShieldCheck className="h-5 w-5 text-emerald-400 shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold text-emerald-200">Unattended Access Enabled</p>
                <p className="text-emerald-400/80 mt-0.5">
                  You can connect to this device directly without entering a password or waiting for user confirmation.
                </p>
              </div>
            </div>
          ) : (
            <div className="flex items-start gap-3 rounded-xl bg-amber-500/10 border border-amber-500/20 p-3.5 text-xs text-amber-300">
              <AlertCircle className="h-5 w-5 text-amber-400 shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold text-amber-200">Interactive / Password Connection</p>
                <p className="text-amber-400/80 mt-0.5">
                  Unattended access is disabled. You may need to ask the customer for their temporary on-screen password.
                </p>
              </div>
            </div>
          )}

          <button
            onClick={launchApp}
            className="w-full flex items-center justify-center gap-2.5 rounded-xl bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-500 hover:to-indigo-500 p-3.5 text-sm font-bold text-white shadow-lg shadow-blue-500/25 transition active:scale-[0.99]"
          >
            <ExternalLink className="h-4 w-4" />
            <span>Open in SupportDesk Client</span>
          </button>

          <div>
            <label className="block text-xs font-medium text-slate-400 mb-1.5 flex items-center justify-between">
              <span className="flex items-center gap-1.5">
                <Terminal className="h-3.5 w-3.5 text-slate-400" />
                Terminal Command (CLI)
              </span>
              <button
                onClick={copyCommand}
                className="text-xs text-blue-400 hover:text-blue-300 flex items-center gap-1"
              >
                {copiedCmd ? <Check className="h-3 w-3 text-emerald-400" /> : <Copy className="h-3 w-3" />}
                <span>{copiedCmd ? 'Copied' : 'Copy'}</span>
              </button>
            </label>
            <div className="rounded-xl bg-slate-950 border border-slate-800 p-3 font-mono text-xs text-slate-300 overflow-x-auto select-all">
              {connectCommand}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 pt-2">
            <div className="rounded-xl bg-slate-950/60 border border-slate-800 p-3">
              <span className="text-[11px] text-slate-400 block mb-1">Device ID</span>
              <span className="font-mono text-sm font-bold text-white">{device.supportdesk_id}</span>
            </div>
            <div className="rounded-xl bg-slate-950/60 border border-slate-800 p-3 relative">
              <span className="text-[11px] text-slate-400 block mb-1">Unattended Password</span>
              {password ? (
                <div className="flex items-center justify-between">
                  <span className="font-mono text-xs font-semibold text-emerald-400 truncate max-w-[120px]">
                    {password}
                  </span>
                  <button
                    onClick={copyPassword}
                    className="text-slate-400 hover:text-white"
                    title="Copy Password"
                  >
                    {copiedPass ? <Check className="h-3.5 w-3.5 text-emerald-400" /> : <Copy className="h-3.5 w-3.5" />}
                  </button>
                </div>
              ) : (
                <span className="text-xs text-slate-500">Not Available</span>
              )}
            </div>
          </div>
        </div>

        <div className="mt-6 flex justify-end">
          <button
            onClick={onClose}
            className="rounded-xl bg-slate-800 hover:bg-slate-700 px-4 py-2 text-xs font-semibold text-slate-200 transition"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
