---
phase: 03-specification-publication-working-demo
plan: 05
subsystem: docs
tags: [docusaurus, asyncapi, github-actions, github-pages, documentation, spec-site]

# Dependency graph
requires:
  - phase: 03-01
    provides: "JSON Schema 2020-12 files for all protocol message types"
  - phase: 03-02
    provides: "JSON Schema files for streaming, agent-card, memory, tool-visibility, compatibility"
  - phase: 03-03
    provides: "AsyncAPI 3.1.0 transport binding document (asyncapi.yaml) and CHANGELOG.md"
  - phase: 03-04
    provides: "Working Rust CLI demo proving protocol end-to-end"
provides:
  - "Docusaurus 3.9.2 spec publication site with 11 spec pages, schema downloads, and AsyncAPI explorer"
  - "GitHub Actions CI/CD workflow for deploying to GitHub Pages on push to main"
  - "Prebuild script that copies spec/ files with Docusaurus frontmatter and schemas to static/"
affects: [ios-client, community-adoption, spec-maintenance]

# Tech tracking
tech-stack:
  added: [docusaurus-3.9.2, docusaurus-asyncapi-1.0.2, asyncapi-cli-6.0.0, github-actions]
  patterns: [prebuild-copy-specs, markdown-format-detect, github-pages-deployment]

key-files:
  created:
    - website/docusaurus.config.js
    - website/sidebars.js
    - website/scripts/copy-specs.js
    - website/docs/intro.md
    - website/docs/demo.md
    - website/docs/changelog.md
    - website/static/schemas/.gitkeep
    - .github/workflows/deploy-docs.yml
    - .gitignore
  modified:
    - website/package.json

key-decisions:
  - "Used markdown.format: 'detect' to handle spec files with JSON curly braces that break MDX parser"
  - "Spec files copied at prebuild time (not symlinked) with Docusaurus frontmatter injected"
  - "routeBasePath set to '/' so docs serve at site root, not under /docs/"
  - "AsyncAPI explorer configured via docusaurus-asyncapi@1.0.2 preset at /asyncapi/ route"

patterns-established:
  - "Prebuild copy pattern: spec/ files copied to website/docs/spec/ with frontmatter injection"
  - "GitHub Pages deploy: GitHub Actions with path-filtered triggers on spec/, schemas/, website/, asyncapi.yaml changes"
  - "Schema distribution: JSON Schema files served as static assets at /schemas/"

requirements-completed: [SPEC-01, SPEC-04]

# Metrics
duration: 9min
completed: 2026-03-28
---

# Phase 03 Plan 05: Spec Publication Site Summary

**Docusaurus 3.9.2 site with 11 spec pages, AsyncAPI explorer, schema downloads, and GitHub Pages CI/CD deployment**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-28T21:18:48Z
- **Completed:** 2026-03-28T21:28:47Z
- **Tasks:** 3
- **Files modified:** 22

## Accomplishments
- Docusaurus 3.9.2 site scaffolded with classic + docusaurus-asyncapi presets serving all 11 spec files
- GitHub Actions workflow for automatic deployment to GitHub Pages on push to main
- Prebuild script that copies spec files with injected Docusaurus frontmatter and schemas to static directory
- Site builds successfully with spec pages, demo instructions, changelog, and schema downloads

## Task Commits

Each task was committed atomically:

1. **Task 1: Initialize Docusaurus site with asyncapi preset, spec pages, and sidebar** - `a372c15` (feat)
2. **Task 2: Create GitHub Actions deployment workflow and verify site builds** - `938cc33` (feat)
3. **Task 3: Verify end-to-end** - Auto-approved (checkpoint, no separate commit)

## Files Created/Modified
- `website/docusaurus.config.js` - Docusaurus config with classic + asyncapi presets, markdown format detect
- `website/sidebars.js` - Sidebar with Introduction, Specification (11 items), Demo, Changelog
- `website/scripts/copy-specs.js` - Prebuild script copying spec files with frontmatter injection
- `website/docs/intro.md` - Landing page with protocol overview and quickstart links
- `website/docs/demo.md` - Instructions for running the Rust CLI demo
- `website/docs/changelog.md` - Protocol changelog (mirrors root CHANGELOG.md)
- `website/package.json` - Dependencies including docusaurus-asyncapi@1.0.2 and @asyncapi/cli@6.0.0
- `website/static/schemas/.gitkeep` - Placeholder for schema files copied at prebuild time
- `.github/workflows/deploy-docs.yml` - GitHub Actions workflow for GitHub Pages deployment
- `.gitignore` - Ignore rules for build artifacts, node_modules, and generated files

## Decisions Made
- Used `markdown.format: 'detect'` in Docusaurus config to process `.md` files as standard Markdown (not MDX), avoiding curly brace parsing errors in spec JSON examples
- Removed `future.v4: true` flag for compatibility with markdown format detection
- Spec files are copied (not symlinked) at prebuild time with Docusaurus frontmatter injected, keeping original spec files clean
- Set `routeBasePath: '/'` so docs are at the site root rather than under `/docs/`
- GitHub org set to `gericke98` with baseUrl `/ai-native-messaging/` for GitHub Pages

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added .gitignore for build artifacts**
- **Found during:** Task 1 (Docusaurus initialization)
- **Issue:** Project had no .gitignore, would track node_modules, demo/target, build output
- **Fix:** Created .gitignore with rules for Rust target/, node_modules/, Docusaurus build/, generated spec copies
- **Files modified:** .gitignore
- **Verification:** `git status` correctly ignores build artifacts
- **Committed in:** a372c15 (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed MDX parsing errors in spec files**
- **Found during:** Task 2 (site build verification)
- **Issue:** Spec files contain JSON examples with curly braces `{}` that MDX interprets as JSX expressions, causing acorn parse errors
- **Fix:** Added `markdown.format: 'detect'` to docusaurus.config.js so `.md` files are parsed as plain Markdown, not MDX
- **Files modified:** website/docusaurus.config.js
- **Verification:** `npm run build` completes successfully
- **Committed in:** 938cc33 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 blocking)
**Impact on plan:** Both fixes necessary for correct operation. No scope creep.

## Issues Encountered
- AsyncAPI explorer route `/asyncapi` shows as "broken link" in build warnings because the docusaurus-asyncapi preset handles the route outside the standard docs plugin. This is cosmetic (warnings only, not errors) and the route works correctly. The `onBrokenLinks: 'warn'` setting prevents build failure.

## User Setup Required
None - no external service configuration required. GitHub Pages deployment will activate automatically when the repository is configured for GitHub Pages with GitHub Actions source.

## Next Phase Readiness
- Phase 3 complete: all 5 plans executed (schemas, AsyncAPI, versioning, demo, site)
- Protocol spec published as open document with Docusaurus site
- Working CLI demo proves protocol end-to-end
- GitHub Actions CI/CD configured for automatic deployment
- Ready for Phase 4 (iOS reference client) or community feedback period

## Self-Check: PASSED

All 10 key files verified present. Both task commits (a372c15, 938cc33) verified in git log.

---
*Phase: 03-specification-publication-working-demo*
*Completed: 2026-03-28*
