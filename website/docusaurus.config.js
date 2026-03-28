// @ts-check

import {themes as prismThemes} from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'AI-Native Messaging Protocol',
  tagline: 'Where humans and agents are equal participants in conversations',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://gericke98.github.io',
  baseUrl: '/ai-native-messaging/',

  organizationName: 'gericke98',
  projectName: 'ai-native-messaging',
  trailingSlash: false,

  onBrokenLinks: 'warn',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          routeBasePath: '/',
          editUrl: 'https://github.com/gericke98/ai-native-messaging/tree/main/website/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
    [
      'docusaurus-asyncapi',
      /** @type {any} */
      ({
        id: 'asyncapi',
        spec: '../asyncapi.yaml',
        route: '/asyncapi/',
        config: {
          show: {
            sidebar: true,
          },
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: 'AINM Protocol',
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docs',
            position: 'left',
            label: 'Docs',
          },
          {to: '/asyncapi', label: 'AsyncAPI', position: 'left'},
          {
            href: 'https://github.com/gericke98/ai-native-messaging',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Documentation',
            items: [
              {
                label: 'Introduction',
                to: '/',
              },
              {
                label: 'Specification',
                to: '/spec/identity',
              },
              {
                label: 'AsyncAPI Explorer',
                to: '/asyncapi',
              },
            ],
          },
          {
            title: 'Resources',
            items: [
              {
                label: 'Demo',
                to: '/demo',
              },
              {
                label: 'Changelog',
                to: '/changelog',
              },
              {
                label: 'GitHub',
                href: 'https://github.com/gericke98/ai-native-messaging',
              },
            ],
          },
        ],
        copyright: `AI-Native Messaging Protocol v0.1.0`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['rust', 'json', 'bash'],
      },
    }),
};

export default config;
