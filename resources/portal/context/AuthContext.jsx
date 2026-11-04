import React, { createContext, useContext, useState, useEffect } from 'react';
import { api } from '../api';

const AuthContext = createContext(null);

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null);
  const [customer, setCustomer] = useState(null);
  const [clientApp, setClientApp] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchMe = async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await api.get('/api/portal/auth/me');
      if (res.authenticated) {
        setUser(res.user);
        setCustomer(res.customer);
        setClientApp(res.client_app);
      } else {
        setUser(null);
        setCustomer(null);
        setClientApp(null);
      }
    } catch (err) {
      setUser(null);
      setCustomer(null);
      setClientApp(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMe();
  }, []);

  const login = async (email, password) => {
    setError(null);
    try {
      const res = await api.post('/api/portal/auth/login', { email, password });
      setUser(res.user);
      setCustomer(res.customer);
      setClientApp(res.client_app);
      return res;
    } catch (err) {
      setError(err.message);
      throw err;
    }
  };

  const register = async (data) => {
    setError(null);
    try {
      const res = await api.post('/api/portal/auth/register', data);
      setUser(res.user);
      setCustomer(res.customer);
      setClientApp(res.client_app);
      return res;
    } catch (err) {
      setError(err.message);
      throw err;
    }
  };

  const oauth = (provider) => {
    window.location.href = `/auth/${provider}/redirect`;
  };

  const logout = async () => {
    try {
      await api.post('/api/portal/auth/logout');
    } finally {
      setUser(null);
      setCustomer(null);
      setClientApp(null);
    }
  };

  const updateClientApp = (newApp) => {
    setClientApp((prev) => ({ ...prev, ...newApp }));
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        customer,
        clientApp,
        loading,
        error,
        login,
        register,
        oauth,
        logout,
        refreshUser: fetchMe,
        updateClientApp,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return ctx;
}
