import React, { useState } from 'react';
import { AuthProvider, useAuth } from './context/AuthContext';
import Navbar from './components/Navbar';
import AuthView from './components/AuthView';
import DeviceList from './components/DeviceList';
import BrandSettings from './components/BrandSettings';
import BuildManager from './components/BuildManager';
import ShortLinks from './components/ShortLinks';
import { RefreshCw, Monitor } from 'lucide-react';

function Dashboard() {
  const { user, loading } = useAuth();
  const [activeTab, setActiveTab] = useState('devices');

  if (loading) {
    return (
      <div className="min-h-screen bg-slate-950 flex flex-col items-center justify-center">
        <div className="h-14 w-14 rounded-2xl bg-gradient-to-tr from-blue-600 to-indigo-500 flex items-center justify-center text-black shadow-xl mb-4 animate-pulse">
          <Monitor className="h-7 w-7" />
        </div>
        <p className="text-xs text-slate-400 font-medium">Loading...</p>
      </div>
    );
  }

  if (!user) {
    return <AuthView />;
  }

  return (
    <div className="min-h-screen bg-slate-950 flex flex-col text-slate-100">
      <Navbar activeTab={activeTab} setActiveTab={setActiveTab} />

      <main className="flex-1 mx-auto w-full max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        {activeTab === 'devices' && <DeviceList onGoToLinks={() => setActiveTab('links')} />}
        {activeTab === 'brand' && <BrandSettings />}
        {activeTab === 'builds' && <BuildManager />}
        {activeTab === 'links' && <ShortLinks />}
      </main>

      <footer className="border-t border-slate-900 bg-slate-950/60 py-6 text-center text-xs text-slate-500">
        <p>&copy; {new Date().getFullYear()} SupportDesk Multi-Tenant Remote Support Platform</p>
      </footer>
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <Dashboard />
    </AuthProvider>
  );
}
