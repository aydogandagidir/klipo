# Klipo — Gumroad'da Ürün Oluşturma Kılavuzu

> Adım-adım rehber: bluedev Gumroad hesabıyla Klipo'yu satılabilir bir
> ürün olarak ayağa kaldırma. Tahmini süre: **20-30 dakika**.
>
> Bu rehber WA Contacts Exporter Pro listing pattern'ine göre yazılmıştır
> (sibling project, aynı bluedev hesabı, aynı $29 lifetime model).
> Klipo'ya özel farklar açıklandı.

---

## Önkoşul checklist

| # | Madde | Hazır mı? |
|---|---|---|
| 1 | bluedev Gumroad hesabı (giriş erişimin var) | ✅ varsayılan (WA exporter zaten orada) |
| 2 | Klipo product_id şu an sahte (`KLIPO_PRODUCT_ID_DEFAULT = "TODO_REPLACE_AFTER_GUMROAD_LISTING"`) | ⏳ bu kılavuzun çıktısı |
| 3 | Cover image / thumbnail / gallery PNG'leri | ⏳ Adım 4'te ben mockup üreteceğim |
| 4 | Demo video MP4 | ⏳ ileride (script `docs/demo-video-script.md` hazır) |
| 5 | Buyer email (license key delivery sonrası gönderilen) | ⏳ Klipo için yazılacak (`marketing/gumroad/buyer-readme.txt`, WA pattern'inden uyarlanacak) |
| 6 | Klipo açıklama copy'si | ✅ `docs/gumroad-product-page.md` zaten var, WA formatına revize edilecek |

> **Şu an sadece ürünü "Unlisted" olarak yarat** — Description, cover, gallery
> yokken "Save as draft / Unlisted". Ben paralel olarak asset'leri
> üreteceğim, Adım 9'da dönüp finalize edersin.

---

## Adım 1 — Gumroad'a giriş + Yeni ürün

1. https://gumroad.com/dashboard adresine git, **bluedev** hesabıyla giriş yap.
2. Sol üstte **`+ New product`** (veya **Products → Create**) butonuna bas.
3. Ürün tipi seçim ekranı açılır → **`Digital product`** seç.
4. "What are you selling?" (veya "Product type") → **`Software`** (downloadable file).

> WA Contacts Exporter de aynı tip altında listelenmiş — Gumroad'un
> "Software" kategorisi altında license key delivery toggle'ı çıkıyor,
> tip seçimi önemli.

---

## Adım 2 — Temel bilgiler

| Alan | Değer |
|---|---|
| **Name** | `Klipo by bluedev — Searchable Clipboard Manager` *(60 char max recommended; bu 53 char)* |
| **Permalink** | `klipo` *(varsayılan)* — sonuç URL'i: `https://bluedev.gumroad.com/l/klipo` |
| **Subtitle / one-liner** | `Lifetime license: Every Ctrl+C, captured locally and searchable in milliseconds. Sensitive content auto-detected.` |
| **Price** | `$29 USD` — one-time payment, lifetime license |
| **"Pay what you want"** | **OFF** |
| **Currency** | `USD` (Gumroad auto-converts for global buyers) |
| **Type** | Digital → Software |

> Permalink değiştirmek istersen 5 karakter ve üzeri lowercase, birkaç tire
> ile. Marka bütünlüğü için `klipo` direkt en sade.

---

## Adım 3 — Description (geçici)

Şu an description'ı **boş bırak veya tek satır TODO** koy. Gerçek
description'ı Adım 9'da `docs/gumroad-product-page.md`'in WA formatına
revize edilmiş halinden alacağız.

Geçici:

```
Coming soon — full listing copy + screenshots + demo video.
```

Bu yeterli — ürün **Unlisted** olduğu sürece kimse görmüyor.

---

## Adım 4 — License key generation **kritik**

Bu adımı atlama:

1. Ürün düzenleme sayfası → sol sidebar'da **`Settings`** veya **`Sales settings`** sekmesi.
2. **`Generate license keys`** toggle'ını **ON** yap.
3. *Buy in same key reuse* veya *Allow multiple uses* gibi alt seçenekler varsa, **OFF** bırak (her satışta benzersiz key, multi-device kontrolü Klipo backend'i Gumroad uses counter ile yapacak).

> WA Contacts Exporter'ın `gumroad-api.js`'i `increment_uses_count=true`
> ile ilk aktivasyonda Gumroad'da uses sayacını artırır. Bu sayaç
> aynı zamanda 3-cihaz limitinin enforce edicisidir. **License key
> generation OFF ise activation çağrıları başarısız olur** — Klipo
> kullanıcıyı "trial only" tutmaya devam eder.

---

## Adım 5 — Refund policy

- Sol sidebar → **`Refunds`** veya ürün ayarlarındaki refund alanı.
- Set: **No refunds. The 14-day free trial gives buyers full access to evaluate before paying.**
- Statutory consumer rights (EU 14-day right of withdrawal etc.) still apply where required by law.
- Klipo EULA `LEGAL/REFUND.md` ile uyumlu.

---

## Adım 6 — Content (geçici)

Gumroad "Content" alanı = alıcının indirdiği file. Klipo için iki seçenek:

**Seçenek A (önerilen — auto-update zinciri ile uyumlu):**
- Gumroad upload alanına `marketing/gumroad/buyer-readme.txt` dosyasını yükle.
- Bu dosya:
  - Klipo'nun **bluedev.dev/products/klipo'dan ücretsiz indirileceğini** açıklar
  - License key + email ile activation talimatlarını verir
  - Lokal kullanım, privacy, support bilgisi
- Klipo binary'sini **upload etme** — auto-update zinciri ve installer her zaman GitHub Releases'tan gelir, Gumroad'da binary tutmaya gerek yok (versiyon güncel kalmalı, Gumroad'a manuel upload zorlaşır).

**Seçenek B (alternatif):**
- Klipo binary'yi Gumroad'a yükle, kullanıcı buradan indirsin
- Dezavantaj: her release'de manuel re-upload gerek; auto-update zinciri yine GitHub'a bağlı, gereksiz çift kanal

> Bu kılavuz Seçenek A'yı varsayar. `marketing/gumroad/buyer-readme.txt`
> şu an YOK — Adım 9'da yazacağım, sonra yüklersin.

Şu an Content alanına geçici olarak **tek satır boş text dosyası** yükleyebilirsin (`echo "TBD" > tbd.txt`) ya da Gumroad sana izin veriyorsa boş bırak.

---

## Adım 7 — Cover / Thumbnail / Gallery (geçici)

| Alan | Önerilen | Şu an |
|---|---|---|
| Cover image | 1280×720 PNG | ⏳ TODO (Adım 9 — bluedev brand renkleriyle mockup) |
| Thumbnail | 600×600 PNG | ⏳ TODO |
| Gallery (5 screenshot) | 1280×720 her biri | ⏳ TODO (Klipo popup, Settings, License tab, sk-proj fix red border, Re-scan history toast) |
| Demo video | MP4 H.264 1080p ≤ 50 MB | ⏳ TODO (script + shotlist hazır: `docs/demo-video-script.md`) |

Şu an Gumroad'da ya **placeholder PNG** yükle (1280×720 düz mavi `#015AFF` arkaplan + "Klipo" text), ya da bu alanları boş bırak. **Unlisted** olduğu için sorun değil.

---

## Adım 8 — Save unlisted + product_id'yi al ⭐

1. Sayfanın altında / üstünde **`Save`** veya **`Publish as unlisted`** butonu.
2. **"Save as draft"** veya **"Save as unlisted"** SEÇİMİYLE kaydet — public publish'leme!
3. Ürün başarıyla yaratıldıktan sonra **product_id'yi bul:**

   **Yöntem A (URL'den):**
   - Ürün düzenleme sayfasının URL'i: `https://app.gumroad.com/products/<product_id>/edit`
   - URL'deki `<product_id>` 5-12 karakterlik kısa hash, örn. WA exporter'ınki **`axfxg`**.

   **Yöntem B (Settings sekmesinden):**
   - Sol sidebar → **`Settings`** veya **`Permalink`**
   - "Product ID" veya "ID" etiketli kutucukta görünür.

4. **Bu product_id'yi bana yaz** — örn. *"product_id: xy123"* — ben:
   - `src-tauri/src/license/mod.rs:KLIPO_PRODUCT_ID_DEFAULT` değerini güncelleyeceğim
   - v0.1.5 retag yapacağım
   - CI yeni release çıkaracak
   - Mevcut Klipo v0.1.4 kullanıcılar (sen) auto-update ile v0.1.5'e geçecek
   - License activation gerçek key ile çalışmaya başlayacak

---

## Adım 9 — (Ben yapacağım) Final asset paketi

Sen ürünü unlisted yarattıktan ve product_id verdikten sonra ben:

1. **`docs/gumroad-product-page.md`'i WA formatına revize et** — 11 sabit bölüm sırası, paragraflı kopyalama-yapıştır hazır.
2. **`marketing/gumroad/`** scaffold (Klipo için):
   - `cover.html` (1280×720, bluedev `#015AFF` accent + `#0A1628` navy + General Sans font)
   - `thumbnail.html` (600×600 square, app icon over gradient)
   - `hero.html` (1280×720, 3 superpower: Capture / Search / Protect)
   - `comparison.html` (1280×720, Free trial vs Pro pricing card)
   - `style.css` (bluedev brand system)
   - `capture.mjs` (Playwright + ffmpeg pipeline, WA'nınkinden port)
   - `scenes/01-08.html` (demo video sahne HTML'leri, ~90 sn pipeline)
3. **`marketing/gumroad/buyer-readme.txt`** — license key delivery email içeriği
4. **`docs/gumroad-listing.md`** — sen Gumroad description editor'ünde paste edebileceğin paragraflı tam metin
5. **Tags listesi:** `clipboard-manager`, `productivity`, `windows`, `developer-tools`, `tauri`, `rust`, `bluedev`

Sen sadece:
- Üretilen PNG'leri Gumroad'a yükle
- Description'ı paste'le
- Tags'leri yaz
- Demo video MP4'ü yükle (script verirken sen kaydedersin, Adım 10)
- "Publish" → ürün canlıya girer

---

## Adım 10 — Sonra: video kayıt + asset üretimi

Bu kılavuzun kapsamı dışında, ileri için:

- **Demo video kayıt:** OBS Studio + voice-over (sen kaydedersin)
  - Script: `docs/demo-video-script.md`
  - Shotlist: `docs/demo-video-shotlist.md`
  - Çıktı: `marketing/gumroad/out/klipo-demo.mp4`
- **Cover/thumbnail/hero PNG üretim:**
  - `cd marketing/gumroad && node capture.mjs` → `out/` klasörüne 4 PNG
  - Manuel iyileştirme istiyorsan Figma/Photoshop ile editle

---

## Adım 11 — Public publish (final)

Tüm asset'ler hazır + Description paste'lendi + product_id Klipo binary'sine
yansıdı + v0.1.5 release çıktı + auto-update zinciri test edildi:

1. Gumroad ürün sayfası → **`Publish`** (Unlisted'tan Public'e geçiş)
2. URL: `https://bluedev.gumroad.com/l/klipo` veya `https://bluedev.gumroad.com/klipo`
3. bluedev.dev'de Klipo landing page'i `/products/klipo` ile aktive et
4. Twitter/X, Reddit r/Windows10, ProductHunt, Show HN posts (`docs/launch-draft.md` taslakları)

---

## Bilmen gereken küçük şeyler

- **Vergi:** Gumroad sales tax otomatik halleder; sen ABD/AB vergi numarası girmek zorunda kalmıyorsun. Türkiye'de gelir vergisi beyanını **kendin** yaparsın (yıl sonu Gumroad'dan toplam satış raporu alıp muhasebenle paylaşırsın).
- **Payout:** Gumroad → Settings → Payout Settings'te bluedev'in banka/PayPal hesabı bağlı. Min payout eşiği genellikle $10.
- **Discount codes:** Lansmanda %20 launch discount koyabilirsin (Settings → Discount codes). 7-14 gün'lük süre sınırı önerilir.
- **Affiliates:** ileride bloglar/podcast'ler Klipo'yu öneriyorsa Gumroad otomatik affiliate revenue split'i destekler — şu an gerek yok.

---

## Hızlı özet — Sen şu 5 şeyi yapacaksın bu turn'de

1. https://gumroad.com/dashboard → New product → Software
2. Name: `Klipo by bluedev — Searchable Clipboard Manager`, Subtitle, Price `$29`, Permalink `klipo`
3. Settings → **Generate license keys: ON**
4. Save as **Unlisted** (Public DEĞİL)
5. URL'den product_id'yi al, bana yaz: *"product_id: xxxxx"*

Ben paralel olarak Adım 9'daki asset paketini sandbox'ta üretip ana repo'ya merge edeceğim. Sen product_id verdiğinde Klipo source güncellenip v0.1.5 release çıkar.
