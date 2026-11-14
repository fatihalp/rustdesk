import React, { useState } from 'react';
import { Search } from 'lucide-react';
import { api } from '../api';
import { useAuth } from '../context/AuthContext';
import { useLiveDevices } from '../hooks/useLiveDevices';
import { card, errorNotice, input, primaryButton } from '../styles';
import ConnectDialog from './ConnectDialog';
import CopyButton from './CopyButton';
import DeviceRow from './DeviceRow';
import EditDeviceDialog from './EditDeviceDialog';

const SEARCH_THRESHOLD = 6;

const RELAY_ACTION_PATCHES = {
  disconnect: { is_online: false, active_connections: 0 },
  block: { is_blocked: true, is_online: false },
  unblock: { is_blocked: false },
};

function matchesSearch(device, query) {
  return [device.hostname, device.supportdesk_id, device.username, device.notes]
    .some((value) => value?.toLowerCase().includes(query));
}

export default function Devices() {
  const { clientApp } = useAuth();
  const { devices, loading, patchDevice, forgetDevice } = useLiveDevices();
  const [search, setSearch] = useState('');
  const [connecting, setConnecting] = useState(null);
  const [editing, setEditing] = useState(null);
  const [error, setError] = useState(null);

  const attempt = async (operation) => {
    setError(null);
    try {
      await operation();
    } catch (failure) {
      setError(failure.message);
    }
  };

  const runRelayAction = (device, action) =>
    attempt(async () => {
      await api.post(`/api/portal/devices/${device.id}/action`, { action });
      patchDevice(device.id, RELAY_ACTION_PATCHES[action]);
    });

  const remove = (device) => {
    if (!window.confirm(`Remove ${device.hostname} from your devices?`)) return;

    attempt(async () => {
      await api.delete(`/api/portal/devices/${device.id}`);
      forgetDevice(device.id);
    });
  };

  const visibleDevices = devices.filter((device) => matchesSearch(device, search.trim().toLowerCase()));

  return (
    <div className="space-y-10">
      <section className={`${card} p-6 sm:p-8`}>
        <h1 className="text-2xl font-semibold">Add a customer</h1>
        <p className="mt-1.5 text-slate-400">
          Send this link. When your customer opens the app, their computer shows up below.
        </p>
        <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center">
          <p className="min-w-0 flex-1 truncate rounded-full bg-black px-5 py-2.5 font-mono text-sm text-slate-200">
            {clientApp?.short_url}
          </p>
          <CopyButton value={clientApp?.short_url} label="Copy Link" className={primaryButton} />
        </div>
      </section>

      <section>
        <div className="mb-4 flex items-center justify-between gap-4">
          <h2 className="text-2xl font-semibold">Devices</h2>
          {devices.length > SEARCH_THRESHOLD && (
            <div className="relative w-full max-w-56">
              <Search className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
              <input
                type="search"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search"
                aria-label="Search devices"
                className={`${input} rounded-full bg-slate-900 py-2 pl-10`}
              />
            </div>
          )}
        </div>

        {error && <p className={`${errorNotice} mb-4`}>{error}</p>}

        {loading ? null : visibleDevices.length === 0 ? (
          <p className={`${card} px-6 py-14 text-center text-slate-400`}>
            {devices.length === 0 ? 'No devices yet. Share your link to get started.' : 'No devices match your search.'}
          </p>
        ) : (
          <ul className={`${card} divide-y divide-slate-800`}>
            {visibleDevices.map((device) => (
              <DeviceRow
                key={device.id}
                device={device}
                onConnect={() => setConnecting(device)}
                onRename={() => setEditing(device)}
                onRelayAction={(action) => runRelayAction(device, action)}
                onRemove={() => remove(device)}
              />
            ))}
          </ul>
        )}
      </section>

      {connecting && <ConnectDialog device={connecting} onClose={() => setConnecting(null)} />}
      {editing && (
        <EditDeviceDialog
          device={editing}
          onClose={() => setEditing(null)}
          onSaved={(changes) => {
            patchDevice(editing.id, changes);
            setEditing(null);
          }}
        />
      )}
    </div>
  );
}
