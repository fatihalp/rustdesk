# Uzaktan destek paneli (monorepo)

| Klasör | İçerik |
| --- | --- |
| `/` (kök) | Laravel 13 + Filament 5 yönetim paneli (PostgreSQL) |
| `client/` | RustDesk istemcisi (Apple tarzı arayüz). Marka/sunucu ayarı `client/src/brand.rs` |
| `relay/` | hbbs + hbbr relay sunucusu (docker compose) |
| `packaging/`, `.github/workflows/brand-package.yml` | Müşteriye özel markalı, imzalı Windows/macOS paketi üretimi |

## Akış

1. Panelde **Müşteri** ve **İstemci** (isim + logo) oluşturulur.
2. İstemci satırında **Derle → Windows / macOS**: panel GitHub Actions `brand-package.yml` iş akışını tetikler. İş akışı hazır istemciye `brand.json` + logo + isim ekler, imzalar (macOS: Developer ID + notarization, Windows: Authenticode) ve panele yükler.
3. **İndirme linki** ile müşteriye `https://panel/d/<token>` gönderilir (süre / indirme limiti / iptal).
4. Müşteri indirip açar. İstemci relay sunucusuna bağlanır ve panele `/api/heartbeat` + `/api/sysinfo` gönderir; cihaz **Cihazlar** sayfasında çevrimiçi görünür.
5. Cihaz satırındaki **Bağlan** (`rustdesk://connection/new/<id>?password=…`) veya **Komut** (`rustdesk --connect <id> --password …`) ile şifre sorulmadan bağlanılır. Şifreyi istemci ilk açılışta kendisi üretir; panel veritabanında şifreli saklanır.

## Panel kurulumu

```bash
composer install
cp .env.example .env && php artisan key:generate
# .env: DB_* (PostgreSQL), PANEL_API_URL (https), PANEL_RENDEZVOUS_SERVER, PANEL_RELAY_SERVER,
#       PANEL_RELAY_KEY, PANEL_BUILD_SECRET, PANEL_GITHUB_*, PANEL_ADMIN_*
php artisan migrate --seed
php artisan storage:link
php artisan serve        # /admin
php artisan test
```

## İmzalama için GitHub secret'ları

- Ortak: `PANEL_BUILD_SECRET` (panel `.env` ile aynı)
- macOS: `MACOS_CERT_P12` (base64), `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`
- Windows: `WINDOWS_CERT_PFX` (base64), `WINDOWS_CERT_PASSWORD`
- İş akışı, `client-base` adlı release'teki `client-base-macos.zip` (.app) ve `client-base-windows.zip` dosyalarını taban istemci olarak kullanır.

## Geliştirmede istemciyi panele bağlamak

```bash
cd client
RUSTDESK_BRAND_FILE=/path/brand.json DYLD_LIBRARY_PATH="$PWD/target/debug" ./target/debug/rustdesk
```

`brand.json` içeriği panelin `GET /api/builds/{id}/manifest` çıktısındaki `manifest` alanıyla aynıdır.

## Bilinen sınırlar

- `client/` altındaki eski RustDesk workflow'ları kök yollarına göre yazılmıştı; `client/` ön ekine göre güncellenmeli.
- İmzalama sertifikaları olmadan paketleme iş akışı çalıştırılamaz; sertifikalar bu repoda yoktur.
- Şifresiz bağlantı bağlantısı (`rustdesk://`) yalnızca Flutter arayüzünde çalışır; Sciter sürümünde komut kullanılır.
- Kullanıcı istemcide kalıcı şifreyi kendisi değiştirirse panelde saklanan şifre eskir.
- `relay/` Docker ile çalışır; bu makinede Docker olmadığı için compose dosyası denenmedi.
