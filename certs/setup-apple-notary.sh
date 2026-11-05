#!/usr/bin/env bash
set -euo pipefail

# Helper script to configure Apple Developer ID & Notary Service credentials for Visiosoft

echo "=========================================================="
echo "  Apple Developer & Notary Service Yapılandırması"
echo "  VİSİOSOFT TEKNOLOJİ ANONİM ŞİRKETİ"
echo "=========================================================="

APPLE_ID="${2:-fatihalp89@gmail.com}"
TEAM_ID="${3:-RKAK7835AK}"
PASSWORD="${1:-}"

if [[ -z "$PASSWORD" ]]; then
  echo ""
  echo "KULLANIM:"
  echo "  $0 <uygulamaya-ozel-parola> [apple-id] [team-id]"
  echo ""
  echo "GEREKSİNİMLER:"
  echo "1. Developer ID Application Sertifikası:"
  echo "   - Xcode'u açın > Settings (Ayarlar) > Accounts (Hesaplar)"
  echo "   - Apple ID'nizi seçin > 'Manage Certificates...' butonuna tıklayın"
  echo "   - Sol alttaki '+' butonuna basıp 'Developer ID Application' seçin."
  echo ""
  echo "2. Apple Uygulamaya Özel Parola (App-Specific Password):"
  echo "   - https://appleid.apple.com adresine gidin"
  echo "   - 'Giriş Yapma ve Güvenlik' > 'Uygulamaya Özel Parolalar' bölümünden 'Visiosoft' için parola üretin"
  echo "   - Örnek: abcd-efgh-ijkl-mnop"
  echo ""
  echo "Örnek Komut:"
  echo "  $0 abcd-efgh-ijkl-mnop fatihalp89@gmail.com RKAK7835AK"
  echo "=========================================================="
  exit 1
fi

echo "==> Apple Noter kimliği kaydediliyor (profile: visiosoft-notary)..."
xcrun notarytool store-credentials "visiosoft-notary" \
  --apple-id "$APPLE_ID" \
  --team-id "$TEAM_ID" \
  --password "$PASSWORD"

echo "==> Başarılı! 'visiosoft-notary' profili anahtarlığınıza kaydedildi."
echo "==> Artık her derlemede DMG ve APP otomatik olarak Apple Noter Servisi'ne onaylatılacak (Notarized & Stapled)."
