import React from 'react';
import AppBuilds from './AppBuilds';
import BrandForm from './BrandForm';

export default function Settings() {
  return (
    <div className="space-y-10">
      <BrandForm />
      <AppBuilds />
    </div>
  );
}
