import React, { createContext, useContext, useEffect, useState } from 'react';
import { api } from '../api';

const AuthContext = createContext(null);

const SIGNED_OUT = { user: null, customer: null, clientApp: null };

function sessionFrom(response) {
  return { user: response.user, customer: response.customer, clientApp: response.client_app };
}

export function AuthProvider({ children }) {
  const [session, setSession] = useState(SIGNED_OUT);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api
      .get('/api/portal/auth/me')
      .then((response) => setSession(sessionFrom(response)))
      .catch(() => setSession(SIGNED_OUT))
      .finally(() => setLoading(false));
  }, []);

  const login = async (email, password) => {
    setSession(sessionFrom(await api.post('/api/portal/auth/login', { email, password })));
  };

  const register = async (account) => {
    setSession(sessionFrom(await api.post('/api/portal/auth/register', account)));
  };

  const oauth = (provider) => {
    window.location.href = `/auth/${provider}/redirect`;
  };

  const logout = async () => {
    try {
      await api.post('/api/portal/auth/logout');
    } finally {
      setSession(SIGNED_OUT);
    }
  };

  const updateClientApp = (changes) => {
    setSession((current) => ({ ...current, clientApp: { ...current.clientApp, ...changes } }));
  };

  return (
    <AuthContext.Provider value={{ ...session, loading, login, register, oauth, logout, updateClientApp }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}
