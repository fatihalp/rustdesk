import React, { useState } from 'react';
import { Check, Copy } from 'lucide-react';

const CONFIRMATION_MS = 2000;

export default function CopyButton({ value, label, className }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), CONFIRMATION_MS);
  };

  return (
    <button type="button" onClick={copy} className={className}>
      {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
      <span>{copied ? 'Copied' : label}</span>
    </button>
  );
}
