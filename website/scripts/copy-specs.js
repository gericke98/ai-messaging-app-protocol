const fs = require('fs');
const path = require('path');

// Copy spec/ files to docs/spec/ with Docusaurus frontmatter
const specDir = path.join(__dirname, '../../spec');
const docsSpecDir = path.join(__dirname, '../docs/spec');
fs.mkdirSync(docsSpecDir, { recursive: true });

const specOrder = [
  'identity', 'security', 'transport', 'content', 'interactive',
  'threading', 'streaming', 'agent-card', 'memory', 'tool-visibility', 'compatibility'
];

specOrder.forEach((name, index) => {
  const src = path.join(specDir, `${name}.md`);
  const dest = path.join(docsSpecDir, `${name}.md`);

  if (!fs.existsSync(src)) {
    console.warn(`Warning: spec file not found: ${src}`);
    return;
  }

  let content = fs.readFileSync(src, 'utf8');

  // Extract title from first H1 heading
  const titleMatch = content.match(/^#\s+(.+)/m);
  const title = titleMatch ? titleMatch[1] : name;

  // Remove existing frontmatter-like lines at top (Status, Version, Last Updated)
  // These are not YAML frontmatter but status lines that could conflict
  content = content.replace(/^---\n[\s\S]*?\n---\n/, '');

  // Remove the first H1 line (Docusaurus uses title from frontmatter)
  content = content.replace(/^#\s+.+\n/, '');

  // Remove Status/Version/Last Updated lines that appear after the heading
  content = content.replace(/^\*\*Status:\*\*.*\n/m, '');
  content = content.replace(/^\*\*Version:\*\*.*\n/m, '');
  content = content.replace(/^\*\*Last Updated:\*\*.*\n/m, '');

  // Prepend Docusaurus frontmatter
  const frontmatter = `---\nsidebar_position: ${index + 1}\ntitle: "${title}"\n---\n\n`;

  fs.writeFileSync(dest, frontmatter + content.trimStart());
});

console.log(`Copied ${specOrder.length} spec files to docs/spec/`);

// Copy schemas to static/schemas for download
const schemasDir = path.join(__dirname, '../../schemas');
const staticSchemasDir = path.join(__dirname, '../static/schemas');
fs.mkdirSync(staticSchemasDir, { recursive: true });

if (fs.existsSync(schemasDir)) {
  const schemaFiles = fs.readdirSync(schemasDir).filter(f => f.endsWith('.json'));
  schemaFiles.forEach(file => {
    fs.copyFileSync(
      path.join(schemasDir, file),
      path.join(staticSchemasDir, file)
    );
  });
  console.log(`Copied ${schemaFiles.length} schema files to static/schemas/`);
} else {
  console.warn('Warning: schemas/ directory not found');
}

// Copy asyncapi.yaml to static for download
const asyncapiSrc = path.join(__dirname, '../../asyncapi.yaml');
const asyncapiDest = path.join(__dirname, '../static/asyncapi.yaml');
if (fs.existsSync(asyncapiSrc)) {
  fs.copyFileSync(asyncapiSrc, asyncapiDest);
  console.log('Copied asyncapi.yaml to static/');
}
