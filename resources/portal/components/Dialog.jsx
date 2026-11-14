import React, { useEffect } from 'react';

export default function Dialog({ title, description, onClose, children }) {
  useEffect(() => {
    const closeOnEscape = (event) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', closeOnEscape);

    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-5 backdrop-blur-sm"
      onMouseDown={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
        className="w-full max-w-sm rounded-3xl border border-slate-800 bg-slate-900 p-7"
      >
        <h2 className="text-xl font-semibold">{title}</h2>
        {description && <p className="mt-1.5 text-sm text-slate-400">{description}</p>}
        <div className="mt-6">{children}</div>
      </div>
    </div>
  );
}
