# Repository Guidelines

## Project Structure & Module Organization
- `index.ts`: Express entrypoint running on Bun; serves the homepage at `/`.
- `package.json`, `bun.lockb`: Dependency and runtime metadata.
- `tsconfig.json`: TypeScript settings for Bun with strict mode.
- `README.md`: Quickstart for install/run. No tests or assets directories yet; add new modules under `src/` if the project grows.

## Build, Test, and Development Commands
- `bun install`: Install dependencies.
- `bun run index.ts` or `bun start`: Start the server once.
- `bun --watch index.ts` or `bun run dev`: Run with auto-reload during development.
- No test suite is defined yet; add one before introducing features that need coverage.

## Coding Style & Naming Conventions
- Language: TypeScript targeting Bun; ES module syntax.
- Indentation: 2 spaces; keep lines concise and prefer early returns.
- Routes/controllers: name files by route intent (e.g., `src/routes/health.ts`).
- Environment: read config via `process.env`; default to safe fallbacks (e.g., `PORT` with numeric default).
- Keep inline HTML minimal and semantic; extract templates if they grow.

## Testing Guidelines
- Framework: not set up. Recommended: `vitest` or `bun:test`.
- Test layout: mirror `src/` (e.g., `tests/health.test.ts`).
- Naming: describe behavior (`should respond with 200 on /health`).
- Coverage: add thresholds when tests exist; run via `bun test`.

## Commit & Pull Request Guidelines
- Commit messages: follow conventional commits seen in history (e.g., `feat: initial bun express homepage`).
- Scope small, readable commits; avoid mixing refactors with features.
- Pull requests: include summary, manual test notes (commands and results), and linked issues; attach screenshots for UI changes.

## Security & Configuration Tips
- Do not commit secrets; prefer `.env` (already gitignored). Document required vars in README or PR.
- Validate and sanitize request input when adding new routes; return safe defaults on errors.
- Pin APIs to explicit versions in `package.json`; keep `bun.lockb` in sync.
