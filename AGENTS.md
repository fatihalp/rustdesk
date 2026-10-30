# Monorepo guide

- `client/` — RustDesk client. Follow `client/AGENTS.md` for everything inside it (minimal diffs, thin hooks, i18n rules, tests).
- Repo root — Laravel + Filament support panel (PostgreSQL). Models in `app/Models`, Filament resources in `app/Filament`, device/build API in `routes/api.php`, public download links in `routes/web.php`.
- `relay/` — hbbs/hbbr docker compose. `packaging/` + `.github/workflows/brand-package.yml` — branded, signed client packaging.

Rules for the panel:

- The client protocol (`/api/heartbeat`, `/api/sysinfo`, `/api/sysinfo_ver`) is defined by `client/src/hbbs_http/sync.rs`; keep `DeviceSyncController` compatible with it.
- Brand config format is `ClientApp::manifest()` ↔ `client/src/brand.rs`; change both together.
- Never expose `unattended_password` or `enroll_token` outside the admin panel and the build-runner API.
- Run `php artisan test` before committing. Tests use SQLite in memory.
