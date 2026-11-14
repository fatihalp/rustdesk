import React, { useState } from 'react';
import { AuthProvider, useAuth } from './context/AuthContext';
import Header from './components/Header';
import AuthView from './components/AuthView';
import Devices from './components/Devices';
import Settings from './components/Settings';

function Portal() {
  const { user, loading } = useAuth();
  const [page, setPage] = useState('devices');

  if (loading) {
    return <div className="min-h-screen bg-black" />;
  }

  if (!user) {
    return <AuthView />;
  }

  return (
    <div className="min-h-screen bg-black text-white">
      <Header page={page} onNavigate={setPage} />
      <main className="mx-auto w-full max-w-3xl px-5 py-10 sm:py-14">
        {page === 'devices' ? <Devices /> : <Settings />}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <Portal />
    </AuthProvider>
  );
}
