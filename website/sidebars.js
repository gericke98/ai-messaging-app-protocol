// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    'intro',
    {
      type: 'category',
      label: 'Specification',
      collapsed: false,
      items: [
        'spec/identity',
        'spec/security',
        'spec/transport',
        'spec/content',
        'spec/interactive',
        'spec/threading',
        'spec/streaming',
        'spec/agent-card',
        'spec/memory',
        'spec/tool-visibility',
        'spec/compatibility',
      ],
    },
    'asyncapi',
    'demo',
    'changelog',
  ],
};

export default sidebars;
