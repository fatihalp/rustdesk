import React, { useState, useEffect } from 'react';
import { api } from '../api';
import { useAuth } from '../context/AuthContext';
import {
  Upload,
  Check,
  ShieldCheck,
  Sparkles,
  Globe,
  Palette,
  Eye,
  Monitor,
  ExternalLink
} from 'lucide-react';

export default function BrandSettings() {
  const { updateClientApp } = useAuth();
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [successMsg, setSuccessMsg] = useState(null);
  const [errorMsg, setErrorMsg] = useState(null);

  const [company, setCompany] = useState('');
  const [appName, setAppName] = useState('');
  const [slug, setSlug] = useState('');
  const [unattended, setUnattended] = useState(true);
  const [primaryColor, setPrimaryColor] = useState('#2563eb');
  const [website, setWebsite] = useState('');
  const [logoPreview, setLogoPreview] = useState(null);
  const [logoFile, setLogoFile] = useState(null);
  const [shortUrl, setShortUrl] = useState('');

  const fetchBrand = async () => {
    try {
      setLoading(true);
      const res = await api.get('/api/portal/brand');
      setCompany(res.customer?.company || res.customer?.name || '');
      setAppName(res.client_app?.app_name || '');
      setSlug(res.client_app?.slug || '');
      setUnattended(res.client_app?.unattended ?? true);
      setPrimaryColor(res.client_app?.primary_color || '#2563eb');
      setWebsite(res.client_app?.website || '');
      setLogoPreview(res.client_app?.logo_url || null);
      setShortUrl(res.client_app?.short_url || '');
    } catch (err) {
      setErrorMsg(err.message || 'Failed to load brand settings.');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchBrand();
  }, []);

  const handleLogoChange = (e) => {
    const file = e.target.files[0];
    if (file) {
      setLogoFile(file);
      setLogoPreview(URL.createObjectURL(file));
    }
  };

  const handleSave = async (e) => {
    e.preventDefault();
    setSaving(true);
    setSuccessMsg(null);
    setErrorMsg(null);

    const formData = new FormData();
    formData.append('company', company);
    formData.append('app_name', appName);
    formData.append('slug', slug);
    formData.append('unattended', unattended ? '1' : '0');
    formData.append('primary_color', primaryColor);
    if (website) formData.append('website', website);
    if (logoFile) formData.append('logo', logoFile);

    try {
      const res = await api.postForm('/api/portal/brand', formData);
      setSuccessMsg('Brand settings saved successfully!');
      updateClientApp(res.client_app);
      setShortUrl(res.client_app?.short_url || '');
      setTimeout(() => setSuccessMsg(null), 3000);
    } catch (err) {
      setErrorMsg(err.message || 'Failed to save changes.');
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="rounded-2xl bg-slate-900 border border-slate-800 p-12 text-center text-slate-400">
        <p className="text-sm">Loading brand settings...</p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <div className="lg:col-span-2 space-y-6">
        <div className="rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-xl">
          <div className="border-b border-slate-800 pb-4 mb-6">
            <h2 className="text-lg font-bold text-white flex items-center gap-2">
              <Sparkles className="h-5 w-5 text-blue-500" />
              Brand & Client Configuration
            </h2>
            <p className="text-xs text-slate-400 mt-1">
              Configure your custom client application name, logo, and unattended access preferences.
            </p>
          </div>

          {successMsg && (
            <div className="mb-6 rounded-xl bg-emerald-500/10 border border-emerald-500/30 p-3.5 text-xs text-emerald-300 flex items-center gap-2">
              <Check className="h-4 w-4 text-emerald-400" />
              <span>{successMsg}</span>
            </div>
          )}

          {errorMsg && (
            <div className="mb-6 rounded-xl bg-red-500/10 border border-red-500/30 p-3.5 text-xs text-red-400">
              {errorMsg}
            </div>
          )}

          <form onSubmit={handleSave} className="space-y-5">
            <div>
              <label className="block text-xs font-semibold text-slate-300 mb-2">
                Company Logo
              </label>
              <div className="flex items-center gap-5">
                <div className="h-20 w-20 rounded-2xl bg-slate-950 border border-slate-800 flex items-center justify-center overflow-hidden shrink-0 shadow-inner">
                  {logoPreview ? (
                    <img src={logoPreview} alt="" className="h-full w-full object-cover" />
                  ) : (
                    <Monitor className="h-8 w-8 text-slate-600" />
                  )}
                </div>
                <div>
                  <label className="cursor-pointer inline-flex items-center gap-2 rounded-xl bg-slate-800 hover:bg-slate-700 px-4 py-2.5 text-xs font-semibold text-white transition border border-slate-700">
                    <Upload className="h-4 w-4 text-slate-400" />
                    <span>Upload Logo (PNG/JPG)</span>
                    <input
                      type="file"
                      accept="image/png,image/jpeg,image/webp,image/svg+xml"
                      onChange={handleLogoChange}
                      className="hidden"
                    />
                  </label>
                  <p className="text-[11px] text-slate-500 mt-1.5">
                    Recommended size: 512x512px square image with transparent background.
                  </p>
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-300 mb-1.5">
                  Company Name
                </label>
                <input
                  type="text"
                  required
                  value={company}
                  onChange={(e) => setCompany(e.target.value)}
                  className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3.5 py-2.5 text-sm text-white focus:border-blue-500 focus:outline-none"
                  placeholder="e.g. Visiosoft Inc."
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-300 mb-1.5">
                  Application / Client Name
                </label>
                <input
                  type="text"
                  required
                  value={appName}
                  onChange={(e) => setAppName(e.target.value)}
                  className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3.5 py-2.5 text-sm text-white focus:border-blue-500 focus:outline-none"
                  placeholder="e.g. Visiosoft Support"
                />
              </div>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-300 mb-1.5">
                Short Link Slug
              </label>
              <div className="flex items-center rounded-xl bg-slate-950 border border-slate-800 overflow-hidden focus-within:border-blue-500">
                <span className="px-3.5 text-xs text-slate-500 select-none bg-slate-900 border-r border-slate-800 py-2.5">
                  {window.location.origin}/s/
                </span>
                <input
                  type="text"
                  required
                  value={slug}
                  onChange={(e) => setSlug(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                  className="w-full bg-transparent px-3 py-2.5 text-sm text-white focus:outline-none font-mono"
                  placeholder="visiosoft"
                />
              </div>
              <p className="text-[11px] text-slate-500 mt-1">
                Your customers can open this short URL directly in their browser to download the client.
              </p>
            </div>

            <div className="rounded-xl bg-slate-950 border border-slate-800/80 p-4">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <div className="flex items-center gap-2">
                    <ShieldCheck className="h-4 w-4 text-emerald-400" />
                    <span className="text-sm font-bold text-white">Authorized Unattended Access</span>
                  </div>
                  <p className="text-xs text-slate-400 mt-1 max-w-lg">
                    When enabled, your support agents can connect to customer machines with a single click without asking for a password. Customers can toggle this off inside their client at any time.
                  </p>
                </div>
                <input
                  type="checkbox"
                  checked={unattended}
                  onChange={(e) => setUnattended(e.target.checked)}
                  className="h-5 w-5 rounded bg-slate-900 border-slate-700 text-blue-600 focus:ring-0 mt-1"
                />
              </label>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-300 mb-1.5 flex items-center gap-1.5">
                  <Palette className="h-3.5 w-3.5 text-slate-400" />
                  Brand Color
                </label>
                <div className="flex items-center gap-3">
                  <input
                    type="color"
                    value={primaryColor}
                    onChange={(e) => setPrimaryColor(e.target.value)}
                    className="h-10 w-12 rounded-xl bg-slate-950 border border-slate-800 p-1 cursor-pointer"
                  />
                  <input
                    type="text"
                    value={primaryColor}
                    onChange={(e) => setPrimaryColor(e.target.value)}
                    className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3 py-2 text-xs font-mono text-white"
                  />
                </div>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-300 mb-1.5 flex items-center gap-1.5">
                  <Globe className="h-3.5 w-3.5 text-slate-400" />
                  Website (Optional)
                </label>
                <input
                  type="url"
                  value={website}
                  onChange={(e) => setWebsite(e.target.value)}
                  className="w-full rounded-xl bg-slate-950 border border-slate-800 px-3.5 py-2.5 text-sm text-white focus:border-blue-500 focus:outline-none"
                  placeholder="https://yourcompany.com"
                />
              </div>
            </div>

            <div className="pt-4 border-t border-slate-800 flex justify-end">
              <button
                type="submit"
                disabled={saving}
                className="flex items-center gap-2 rounded-xl bg-blue-600 hover:bg-blue-500 px-5 py-2.5 text-xs font-bold text-black shadow-lg transition disabled:opacity-50"
              >
                {saving ? 'Saving...' : 'Save Changes'}
              </button>
            </div>
          </form>
        </div>
      </div>

      <div className="space-y-6">
        <div className="rounded-2xl bg-slate-900 border border-slate-800 p-6 shadow-xl sticky top-24">
          <h3 className="text-sm font-bold text-white flex items-center gap-2 mb-4">
            <Eye className="h-4 w-4 text-blue-400" />
            Customer Download Page Preview
          </h3>

          <div className="rounded-2xl bg-white p-6 text-center text-slate-900 shadow-xl border border-slate-200">
            <div className="h-16 w-16 rounded-2xl mx-auto mb-3 overflow-hidden bg-slate-100 flex items-center justify-center border border-slate-200 shadow-sm">
              {logoPreview ? (
                <img src={logoPreview} alt="" className="h-full w-full object-cover" />
              ) : (
                <Monitor className="h-8 w-8 text-slate-400" />
              )}
            </div>

            <h4 className="text-base font-extrabold text-slate-900 tracking-tight">
              {appName || 'Remote Support'}
            </h4>
            <p className="text-xs text-slate-500 mt-1 mb-4">
              Download the app and open it. Your support specialist will assist you right away.
            </p>

            <div
              className="w-full py-2.5 rounded-xl font-bold text-xs text-white shadow-md transition"
              style={{ backgroundColor: primaryColor }}
            >
              Download for macOS
            </div>
            <div className="w-full py-2.5 rounded-xl font-bold text-xs text-slate-700 bg-slate-100 hover:bg-slate-200 mt-2 transition">
              Download for Windows
            </div>

            <p className="text-[10px] text-slate-400 mt-4">
              Securely Provided by {company}
            </p>
          </div>

          <div className="mt-4 pt-4 border-t border-slate-800">
            <span className="text-xs text-slate-400 block mb-1">Customer Short Link:</span>
            <a
              href={`/s/${slug}`}
              target="_blank"
              rel="noreferrer"
              className="text-xs text-blue-400 hover:text-blue-300 font-mono flex items-center gap-1 truncate"
            >
              <span>{window.location.origin}/s/{slug}</span>
              <ExternalLink className="h-3 w-3 shrink-0" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
