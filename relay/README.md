# Relay sunucusu

Müşteri istemcilerinin bağlandığı rendezvous (`hbbs`) ve relay (`hbbr`) sunucusu.

> `rustdesk/rustdesk-server-demo` yalnızca tek bağlantılı, şifrelemesiz bir örnektir.
> Üretim için resmi `rustdesk/rustdesk-server` imajı (hbbs + hbbr) kullanılır; bu klasör onu çalıştırır.

## Kurulum

```bash
cp .env.example .env          # RELAY_HOST değerini kendi alan adınla değiştir
docker compose up -d
cat data/id_ed25519.pub       # PANEL_RELAY_KEY değeri
```

Sunucu makinesinde şu portlar açık olmalı: `21115-21119/tcp`, `21116/udp`.

## Panelle bağlantı

Panelin `.env` dosyasına yaz:

```
PANEL_RENDEZVOUS_SERVER=relay.example.com
PANEL_RELAY_SERVER=relay.example.com
PANEL_RELAY_KEY=<data/id_ed25519.pub içeriği>
```

Bu değerler her müşteri istemcisinin `brand.json` dosyasına gömülür; istemci açılınca bu sunucuya
bağlanır ve panel `PANEL_API_URL` üzerinden cihazı görür.
