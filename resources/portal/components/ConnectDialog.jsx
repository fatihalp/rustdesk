import React, { useEffect, useState } from 'react';
import { api } from '../api';
import { errorNotice, primaryButton, secondaryButton } from '../styles';
import CopyButton from './CopyButton';
import Dialog from './Dialog';

const copyLink = 'inline-flex items-center gap-1.5 text-sm text-slate-400 transition hover:text-white';

function Credential({ label, value }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="min-w-0">
        <p className="text-xs text-slate-500">{label}</p>
        <p className="truncate font-mono text-sm">{value}</p>
      </div>
      <CopyButton value={value} label="Copy" className={copyLink} />
    </div>
  );
}

export default function ConnectDialog({ device, onClose }) {
  const [connection, setConnection] = useState(null);
  const [error, setError] = useState(null);

  useEffect(() => {
    api
      .get(`/api/portal/devices/${device.id}/connect`)
      .then(setConnection)
      .catch((failure) => setError(failure.message));
  }, [device.id]);

  const password = connection?.unattended_password;
  const sessionUri = connection?.connect_uri || `supportdesk://connection/new/${device.supportdesk_id}`;

  return (
    <Dialog
      title={`Connect to ${device.hostname}`}
      description={
        device.has_unattended_password
          ? 'Your support app opens and connects right away.'
          : 'Your support app opens. Ask your customer for the password shown on their screen.'
      }
      onClose={onClose}
    >
      {error && <p className={`${errorNotice} mb-4`}>{error}</p>}

      <div className="divide-y divide-slate-800 rounded-2xl bg-black px-4">
        <Credential label="Device ID" value={device.supportdesk_id} />
        {password && <Credential label="Password" value={password} />}
      </div>

      <div className="mt-6 flex flex-col gap-2">
        <a href={sessionUri} className={primaryButton}>
          Open Remote Session
        </a>
        <button type="button" onClick={onClose} className={secondaryButton}>
          Close
        </button>
      </div>
    </Dialog>
  );
}
