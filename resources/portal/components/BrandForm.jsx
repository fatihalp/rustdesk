import React, { useEffect, useState } from 'react';
import { Monitor } from 'lucide-react';
import { api } from '../api';
import { useAuth } from '../context/AuthContext';
import { card, errorNotice, fieldLabel, input, primaryButton } from '../styles';

const SAVED_CONFIRMATION_MS = 2500;

export default function BrandForm() {
  const { updateClientApp } = useAuth();
  const [brand, setBrand] = useState(null);
  const [logoFile, setLogoFile] = useState(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    api
      .get('/api/portal/brand')
      .then((response) =>
        setBrand({
          company: response.customer.company || '',
          appName: response.client_app.app_name,
          color: response.client_app.primary_color,
          unattended: response.client_app.unattended,
          logoUrl: response.client_app.logo_url,
        })
      )
      .catch((failure) => setError(failure.message));
  }, []);

  const change = (changes) => setBrand((current) => ({ ...current, ...changes }));

  const chooseLogo = (event) => {
    const file = event.target.files[0];
    if (!file) return;

    setLogoFile(file);
    change({ logoUrl: URL.createObjectURL(file) });
  };

  const save = async (event) => {
    event.preventDefault();
    setSaving(true);
    setError(null);

    const form = new FormData();
    form.append('company', brand.company);
    form.append('app_name', brand.appName);
    form.append('primary_color', brand.color);
    form.append('unattended', brand.unattended ? '1' : '0');
    if (logoFile) form.append('logo', logoFile);

    try {
      const response = await api.post('/api/portal/brand', form);
      updateClientApp(response.client_app);
      setLogoFile(null);
      setSaved(true);
      setTimeout(() => setSaved(false), SAVED_CONFIRMATION_MS);
    } catch (failure) {
      setError(failure.message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <section>
      <h1 className="mb-4 text-2xl font-semibold">Your brand</h1>
      {error && <p className={`${errorNotice} mb-4`}>{error}</p>}

      {brand && (
        <form onSubmit={save} className={`${card} space-y-6 p-6 sm:p-8`}>
          <label className="flex cursor-pointer items-center gap-4">
            <span className="flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-2xl bg-black">
              {brand.logoUrl ? (
                <img src={brand.logoUrl} alt="" className="h-full w-full object-cover" />
              ) : (
                <Monitor className="h-7 w-7 text-slate-500" />
              )}
            </span>
            <span>
              <span className="block font-medium">Logo</span>
              <span className="block text-sm text-slate-400">Choose a square PNG or JPG</span>
            </span>
            <input type="file" accept="image/png,image/jpeg" onChange={chooseLogo} className="sr-only" />
          </label>

          <div className="grid gap-4 sm:grid-cols-2">
            <label className="block">
              <span className={fieldLabel}>App name</span>
              <input
                required
                maxLength={60}
                value={brand.appName}
                onChange={(event) => change({ appName: event.target.value })}
                className={input}
              />
            </label>
            <label className="block">
              <span className={fieldLabel}>Company</span>
              <input
                value={brand.company}
                onChange={(event) => change({ company: event.target.value })}
                className={input}
              />
            </label>
          </div>

          <label className="flex items-center justify-between gap-4">
            <span>
              <span className="block font-medium">Color</span>
              <span className="block text-sm text-slate-400">Shown on your download page</span>
            </span>
            <input
              type="color"
              value={brand.color}
              onChange={(event) => change({ color: event.target.value })}
              className="h-9 w-14 cursor-pointer rounded-lg border border-slate-700 bg-black p-1"
            />
          </label>

          <div className="flex items-center justify-between gap-4">
            <span id="unattended-label">
              <span className="block font-medium">Connect without a password</span>
              <span className="block text-sm text-slate-400">
                Reach customer computers in one click. Customers can turn this off in the app.
              </span>
            </span>
            <button
              type="button"
              role="switch"
              aria-checked={brand.unattended}
              aria-labelledby="unattended-label"
              onClick={() => change({ unattended: !brand.unattended })}
              className={`relative h-7 w-12 shrink-0 rounded-full transition ${
                brand.unattended ? 'bg-[#30d158]' : 'bg-slate-700'
              }`}
            >
              <span
                className={`absolute left-0.5 top-0.5 h-6 w-6 rounded-full bg-white transition-transform ${
                  brand.unattended ? 'translate-x-5' : ''
                }`}
              />
            </button>
          </div>

          <div className="flex items-center gap-4 border-t border-slate-800 pt-6">
            <button type="submit" disabled={saving} className={primaryButton}>
              Save
            </button>
            {saved && <span className="text-sm text-slate-400">Saved</span>}
          </div>
        </form>
      )}
    </section>
  );
}
