import React, { useState } from 'react';
import { api } from '../api';
import { errorNotice, fieldLabel, input, primaryButton, secondaryButton } from '../styles';
import Dialog from './Dialog';

export default function EditDeviceDialog({ device, onSaved, onClose }) {
  const [hostname, setHostname] = useState(device.hostname || '');
  const [notes, setNotes] = useState(device.notes || '');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState(null);

  const save = async (event) => {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const response = await api.put(`/api/portal/devices/${device.id}`, { hostname, notes });
      onSaved({ hostname: response.device.hostname, notes: response.device.notes });
    } catch (failure) {
      setError(failure.message);
      setSaving(false);
    }
  };

  return (
    <Dialog title="Rename Device" onClose={onClose}>
      <form onSubmit={save} className="space-y-4">
        {error && <p className={errorNotice}>{error}</p>}
        <label className="block">
          <span className={fieldLabel}>Name</span>
          <input
            autoFocus
            value={hostname}
            onChange={(event) => setHostname(event.target.value)}
            placeholder="Front desk computer"
            className={input}
          />
        </label>
        <label className="block">
          <span className={fieldLabel}>Note</span>
          <input
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            placeholder="Optional"
            className={input}
          />
        </label>
        <div className="flex flex-col gap-2 pt-2">
          <button type="submit" disabled={saving} className={primaryButton}>
            Save
          </button>
          <button type="button" onClick={onClose} className={secondaryButton}>
            Cancel
          </button>
        </div>
      </form>
    </Dialog>
  );
}
