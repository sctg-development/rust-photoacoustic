import { defineConfig } from 'vite';
import fs from 'fs';
import path from 'path';

// When built from build.rs, exclude openapi.json from public assets
const isBuiltByBuildRs = process.env.BUILT_BY_BUILD_RS === 'true';

// Get the base path from environment variable, default to /api/doc/ when built from build.rs
const base = process.env.VITE_BASE_PATH || (isBuiltByBuildRs ? '/api/doc/' : '/');

export default defineConfig({
  root: '.',
  base: base,
  publicDir: isBuiltByBuildRs ? false : 'public',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    minify: 'terser',
    sourcemap: true,
    rollupOptions: {
      input: 'index.html',
      output: {
        entryFileNames: 'helper.js',
        chunkFileNames: '[name].js',
        assetFileNames: '[name].[ext]',
      },
    },
  },
  server: {
    port: 8080,
    open: false,
    cors: true,
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, PATCH, OPTIONS',
      'Access-Control-Allow-Headers': 'X-Requested-With, content-type, Authorization',
    },
  },
  resolve: {
    alias: {
      process: 'process/browser',
    },
  },
  define: {
    global: 'globalThis',
  },
});
