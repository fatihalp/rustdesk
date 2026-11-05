import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  publicDir: false,
  build: {
    outDir: 'public/portal-dist',
    emptyOutDir: true,
    rollupOptions: {
      input: 'resources/portal/main.jsx',
      output: {
        entryFileNames: 'assets/portal.js',
        chunkFileNames: 'assets/[name].js',
        assetFileNames: (assetInfo) => {
          if (assetInfo.name && assetInfo.name.endsWith('.css')) {
            return 'assets/portal.css';
          }
          return 'assets/[name].[ext]';
        },
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './resources/portal'),
    },
  },
});
