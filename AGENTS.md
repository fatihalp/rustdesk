# SupportDesk Developer & Agent Guide

This monorepo contains the SupportDesk self-hosted remote support platform, comprising a Laravel 13 backend, a React customer portal, a high-performance single-binary Rust relay server, and a branded desktop client.

All automated agents and contributors must strictly adhere to the engineering standards and architecture described below.

---

## 1. Core Engineering Principles

### MVC (Model-View-Controller)
- **Laravel Backend**:
  - **Models** (`app/Models`): Represent database entities, relationships, casts, and data access. Keep models focused on state.
  - **Controllers** (`app/Http/Controllers`): Thin orchestration layer handling request validation, delegating business logic to domain services, and returning HTTP/JSON responses.
  - **Views** (`resources/views`, `resources/portal`): Blade templates for public views and React components for the single-page partner portal.
  - **Services** (`app/Services`): Dedicated business logic encapsulated into focused domain services.
- **RelayServer (Rust)**:
  - **Models** (`RelayServer/src/models`): State, repositories, cryptographic keys, rate limiting, and peer stores.
  - **Controllers** (`RelayServer/src/controllers`): Pure synchronous business logic taking decoded protocol or HTTP requests and returning replies/actions.
  - **Views** (`RelayServer/src/views`): Protocol message builders and response serialization.
  - **Server & Net** (`RelayServer/src/server.rs`, `RelayServer/src/net`): Sockets, framing codec, and asynchronous Tokio I/O.

### SOLID Principles
- **Single Responsibility (SRP)**: Each class, module, or function must have one reason to change. Business logic resides in dedicated services (e.g. `DeviceSyncService`, `RelayBridgeService`, `ClientPackagingService`), not controllers or views.
- **Open/Closed (OCP)**: Code is open for extension via interfaces, events, and provider bindings, but closed for modification.
- **Liskov Substitution (LSP)**: Derived implementations must remain compatible with base contracts and interfaces.
- **Interface Segregation (ISP)**: Keep interfaces and service contracts lean, targeted, and cohesion-driven.
- **Dependency Inversion (DIP)**: Depend upon abstractions; services are bound and resolved via `AppServiceProvider` using dependency injection.

### KISS (Keep It Simple, Stupid)
- Favor simple, direct, readable implementations over premature abstractions, complex inheritance hierarchies, or unnecessary indirection.

### DRY (Don't Repeat Yourself)
- Centralize shared logic into reusable services, helpers, or traits. Avoid duplication across controllers, Filament resources, and portal endpoints.

### YAGNI (You Aren't Gonna Need It)
- Implement only what is immediately required. Do not introduce speculative features, unused configuration parameters, or unused abstractions.

### Self-Documenting Code (Zero Comments Policy)
- Code must express intent clearly through expressive, descriptive naming of types, methods, parameters, and variables.
- Maintain a strict zero-comment policy across the codebase: no inline comments, no block comments, no commented-out code, and no redundant docblocks.

### Zero Tests Policy
- The project does not maintain test suites (`tests/`, `phpunit.xml`, and client test suites are removed). Do not introduce unit, feature, or integration test files.

### Strictly Zero Dead Code
- Unused methods, orphaned routes, dead imports, obsolete assets, and unused configuration entries must be eliminated immediately.

### English-Only Localization
- The codebase, user interfaces, logs, API messages, and assets are strictly English (`en`).
- Non-English localization files and multi-language translation dictionaries are removed from the project.

---

## 2. Monorepo Architecture

```
.
├── app/
│   ├── Filament/         # Filament 5 Admin Panel resources, tables, and forms
│   ├── Http/
│   │   ├── Controllers/  # Thin HTTP controllers (Api, Portal, Download)
│   │   ├── Middleware/   # Tenant and admin authorization guards
│   │   └── Resources/    # API resource transformers
│   ├── Models/           # Eloquent models (Customer, ClientApp, Device, User, etc.)
│   ├── Providers/        # Service providers and dependency injection container bindings
│   └── Services/         # Domain services (DeviceSyncService, RelayBridgeService, etc.)
├── client/               # Branded remote desktop client (Rust + Flutter)
│   ├── build.py          # Client build runner
│   ├── brand.json        # Dynamic brand configuration template
│   └── src/              # Client Rust engine, hooks, and English localization (src/lang/en.rs)
├── packaging/            # Packaging and code-signing scripts for macOS and Windows
├── RelayServer/          # Single-binary Rust rendezvous, relay, and control server
│   ├── src/
│   │   ├── controllers/  # Control, relay, and rendezvous controllers
│   │   ├── models/       # Peer, peer store, keys, repository, ip guard
│   │   ├── net/          # Socket wrappers, connection handlers, address encoding
│   │   ├── views/        # Protocol message serialization
│   │   └── server.rs     # Async network router and event loop
├── resources/
│   ├── portal/           # React SPA portal (Tailwind CSS, Apple-inspired monochrome theme)
│   └── views/            # Blade templates (portal container, download landing page)
└── routes/
    ├── api.php           # Device synchronization, runner, and portal API endpoints
    └── web.php           # Public short links (/s/{slug}) and download delivery
```

---

## 3. Component Details & Contracts

### Laravel Backend & Filament Panel
- **Database**: PostgreSQL (production), SQLite compatible for local utilities.
- **Admin Panel** (`/admin`): Filament 5 resources for managing customers, client applications, devices, download links, and builds. Requires `role = 'admin'`.
- **Portal API** (`/api/portal/*`): Multi-tenant REST endpoints for authenticated customer portal users. Guarded by `EnsureTenant` middleware.
- **Device Sync** (`/api/heartbeat`, `/api/sysinfo`, `/api/sysinfo_ver`):
  - Ingests heartbeats and system information from desktop clients.
  - Business logic is strictly contained in `DeviceSyncService`.
  - Must remain wire-compatible with `client/src/hbbs_http/sync.rs`.
- **Real-Time Relay Bridge** (`App\Services\RelayBridgeService`):
  - Communicates directly with `RelayServer` control port `21120`.
  - Queries live peer presence (`GET /status`), disconnects active sessions, and blocks/unblocks IDs (`POST /peers/{id}/{disconnect,block,unblock}`).

### React Portal (`resources/portal/`)
- Single-page application served at `/portal`.
- Uses React, Tailwind CSS, and Lucide icons with an Apple-inspired dark monochrome aesthetic.
- Connects directly to `RelayServer` live event WebSocket (`ws://<relay-host>:21120/ws`) for zero-polling real-time device status and active session indicators.

### RelayServer (`RelayServer/`)
- Single-process Rust service replacing upstream `hbbs` and `hbbr`.
- **Ports & Roles**:
  - `21115/tcp`: NAT test and online status queries.
  - `21116/udp`: Rendezvous peer registration and keep-alives.
  - `21116/tcp`, `21118/tcp` (WS): Rendezvous connection brokering and NAT punch coordination.
  - `21117/tcp`, `21119/tcp` (WS): Data relay pairing and forwarding.
  - `21120/tcp` (HTTP + WS): Control plane for Laravel and Portal (`GET /status`, `POST /peers/{id}/*`, `WS /ws`).

### Desktop Client (`client/`)
- Upstream RustDesk-derived desktop client.
- Branded via `brand.json` generated by `ClientPackagingService` containing rendezvous server, relay server, API URL, public key, and app name.
- Localization is strictly English (`client/src/lang/en.rs` and `client/src/lang.rs`).

---

## 4. Development & Contribution Rules

1. **Keep Controllers Thin**: Any non-trivial business logic, query composition, or external coordination belongs in a service under `app/Services/`.
2. **Preserve Wire Formats**: Do not change the JSON format of `ClientApp::manifest()` or `/api/heartbeat` / `/api/sysinfo` without updating both Laravel and the client simultaneously.
3. **Protect Sensitive Credentials**: `unattended_password` and `enroll_token` must never be exposed to unauthorized callers or non-admin API responses.
4. **No Comments**: Write self-documenting code. Never add explanatory comments, inline remarks, or commented-out blocks.
5. **No Tests**: Do not create or run test suites.
6. **No Non-English Strings**: All text must be in English.
