import React, { useState, useEffect, useRef } from 'react';
import { api } from '../api';
import {
  Monitor,
  RefreshCw,
  Search,
  ExternalLink,
  Trash2,
  ShieldCheck,
  Apple,
  Wifi,
  WifiOff,
  Radio,
  Ban,
  PowerOff,
  Edit3
} from 'lucide-react';
import ConnectModal from './ConnectModal';

export default function DeviceList({ onGoToLinks }) {
  const [devices, setDevices] = useState([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [selectedDevice, setSelectedDevice] = useState(null);
  const [editingDevice, setEditingDevice] = useState(null);
  const [editForm, setEditForm] = useState({ hostname: '', notes: '' });
  const [wsConnected, setWsConnected] = useState(false);
  const [wsUrl, setWsUrl] = useState('ws://127.0.0.1:21120/ws');
  const [activeSessions, setActiveSessions] = useState(0);
  const wsRef = useRef(null);

  const fetchDevices = async () => {
    try {
      const res = await api.get('/api/portal/devices');
      setDevices(res.devices || []);
      if (res.relay?.ws_url) {
        setWsUrl(res.relay.ws_url);
      }
      if (typeof res.relay?.active_sessions === 'number') {
        setActiveSessions(res.relay.active_sessions);
      }
    } catch (err) {
      console.error('Failed to load devices:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchDevices();
  }, []);

  useEffect(() => {
    const intervalMs = wsConnected ? 60000 : 10000;
    const interval = setInterval(() => {
      if (typeof document === 'undefined' || document.visibilityState === 'visible') {
        fetchDevices();
      }
    }, intervalMs);
    return () => clearInterval(interval);
  }, [wsConnected]);

  useEffect(() => {
    let socket;
    let reconnectTimer;
    let disposed = false;

    const connectWs = () => {
      if (disposed || !wsUrl) return;
      try {
        socket = new WebSocket(wsUrl);
        wsRef.current = socket;

        socket.onopen = () => {
          if (!disposed) setWsConnected(true);
        };

        socket.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            handleRealtimeEvent(msg);
          } catch (_) {}
        };

        socket.onclose = () => {
          if (!disposed) {
            setWsConnected(false);
            reconnectTimer = setTimeout(connectWs, 3000);
          }
        };

        socket.onerror = () => {
          socket.close();
        };
      } catch (_) {
        reconnectTimer = setTimeout(connectWs, 3000);
      }
    };

    connectWs();
    return () => {
      disposed = true;
      clearTimeout(reconnectTimer);
      if (socket) socket.close();
    };
  }, [wsUrl]);

  const handleRealtimeEvent = (msg) => {
    if (!msg || !msg.type) return;

    if (msg.type === 'snapshot') {
      const onlineMap = new Map();
      const blockedSet = new Set();
      (msg.peers || []).forEach((p) => {
        if (p.online) onlineMap.set(p.id, p);
        if (p.blocked) blockedSet.add(p.id);
      });
      if (typeof msg.active_sessions === 'number') {
        setActiveSessions(msg.active_sessions);
      }
      setDevices((prev) =>
        prev.map((d) => {
          const live = onlineMap.get(d.supportdesk_id);
          const blocked = blockedSet.has(d.supportdesk_id);
          return {
            ...d,
            is_blocked: blocked,
            is_online: !blocked && (Boolean(live) || d.is_online),
            ip_address: live?.ip || d.ip_address,
          };
        })
      );
      return;
    }

    if (msg.type === 'peer.online') {
      setDevices((prev) =>
        prev.map((d) =>
          d.supportdesk_id === msg.id
            ? {
                ...d,
                is_online: true,
                is_blocked: false,
                ip_address: msg.ip || d.ip_address,
                last_seen_at: new Date().toISOString(),
              }
            : d
        )
      );
    } else if (msg.type === 'peer.offline') {
      setDevices((prev) =>
        prev.map((d) =>
          d.supportdesk_id === msg.id ? { ...d, is_online: false, active_connections: 0 } : d
        )
      );
    } else if (msg.type === 'peer.blocked' || msg.type === 'peer.unblocked') {
      const blocked = msg.type === 'peer.blocked';
      setDevices((prev) =>
        prev.map((d) =>
          d.supportdesk_id === msg.id
            ? { ...d, is_blocked: blocked, is_online: blocked ? false : d.is_online }
            : d
        )
      );
    } else if (msg.type === 'session.started') {
      setActiveSessions((n) => n + 1);
      setDevices((prev) =>
        prev.map((d) =>
          d.supportdesk_id === msg.peer_id
            ? { ...d, active_connections: (d.active_connections || 0) + 1 }
            : d
        )
      );
    } else if (msg.type === 'session.ended') {
      setActiveSessions((n) => Math.max(0, n - 1));
      setDevices((prev) =>
        prev.map((d) =>
          d.supportdesk_id === msg.peer_id
            ? { ...d, active_connections: Math.max(0, (d.active_connections || 1) - 1) }
            : d
        )
      );
    }
  };

  const handleAction = async (device, action) => {
    try {
      await api.post(`/api/portal/devices/${device.id}/action`, { action });
      if (action === 'disconnect') {
        setDevices((prev) =>
          prev.map((d) =>
            d.id === device.id ? { ...d, is_online: false, active_connections: 0 } : d
          )
        );
      } else if (action === 'block' || action === 'unblock') {
        const isBlocked = action === 'block';
        setDevices((prev) =>
          prev.map((d) =>
            d.id === device.id
              ? { ...d, is_blocked: isBlocked, is_online: isBlocked ? false : d.is_online }
              : d
          )
        );
      }
    } catch (err) {
      alert('Action failed: ' + err.message);
    }
  };

  const handleSaveEdit = async (e) => {
    e.preventDefault();
    if (!editingDevice) return;
    try {
      const res = await api.put(`/api/portal/devices/${editingDevice.id}`, editForm);
      setDevices((prev) =>
        prev.map((d) =>
          d.id === editingDevice.id
            ? { ...d, hostname: res.device.hostname || d.hostname, notes: res.device.notes }
            : d
        )
      );
      setEditingDevice(null);
    } catch (err) {
      alert('Failed to update device: ' + err.message);
    }
  };

  const handleDelete = async (deviceId) => {
    if (!window.confirm('Are you sure you want to remove this device from your panel?')) return;
    try {
      await api.delete(`/api/portal/devices/${deviceId}`);
      setDevices((prev) => prev.filter((d) => d.id !== deviceId));
    } catch (err) {
      alert('Failed to delete device: ' + err.message);
    }
  };

  const filtered = devices.filter((d) => {
    const q = search.toLowerCase();
    return (
      (d.hostname && d.hostname.toLowerCase().includes(q)) ||
      (d.supportdesk_id && d.supportdesk_id.includes(q)) ||
      (d.username && d.username.toLowerCase().includes(q)) ||
      (d.ip_address && d.ip_address.includes(q)) ||
      (d.notes && d.notes.toLowerCase().includes(q))
    );
  });

  const onlineCount = devices.filter((d) => d.is_online).length;

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 rounded-2xl bg-slate-900 border border-slate-800 p-5">
        <div>
          <div className="flex items-center flex-wrap gap-3">
            <h2 className="text-xl font-bold text-white">Customer Devices</h2>
            <div className="flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-2.5 py-0.5 text-xs font-semibold text-emerald-400 border border-emerald-500/20">
              <span className="h-2 w-2 rounded-full bg-emerald-400 animate-pulse" />
              <span>{onlineCount} Online</span>
            </div>
            {activeSessions > 0 && (
              <div className="flex items-center gap-1.5 rounded-full bg-blue-500/10 px-2.5 py-0.5 text-xs font-semibold text-blue-400 border border-blue-500/20">
                <span>{activeSessions} Active Session{activeSessions > 1 ? 's' : ''}</span>
              </div>
            )}
            <span className="text-xs text-slate-500">Total: {devices.length}</span>
          </div>
          <p className="text-xs text-slate-400 mt-1">
            Customers running your branded client appear here in real time for instant remote support and device management.
          </p>
        </div>

        <div className="flex items-center gap-2 self-stretch sm:self-auto">
          <div
            className={`flex items-center gap-1.5 rounded-xl px-3 py-2 text-xs font-medium border ${
              wsConnected
                ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                : 'bg-slate-800 text-slate-400 border-slate-700/60'
            }`}
            title={wsConnected ? 'Connected to Rust RelayServer WebSocket stream' : 'Polling fallback active'}
          >
            <Radio className={`h-3.5 w-3.5 ${wsConnected ? 'animate-pulse text-emerald-400' : 'text-slate-500'}`} />
            <span>{wsConnected ? 'Live Socket' : 'Syncing'}</span>
          </div>
          <button
            onClick={() => { setLoading(true); fetchDevices(); }}
            disabled={loading}
            className="flex items-center gap-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 px-3.5 py-2 text-xs font-medium text-slate-200 transition border border-slate-700/60"
            title="Refresh"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      <div className="flex items-center justify-between gap-4">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-500" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search by hostname, ID, username, IP, or notes..."
            className="w-full rounded-xl bg-slate-900 border border-slate-800 pl-10 pr-4 py-2 text-xs text-white placeholder-slate-500 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </div>
      </div>

      {loading && devices.length === 0 ? (
        <div className="rounded-2xl bg-slate-900 border border-slate-800 p-12 text-center text-slate-400">
          <RefreshCw className="h-6 w-6 animate-spin mx-auto mb-3 text-blue-500" />
          <p className="text-sm">Loading devices...</p>
        </div>
      ) : filtered.length === 0 ? (
        <div className="rounded-2xl bg-slate-900 border border-slate-800 p-12 text-center">
          <Monitor className="h-10 w-10 text-slate-600 mx-auto mb-3" />
          <h3 className="text-base font-semibold text-white">
            {devices.length === 0 ? 'No Customer Devices Connected Yet' : 'No Matching Devices Found'}
          </h3>
          <p className="text-xs text-slate-400 max-w-md mx-auto mt-1.5 leading-relaxed">
            {devices.length === 0
              ? 'Share your short download link with your customers. As soon as they launch the client, their computer will appear here.'
              : 'Try adjusting your search query.'}
          </p>
          {devices.length === 0 && onGoToLinks && (
            <button
              onClick={onGoToLinks}
              className="mt-5 inline-flex items-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-500 px-4 py-2.5 text-xs font-semibold text-black shadow-lg transition"
            >
              <span>Share Download Link</span>
            </button>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map((device) => (
            <div
              key={device.id}
              className={`group rounded-2xl bg-slate-900 border p-5 transition flex flex-col justify-between ${
                device.is_blocked
                  ? 'border-red-500/30 opacity-75'
                  : device.is_online
                  ? 'border-emerald-500/30 hover:border-emerald-500/60 shadow-lg'
                  : 'border-slate-800/80 hover:border-slate-700 opacity-85'
              }`}
            >
              <div>
                <div className="flex items-center justify-between gap-2 mb-3">
                  <div className="flex items-center gap-2 flex-wrap">
                    {device.is_blocked ? (
                      <span className="inline-flex items-center gap-1.5 rounded-full bg-red-500/15 px-2.5 py-0.5 text-[11px] font-semibold text-red-400 border border-red-500/30">
                        <Ban className="h-3 w-3" />
                        Blocked
                      </span>
                    ) : device.is_online ? (
                      <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-semibold text-emerald-400 border border-emerald-500/30">
                        <Wifi className="h-3 w-3" />
                        Online
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-800 px-2.5 py-0.5 text-[11px] font-medium text-slate-400">
                        <WifiOff className="h-3 w-3" />
                        Offline
                      </span>
                    )}

                    {device.has_unattended_password && (
                      <span
                        className="inline-flex items-center gap-1 rounded-full bg-blue-500/10 px-2 py-0.5 text-[11px] font-medium text-blue-400 border border-blue-500/20"
                        title="Unattended access is enabled"
                      >
                        <ShieldCheck className="h-3 w-3" />
                        Unattended
                      </span>
                    )}
                  </div>

                  <span className="text-[11px] font-mono text-slate-400 uppercase bg-slate-950 px-2 py-0.5 rounded border border-slate-800 flex items-center gap-1">
                    {device.platform === 'macos' && <Apple className="h-3 w-3" />}
                    {device.platform}
                  </span>
                </div>

                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <h3 className="text-base font-bold text-white truncate" title={device.hostname}>
                      {device.hostname}
                    </h3>
                    <p className="text-xs text-slate-400 truncate mt-0.5">
                      User: <span className="text-slate-300">{device.username}</span>
                    </p>
                  </div>
                  <button
                    onClick={() => {
                      setEditingDevice(device);
                      setEditForm({ hostname: device.hostname || '', notes: device.notes || '' });
                    }}
                    className="rounded-lg p-1.5 text-slate-500 hover:bg-slate-800 hover:text-slate-200 transition"
                    title="Edit device name & notes"
                  >
                    <Edit3 className="h-3.5 w-3.5" />
                  </button>
                </div>

                {device.notes && (
                  <p className="mt-2 rounded-lg bg-slate-950/80 border border-slate-800/80 px-2.5 py-1.5 text-[11px] text-slate-300 line-clamp-2">
                    {device.notes}
                  </p>
                )}

                <div className="mt-4 space-y-1.5 border-t border-slate-800/80 pt-3 text-xs">
                  <div className="flex justify-between">
                    <span className="text-slate-500">Support ID:</span>
                    <span className="font-mono font-bold text-white tracking-wide">
                      {device.supportdesk_id}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-500">Operating System:</span>
                    <span className="text-slate-300 truncate max-w-[160px] text-right" title={device.os}>
                      {device.os}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-500">IP Address:</span>
                    <span className="font-mono text-slate-400">{device.ip_address || '-'}</span>
                  </div>
                </div>
              </div>

              <div className="mt-5 pt-3 border-t border-slate-800/80 flex items-center gap-1.5">
                <button
                  onClick={() => setSelectedDevice(device)}
                  disabled={device.is_blocked}
                  className="flex-1 flex items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-500 hover:to-indigo-500 disabled:opacity-40 py-2.5 px-3 text-xs font-bold text-black shadow-md transition active:scale-[0.98]"
                >
                  <ExternalLink className="h-3.5 w-3.5" />
                  <span>Connect</span>
                </button>
                {device.is_online && (
                  <button
                    onClick={() => handleAction(device, 'disconnect')}
                    className="rounded-xl p-2.5 text-amber-400 hover:bg-amber-500/10 transition"
                    title="Disconnect active session on RelayServer"
                  >
                    <PowerOff className="h-4 w-4" />
                  </button>
                )}
                <button
                  onClick={() => handleAction(device, device.is_blocked ? 'unblock' : 'block')}
                  className={`rounded-xl p-2.5 transition ${
                    device.is_blocked
                      ? 'text-emerald-400 hover:bg-emerald-500/10'
                      : 'text-slate-400 hover:bg-amber-500/10 hover:text-amber-400'
                  }`}
                  title={device.is_blocked ? 'Unblock device on RelayServer' : 'Block device on RelayServer'}
                >
                  <Ban className="h-4 w-4" />
                </button>
                <button
                  onClick={() => handleDelete(device.id)}
                  className="rounded-xl p-2.5 text-slate-500 hover:bg-red-500/10 hover:text-red-400 transition"
                  title="Remove Device"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {editingDevice && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/80 backdrop-blur-sm p-4">
          <form
            onSubmit={handleSaveEdit}
            className="w-full max-w-md rounded-2xl bg-slate-900 border border-slate-800 p-6 space-y-4 shadow-2xl"
          >
            <h3 className="text-base font-bold text-white">
              Edit Device ({editingDevice.supportdesk_id})
            </h3>
            <div>
              <label className="block text-xs font-medium text-slate-300 mb-1">Device Name</label>
              <input
                type="text"
                value={editForm.hostname}
                onChange={(e) => setEditForm({ ...editForm, hostname: e.target.value })}
                className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3.5 py-2 text-xs text-white focus:border-blue-500 focus:outline-none"
                placeholder="e.g. Accounting Workstation"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-300 mb-1">Notes</label>
              <textarea
                rows={3}
                value={editForm.notes}
                onChange={(e) => setEditForm({ ...editForm, notes: e.target.value })}
                className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3.5 py-2 text-xs text-white focus:border-blue-500 focus:outline-none"
                placeholder="Optional notes about this machine or customer..."
              />
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setEditingDevice(null)}
                className="rounded-xl bg-slate-800 hover:bg-slate-700 px-4 py-2 text-xs font-medium text-slate-300"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="rounded-xl bg-blue-600 hover:bg-blue-500 px-4 py-2 text-xs font-semibold text-black"
              >
                Save Changes
              </button>
            </div>
          </form>
        </div>
      )}

      {selectedDevice && (
        <ConnectModal
          device={selectedDevice}
          onClose={() => setSelectedDevice(null)}
        />
      )}
    </div>
  );
}
