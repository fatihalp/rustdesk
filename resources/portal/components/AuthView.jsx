import React, { useState } from 'react';
import { Monitor } from 'lucide-react';
import { useAuth } from '../context/AuthContext';
import { errorNotice, input, primaryButton, secondaryButton } from '../styles';

const MODES = {
  login: {
    title: 'Sign in',
    submit: 'Sign In',
    switchPrompt: 'New here?',
    switchAction: 'Create an account',
    next: 'register',
  },
  register: {
    title: 'Create your account',
    submit: 'Create Account',
    switchPrompt: 'Already have an account?',
    switchAction: 'Sign in',
    next: 'login',
  },
};

const PROVIDERS = [
  { id: 'google', label: 'Continue with Google' },
  { id: 'apple', label: 'Continue with Apple' },
];

export default function AuthView() {
  const { login, register, oauth } = useAuth();
  const [mode, setMode] = useState('login');
  const [fields, setFields] = useState({ name: '', company: '', email: '', password: '' });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);
  const copy = MODES[mode];

  const field = (name, props) => (
    <input
      required
      aria-label={props.placeholder}
      value={fields[name]}
      onChange={(event) => setFields({ ...fields, [name]: event.target.value })}
      className={input}
      {...props}
    />
  );

  const submit = async (event) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await (mode === 'login' ? login(fields.email, fields.password) : register(fields));
    } catch (failure) {
      setError(failure.message);
      setSubmitting(false);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-black px-5 py-12 text-white">
      <div className="w-full max-w-sm">
        <div className="mx-auto mb-6 flex h-14 w-14 items-center justify-center rounded-2xl bg-slate-900">
          <Monitor className="h-7 w-7" />
        </div>
        <h1 className="text-center text-3xl font-semibold">{copy.title}</h1>
        <p className="mt-2 text-center text-slate-400">Remote support under your own brand.</p>

        <form onSubmit={submit} className="mt-8 space-y-3">
          {error && <p className={errorNotice}>{error}</p>}
          {mode === 'register' && field('name', { placeholder: 'Full name', autoComplete: 'name' })}
          {mode === 'register' && field('company', { placeholder: 'Company', autoComplete: 'organization' })}
          {field('email', { type: 'email', placeholder: 'Email', autoComplete: 'email' })}
          {field('password', {
            type: 'password',
            placeholder: 'Password',
            minLength: mode === 'register' ? 8 : undefined,
            autoComplete: mode === 'login' ? 'current-password' : 'new-password',
          })}
          <button type="submit" disabled={submitting} className={`${primaryButton} w-full py-2.5`}>
            {copy.submit}
          </button>
        </form>

        <div className="my-6 flex items-center gap-4 text-sm text-slate-500">
          <span className="h-px flex-1 bg-slate-800" />
          or
          <span className="h-px flex-1 bg-slate-800" />
        </div>

        <div className="space-y-3">
          {PROVIDERS.map(({ id, label }) => (
            <button key={id} type="button" onClick={() => oauth(id)} className={`${secondaryButton} w-full py-2.5`}>
              {label}
            </button>
          ))}
        </div>

        <p className="mt-8 text-center text-sm text-slate-400">
          {copy.switchPrompt}{' '}
          <button
            type="button"
            onClick={() => {
              setMode(copy.next);
              setError(null);
            }}
            className="text-white underline-offset-4 hover:underline"
          >
            {copy.switchAction}
          </button>
        </p>
      </div>
    </div>
  );
}
