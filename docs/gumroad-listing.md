# Gumroad Listing — Klipo by bluedev

> Gumroad ürün düzenleme editörüne **kopyala-yapıştır**: bu sayfadaki her bölüm,
> Gumroad UI'sının ilgili alanına paragraf-paragraf girer. Markdown stili
> ipuçlarını text editor'üne sade metin olarak yapıştır; başlık ve emoji
> stillerini Gumroad'un toolbar'ı ile uygula.
>
> Kaynak: WA Contacts Exporter Pro listing pattern'i. Aynı bluedev hesabı,
> aynı $29 lifetime model.

---

## Title (60 char max recommended)

```
Klipo by bluedev — Searchable Clipboard Manager for Windows
```

*(58 karakter — Gumroad max'ın altında, Google search snippet'inde tam görünür.)*

## Subtitle / one-liner

```
Lifetime license: every Ctrl+C is captured locally, searchable in milliseconds, and sensitive content (API keys, JWTs, credit cards) auto-flagged before paste.
```

## Cover image

- **File:** `marketing/gumroad/out/01-cover.png` (1280×720, üretilir)
- Recommended dimensions: 1280×720 (Gumroad cover ratio 16:9)

## Thumbnail

- **File:** `marketing/gumroad/out/02-thumbnail.png` (600×600 square)
- Recommended: 600×600 (Gumroad search result kartı)

## Gallery (5 screenshot — sıralı yükle)

1. `marketing/gumroad/out/03-hero.png` — 3-up superpower showcase (Capture / Search / Protect)
2. `marketing/gumroad/out/04-comparison.png` — Trial vs $29 Pro karşılaştırma kartı
3. `assets/screenshots/popup.png` — Klipo popup with captured clips
4. `assets/screenshots/popup-sensitive.png` — sensitive content guard (red border + blur)
5. `assets/screenshots/settings.png` — Settings General tab

## Demo video

- **File:** `marketing/gumroad/out/klipo-demo.mp4` (~78.5 saniye, 1280×720, 30fps, H.264)
- Gumroad'un "Featured video" alanına yükle (description'ın hemen üstünde gösterilir)

---

## Price

- **$29 USD** — one-time payment, lifetime license
- Currency: USD (Gumroad otomatik global currency conversion)
- "Pay what you want" toggle: **OFF**
- Min/max: tek fiyat, indirimsiz launch (sonra discount code ile %20 launch promo eklenebilir)

## License key generation ⚠️ KRİTİK

- ✅ **Generate license keys: ON** (Gumroad → Edit product → Sales settings)
- Bu olmadan Klipo'nun in-app activation flow'u çalışmaz; trial 14 gün sonunda tüm kullanıcılar "Klipo Pro — Activated" durumuna geçemez

## Refund policy

- **30 days, no questions asked** (Gumroad default)
- LEGAL/REFUND.md ile uyumlu

---

## Product description (paste into Gumroad description editor)

> Düz paragraflar. Gumroad'un rich-text editor'ü Markdown tablo veya `###`
> başlık render etmiyor; emoji bullets ve bold (B) toolbar düğmesini
> elle uygula. Aşağıdaki versiyon tone-yumuşatılmış, "satış metni" gibi
> okunmuyor — direkt yapıştırabilirsin.

```
Hayatın boyunca kaç kez bir şey kopyaladın, üç saniye sonra başka bir şeyi kopyaladığında o önceki uçtu? Bir Stripe API key'i, müşterinin adresi, satırı, makalede gördüğün bir paragraf — hepsi tek bir Ctrl+C ile geri kazanılamaz. Bu pano (clipboard) probleminin kendisi.

Klipo'yu kendim için yazdım çünkü bu sorun beni günde 20 dakika yiyordu. Windows'un sistem panosuna bir tane şey sığar; ben yüzlerce şey kopyalıyorum ve hepsini hatırlamak istiyorum. Şimdi sen de aynı şeyi 14 gün boyunca ücretsiz dene, beğenirsen $29'a lifetime al.

Klipo Windows tray'inde sessiz çalışır. Her Ctrl+C — text, kod, görsel, dosya, RTF, HTML — yerel bir SQLite veritabanında saklanır. Hiçbir bulut sunucusuna gitmez. Ctrl+Alt+V'ye bas, popup açılır, üç-beş harf yaz, FTS5 BM25 sıralamasıyla aradığın clip 50 milisaniye altında bulunur, Enter'a bas, paste edilir.

Klipo bir başka şey daha yapıyor: kopyaladığın bir API key veya kredi kartı numarasıysa fark eder. 13 örüntü out-of-the-box (OpenAI sk-proj/sk-svcacct/sk-admin, AWS, GitHub token, Anthropic, Stripe live/test, JWT, kredi kartları, SSH/PEM private key, password labels, URL token query strings). Bu clip'ler popup'ta kırmızı border + bulanık preview ile gözükür, paste'lemeden önce onay diyaloğu çıkar. Yanlış kanal'a yapıştırma kazasını kapatıyor.

14 günlük tam-özellik trial sonrasında license key girip Pro'ya geçersin. Tek fark: trial sayacı kalkar, capture limitsiz devam eder. Aynı binary, sadece anahtar Pro'yu açar. License doğrulama Klipo'dan doğrudan api.gumroad.com'a gider — hiçbir bluedev sunucusuna proxy yok, sadece anahtarın transmission edilir, hiç clipboard içeriği yok. 30 günlük offline grace; internet kopuk olsa bile Pro çalışır.

$29 öder, lifetime kullanırsın. Tüm v0.x güncellemeleri dahil; macOS portu (v0.2'de geliyor) free upgrade. Aynı key 3 kişisel cihaza kadar aktive olur. Refund 30 gün, sorgusuz.


Birkaç sıkça gelen soru.

Klipo nasıl indirilir? bluedev.dev/products/klipo veya GitHub Releases sayfasından NSIS installer ile (~3.8 MB). Aynı binary trial olarak başlar, license key girince Pro'ya geçer. Gumroad'dan ekstra bir şey indirmen gerekmiyor.

Anahtarımı kaybettim. Gumroad library sayfasından yeniden gönder, veya satın alırken kullandığın email ile support@bluedev.dev'e yaz.

Bluedev kapanırsa Klipo çalışmaya devam eder mi? License doğrulama doğrudan Gumroad'a gidiyor, bluedev altyapısına bağımlı değil. Ben durdursam bile mevcut alıcılar 30 günlük grace + tekrar bağlanabilir Gumroad endpoint'i ile çalışmaya devam ederler.

Bluedev clipboard içeriğimi görüyor mu? Hayır. Tüm capture, dedup, search, sensitive-detection işlemi senin makinendeki SQLite veritabanında. Tek network call'um Gumroad'a license verify; o request sadece anahtarını içerir, hiçbir clipboard verisi yok.

Mac kullanıyorum, alabilir miyim? Şu an sadece Windows. macOS portu v0.2'de planlandı (yaklaşık 2-3 ay içinde). Şimdi alırsan v0.2 free upgrade — ama v0.1.x kullanmak için Windows gerek.


Klipo'nun kaynak kodu açık değil (v0.1.3'ten itibaren proprietary EULA). Ama mimarinin nasıl çalıştığı, hangi pattern'lerin sensitive olarak işlendiği, performans bütçesi — hepsi bluedev'in public docs'undan görülebilir. Şeffaflık için dökümantasyon paylaşılır, source code ticari ürün olarak korunur.

Destek için support@bluedev.dev — Pro kullanıcılarına 1 iş günü SLA. Bug raporu için Klipo Settings → About'tan "Bug report copy" tıkla; OS sürümü + Klipo sürümü + son 100 satır log'u clipboard'a kopyalar. Onu mail'e yapıştırınca çözüm hızı ikiye katlanır.

GDPR / KVKK / CCPA: Klipo senin makinende, senin clipboard'unu işliyor. Hangi veriyi kopyalamana izin verdiğin, neyle ne yapacağın bizim değil senin sorumluluğun.

Site: bluedev.dev/products/klipo
```

---

## Content delivered to buyer (Gumroad "Content" upload)

`marketing/gumroad/buyer-readme.txt` dosyasını Gumroad upload alanına yükle.
Gumroad satıştan sonra bu dosyayı + benzersiz license key'i otomatik email'le
gönderir.

> **Klipo binary'sini Gumroad'a YÜKLEME.** Binary GitHub Releases'tan
> auto-update ile gelir; Gumroad sadece license key delivery + activation
> talimatları için kullanılır. Bu, her release'de manuel re-upload
> ihtiyacını ortadan kaldırır.

---

## Gumroad settings checklist

- [ ] Type: **Digital product → Software**
- [ ] Cover image yüklendi (1280×720)
- [ ] Thumbnail yüklendi (600×600 square)
- [ ] Gallery: 5 screenshot yüklendi
- [ ] Featured video: `klipo-demo.mp4` yüklendi
- [ ] Price: **$29 USD**
- [ ] Currency: USD
- [ ] **Generate license keys: ON** ⚠️
- [ ] Refund policy: 30 days, no questions asked
- [ ] Tags: `clipboard-manager`, `productivity`, `windows`, `developer-tools`, `tauri`, `rust`, `bluedev`, `clipboard`, `local-first`, `privacy`
- [ ] "Pay what you want": OFF
- [ ] Status: **Unlisted** (description boyunca; tamamlanınca "Public")

---

## SEO meta

Gumroad ürün sayfası SEO için:

- **Page title** (Gumroad UI): `Klipo by bluedev — Searchable Clipboard Manager for Windows`
- **Meta description** (Gumroad SEO ayarı):
  ```
  Klipo captures every Ctrl+C locally and lets you find it again in milliseconds. 13 sensitive-content patterns auto-flagged. Built on Tauri 2 + Rust. $29 lifetime, 14-day free trial. By bluedev.
  ```
- **Open Graph image**: aynı thumbnail (600×600) veya cover image (1280×720)

## Tags (Gumroad search için)

`clipboard-manager`, `productivity`, `windows`, `windows-app`, `developer-tools`, `tauri`, `rust`, `bluedev`, `clipboard`, `local-first`, `privacy`, `keyboard-first`, `ctrl-c`, `clipboard-history`

---

## Launch checklist (sayfa hazırlandıktan sonra)

- [ ] Klipo `KLIPO_PRODUCT_ID_DEFAULT` source'ta gerçek product_id ile güncellendi (v0.1.5 retag yapıldı)
- [ ] CI v0.1.5 release çıkardı, draft GitHub Release publish'lendi
- [ ] bluedev.dev/products/klipo landing page deploy edildi (Buy CTA Gumroad URL'ine işaret ediyor)
- [ ] Gumroad ürünü "Unlisted"'tan "Public"'e geçirildi
- [ ] Test satın alım yapıldı (kendi kartınla, sonra refund), license key delivery + activation flow doğrulandı
- [ ] Twitter/X, Reddit r/Windows10, ProductHunt, Show HN posts (`docs/launch-draft.md` taslakları) gönderildi

---

## Hızlı edit ipuçları

- **Fiyatı değiştirmek istersen** ($19 / $39): bu dosyada tüm `$29` arama-değiştirilir, plus `marketing/gumroad/{cover,thumbnail,comparison}.html` ve `scenes/07-pricing.html`, `scenes/08-cta.html` içinde aynı arama-değiştir, sonra `node marketing/gumroad/capture.mjs` ile asset'leri yeniden üret.
- **Domain değiştirmek istersen**: `bluedev.dev/products/klipo` → yeni URL. Aynı arama-değiştir + capture re-run.
- **Tone değiştirmek istersen** (örn. daha resmi / daha samimi): yukarıdaki product description'ı düzelt, Gumroad UI'sındaki description editor'ünden direkt yapıştır.
