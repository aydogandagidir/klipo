# AGENT.md — Development Context for Klipo

> Bu dosya, kod tabanında çalışan herhangi bir geliştirme aracına / asistana / katkı sağlayan kişiye bağlam verir. Buradaki kararlar refleksle uygulanmalı; gerekçesiz değiştirilmemelidir.

## Proje Tek Cümleyle

Klipo, **cross-platform (Windows v0.1, macOS v0.2)** bir clipboard manager'dır. Hız + gizlilik + opt-in E2E sync + AI transform katmanlarıyla tasarlandı. Tarihsel olarak `ClipFlow` adıyla başladı, **Klipo'ya rebrand edildi**.

## Bağlayıcı Mimari Kararlar

| Karar | Değer | Gerekçe |
|---|---|---|
| Framework | Tauri 2.0 | Electron şişman, native çift kod tabanı sürdürülemez |
| Frontend | React 18 + TypeScript 5 + Vite | Aydoğan'ın deneyimi, ekosistem |
| Styling | Tailwind 3 + shadcn/ui (copy-in) | Tasarım sistemi, bundle kontrolü |
| State | Zustand | Redux gereksiz, Zustand 1KB |
| DB | SQLite + sqlx + FTS5 | Yerel-öncelikli, full-text search |
| Backend lang | Rust 1.83+ | Tauri'nin native dili, perf |
| Hotkey | `tauri-plugin-global-shortcut` | Resmi plugin |
| Crypto | `sodiumoxide` (libsodium) | OpenSSL DEĞİL — libsodium daha az foot-gun |
| Lisans | Apache-2.0 | Patent grant, business-friendly |
| Hotkey default (Windows) | `Ctrl+Alt+V` | `Ctrl+Shift+V` paste-without-formatting çakışması |
| Pkg manager | pnpm | npm DEĞİL |

## Faz Düzeni

- **Faz A — Mimari & spec'ler:** `docs/` doldur, benchmark prototip yaz. ✅ tamamlandı.
- **Faz B — Windows v0.1 MVP build:** M1 Skeleton → M7 Release. M1-M5.x.1 tamamlandı.
- **Faz C — macOS v0.2:** Mac mini erişimi gelince. NSPasteboard watcher, vibrancy, codesign+notarize.
- **Faz D — v0.3 Sync + AI:** E2E sync server + client, AI transform actions.

## Non-Negotiables (Asla Değiştirme)

1. **Kullanıcı clipboard içeriği logda asla görünmez.** Sadece SHA-256 hash + uzunluk loglanır.
2. **Telemetri default OFF.** Açıkça opt-in olmadan tek byte network gönderilmez.
3. **`unsafe` Rust** sadece OS API wrap için, her zaman `// SAFETY:` yorumlu.
4. **TypeScript'te `any` yasak.** `unknown` kullan, narrow et.
5. **AI sonucu otomatik paste edilmez.** Kullanıcı her zaman approve eder.
6. **Sensitive item tespit edilince RAM'de 30s sonra zeroize.** `mlock` ile swap'a yazılmaz.
7. **Conventional Commits** (feat/fix/chore/docs).
8. **Her PR yeşil olmalı:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `pnpm typecheck`, `pnpm lint`, `pnpm test`.

## Kod Tabanı Yapısı

```
klipo/
├── src/                # React frontend (PopupPanel, Settings, ClipCard, ipc.ts)
├── src-tauri/src/      # Rust backend
│   ├── clipboard/      # watcher_windows.rs, normalize.rs, sensitive.rs, paste.rs, pipeline.rs, source_app.rs
│   ├── storage/        # clips.rs, search.rs, blob.rs, migrations/
│   ├── commands.rs
│   └── lib.rs
├── docs/               # Mimari spec'leri
├── bench/              # criterion crate
└── .github/workflows/  # ci.yml, release-windows.yml
```

## Performans Bütçesi (Bağlayıcı)

- Cold start → popup visible: <300ms p95
- Search 1k clips: <50ms p95
- Insert clip: <20ms p95
- RAM idle: <100MB (WebView2 baseline'ı dahil)
- RAM 10k clips: <250MB

Bu sayılar Faz A benchmark sonuçlarıyla doğrulanır; tutmuyorsa mimari revize edilir.

## Bilinen PRD-vs-Gerçek Çatışmaları

PRD detaylı ama 6 noktada gerçek-dünyaya çarpıyor — çözümleri uygulandı:

1. `Ctrl+Shift+V` Windows'ta meşgul → `Ctrl+Alt+V` ✅
2. `tauri-plugin-app-icon` mevcut değil → native shim (windows-rs) ✅
3. `enigo` paste flaky → Windows `SendInput`, macOS `CGEventPost` direkt ✅
4. Mica Win 11 only → Win 10 acrylic fallback, ARM64 solid ✅
5. NSPasteboard 250ms drain → 500ms default, locked iken 2000ms (Faz C)
6. FTS5 Türkçe `ı/i/I/İ` → migration 002_turkish_fts.sql ile rebuild ✅

## Çalışma Kuralı

- **Plan → Apply → Verify** protokolü. Non-trivial değişiklik öncesi 3-7 satır plan, sonra uygula, sonra `cargo test` + lint.
- Mimaride dallandırıcı bir karar varsa **dur ve sor.**
- "Boring beats clever" — popüler/iyi-belgelenmiş kütüphane > akıllı çözüm.
- Spec varsa ona uy. Spec yoksa, sor veya `docs/` altına ek bir spec yazıp PR'la bağla.
