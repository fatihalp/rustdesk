import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';

const LIVE_EVENTS_URL = document.querySelector('meta[name="relay-ws-url"]')?.content;
const RECONNECT_DELAY_MS = 3000;
const REFRESH_INTERVAL_LIVE_MS = 60000;
const REFRESH_INTERVAL_OFFLINE_MS = 10000;

function patchWhere(devices, matches, patch) {
  return devices.map((device) => (matches(device) ? { ...device, ...patch(device) } : device));
}

function applySnapshot(devices, peers) {
  const onlineIds = new Set(peers.filter((peer) => peer.online).map((peer) => peer.id));
  const blockedIds = new Set(peers.filter((peer) => peer.blocked).map((peer) => peer.id));

  return devices.map((device) => {
    const isBlocked = blockedIds.has(device.supportdesk_id);

    return {
      ...device,
      is_blocked: isBlocked,
      is_online: !isBlocked && (onlineIds.has(device.supportdesk_id) || device.is_online),
    };
  });
}

function applyLiveEvent(devices, event) {
  const isPeer = (device) => device.supportdesk_id === (event.id ?? event.peer_id);

  switch (event.type) {
    case 'snapshot':
      return applySnapshot(devices, event.peers || []);
    case 'peer.online':
      return patchWhere(devices, isPeer, () => ({
        is_online: true,
        is_blocked: false,
        last_seen_at: new Date().toISOString(),
      }));
    case 'peer.offline':
      return patchWhere(devices, isPeer, () => ({ is_online: false, active_connections: 0 }));
    case 'peer.blocked':
      return patchWhere(devices, isPeer, () => ({ is_blocked: true, is_online: false }));
    case 'peer.unblocked':
      return patchWhere(devices, isPeer, () => ({ is_blocked: false }));
    case 'session.started':
      return patchWhere(devices, isPeer, (device) => ({
        active_connections: (device.active_connections || 0) + 1,
      }));
    case 'session.ended':
      return patchWhere(devices, isPeer, (device) => ({
        active_connections: Math.max(0, (device.active_connections || 0) - 1),
      }));
    default:
      return devices;
  }
}

export function useLiveDevices() {
  const [devices, setDevices] = useState([]);
  const [loading, setLoading] = useState(true);
  const [live, setLive] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const response = await api.get('/api/portal/devices');
      setDevices(response.devices || []);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh().catch(() => {});
  }, [refresh]);

  useEffect(() => {
    const interval = setInterval(() => {
      if (document.visibilityState === 'visible') {
        refresh().catch(() => {});
      }
    }, live ? REFRESH_INTERVAL_LIVE_MS : REFRESH_INTERVAL_OFFLINE_MS);

    return () => clearInterval(interval);
  }, [live, refresh]);

  useEffect(() => {
    let socket;
    let reconnectTimer;
    let disposed = false;

    const connect = () => {
      socket = new WebSocket(LIVE_EVENTS_URL);
      socket.onopen = () => setLive(true);
      socket.onerror = () => socket.close();
      socket.onmessage = (message) => {
        try {
          const event = JSON.parse(message.data);
          setDevices((current) => applyLiveEvent(current, event));
        } catch {
          return;
        }
      };
      socket.onclose = () => {
        if (disposed) return;
        setLive(false);
        reconnectTimer = setTimeout(connect, RECONNECT_DELAY_MS);
      };
    };

    connect();

    return () => {
      disposed = true;
      clearTimeout(reconnectTimer);
      socket.close();
    };
  }, []);

  const patchDevice = useCallback((deviceId, patch) => {
    setDevices((current) => patchWhere(current, (device) => device.id === deviceId, () => patch));
  }, []);

  const forgetDevice = useCallback((deviceId) => {
    setDevices((current) => current.filter((device) => device.id !== deviceId));
  }, []);

  return { devices, loading, patchDevice, forgetDevice };
}
