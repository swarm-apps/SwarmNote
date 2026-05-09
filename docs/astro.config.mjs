// @ts-check
import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

// 部署目标：当前为 GitHub Pages 子路径（https://swarm-apps.github.io/SwarmNote/）。
// 切到 swarmnote.app 自定义域名时，把 SITE 改成 'https://swarmnote.app'、BASE 改成 '/' 即可。
const SITE = 'https://swarm-apps.github.io';
const BASE = '/SwarmNote/';

// https://astro.build/config
export default defineConfig({
  site: SITE,
  base: BASE,
  trailingSlash: 'ignore',

  integrations: [react(), sitemap()],

  vite: {
    plugins: [tailwindcss()],
  },

  i18n: {
    locales: ['zh', 'en'],
    defaultLocale: 'zh',
    routing: {
      prefixDefaultLocale: false,
    },
  },
});
