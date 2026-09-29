import { defineConfig } from 'vite';

export default defineConfig({
  server: {
    host: '127.0.0.1',
    port: 1421,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/target/**'] },
  },
  clearScreen: false,
});
