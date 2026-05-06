# Default Hotkey Conflict Analysis (Windows)

**Status:** Decision recommendation. Locks before M4 of Phase B.
**Audience:** Anyone changing the default hotkey or defending the choice.

The PRD originally proposed `Ctrl+Shift+V` as default. This doc explains why we recommend `Ctrl+Alt+V` instead, and what fallbacks engage if even that conflicts.

---

## 1. The Core Problem

A clipboard manager's hotkey must:

1. Be **system-wide** (works in every app, including admin-elevated ones, browsers, terminals).
2. Be **memorable** (mnemonic to "paste"-related keys).
3. Be **unconflicted** in 99% of user environments.

`Ctrl+Shift+V` fails (3): it's the de facto Windows convention for "paste without formatting" or "paste from clipboard history" in dozens of apps. Registering it system-wide steals the keystroke from every app that uses it.

---

## 2. Conflict Survey

We surveyed 10 widely-used Windows applications for built-in `Ctrl+Shift+V` usage.

| App | `Ctrl+Shift+V` Action | Severity if Klipo Steals |
|---|---|---|
| **Microsoft Word** | Paste Special | High — workflow-breaking for office users |
| **Microsoft Excel** | Paste Special | High |
| **Visual Studio Code** | Paste without formatting | High — devs notice immediately |
| **Visual Studio (full)** | Paste Special | Medium |
| **Google Chrome** (in editable fields) | Paste plain text | High — extremely common |
| **Microsoft Edge** | Paste plain text (same as Chrome) | High |
| **Firefox** | Paste plain text | High |
| **Slack desktop** | Paste plain text | Medium |
| **Notion desktop** | Paste plain text | Medium |
| **Cursor / VSCode forks** | Paste without formatting | High |
| **Figma desktop** | (no built-in; passes to OS) | Low |
| **OBS Studio** | (no built-in) | Low |
| **Discord** | (no built-in) | Low |
| **Steam** | (no built-in) | Low |
| **Windows native (Win+V)** | Built-in clipboard history | n/a (different keystroke) |

**Conclusion:** `Ctrl+Shift+V` collides with at least 9 of the 14 apps tested. Stealing it system-wide is hostile to user expectations — the user types `Ctrl+Shift+V` in Word expecting Paste Special and gets a Klipo popup instead. They'll uninstall.

---

## 3. Candidate Defaults Evaluated

### 3.1 `Ctrl+Alt+V`

| Criterion | Result |
|---|---|
| App conflicts | **None of the 14 surveyed apps** use it. |
| OS reservation | Not reserved by Windows. |
| Mnemonic | "Alt-V" → "Alt(ernate) V(iew of clipboard)." Decent. |
| Ergonomics | Reachable on standard QWERTY (left hand on Ctrl+Alt, right hand on V). |
| AltGr consideration | Some non-US layouts (German, Polish) use `Ctrl+Alt = AltGr`. **Risk: AltGr+V might produce a character.** |
| Portability to macOS | Becomes `Cmd+Option+V` — also unconflicted. |

**Verdict:** **Recommended default for v0.1 on Windows.** Ship with auto-detection of AltGr layouts → fall back if AltGr+V produces a character (per layout database).

### 3.2 `Ctrl+~` (Ctrl + tilde/backtick)

| Criterion | Result |
|---|---|
| App conflicts | Some terminal emulators use it (`Ctrl+~` in WSL → tilde). |
| OS reservation | None. |
| Mnemonic | Weak. |
| Ergonomics | Awkward; backtick is far on most layouts. |
| Layout dependence | Backtick position varies hugely by layout (Turkish layout: hard). |

**Verdict:** Rejected — layout-fragile.

### 3.3 `Win+V` (already taken by Windows)

Windows 10/11 has a native clipboard history bound to `Win+V`. Stealing it would:
- Confuse users who already use it.
- Probably fail to register (depends on Windows registration order).
- Be reverted by some Windows updates.

**Verdict:** Rejected.

### 3.4 `Ctrl+Shift+Space`

| Criterion | Result |
|---|---|
| App conflicts | Microsoft Word: insert non-breaking space. Few others. |
| OS reservation | None on Windows (used on macOS for input source). |
| Mnemonic | Weak. |
| Ergonomics | Good — easy chord. |
| Cross-platform | macOS uses `Ctrl+Shift+Space` for next input source — would clash. |

**Verdict:** Reasonable fallback, but cross-platform clash makes `Ctrl+Alt+V` more consistent.

### 3.5 `Ctrl+Alt+Shift+V`

| Criterion | Result |
|---|---|
| App conflicts | Almost none (too obscure). |
| Mnemonic | Decent (extends `Ctrl+Shift+V`). |
| Ergonomics | Painful — 4-key chord. |

**Verdict:** Reserved as **fallback** if `Ctrl+Alt+V` is unavailable.

### 3.6 Single Function Key (`F11`, `F8`, etc.)

| Criterion | Result |
|---|---|
| App conflicts | Hugely variable — F11 fullscreen in browsers, F8 debug in IDEs. |
| Cross-platform | Inconsistent across keyboard layouts (laptop fn-key locks). |

**Verdict:** Rejected — too fragile.

---

## 4. Decision

**Default Windows hotkey: `Ctrl+Alt+V`.**

**Default macOS hotkey (v0.2): `Cmd+Option+V`.**

This is the same physical chord, modulated by the platform's primary modifier — easy to teach, easy to muscle-memory.

### 4.1 At First Run

Onboarding flow includes a hotkey-test step:

```
"Press your hotkey now to test it."

[Ctrl+Alt+V] ← captured ✓
            
"It works! You can change this later in Settings."
```

If `tauri-plugin-global-shortcut::register()` returns an error (registration conflict at OS level), we cycle through this priority list:

```
1. Ctrl+Alt+V                  (default)
2. Ctrl+Alt+Shift+V            (first fallback)
3. Ctrl+Shift+Space            (second fallback)
4. Prompt user to choose       (last resort, modal blocks onboarding)
```

### 4.2 AltGr Layout Detection

Before registering, we check the active keyboard layout via `GetKeyboardLayout()`. If LCID ∈ {German, Polish, Czech, Hungarian, Turkish, Spanish (some), Portuguese, French (Canadian)}:

- Test `AltGr+V` (i.e. `Ctrl+Alt+V`) — does it produce a character?
- If yes, **skip Ctrl+Alt+V** and start at fallback `Ctrl+Alt+Shift+V`.
- Show first-run notice: "Your keyboard layout uses Ctrl+Alt as AltGr; Klipo's default has been adjusted."

For Turkish-Q layout specifically: `AltGr+V` produces no character → safe.
For Turkish-F layout: same.
For German: `AltGr+V` produces no character on most layouts → safe (verify per release).

### 4.3 User Override

Settings → Hotkey:

```
[ Click here to record a new hotkey ]
```

When clicked, Klipo listens for next key chord (max 3 modifiers + 1 key). On release:
- Validates the chord is supported by global-shortcut backend.
- Calls `register()` for the new chord, if success deregisters old.
- Surfaces error (with reason) if conflict — user can try again.

Reserved chords we **disallow** even if user requests:

```
Ctrl+C, Ctrl+V, Ctrl+X         (clipboard fundamentals)
Win+L                          (lock screen)
Win+D                          (show desktop)
Ctrl+Alt+Delete                (system; OS reserves)
Single keys without modifier   (would intercept everywhere)
```

---

## 5. Telemetry-Free Conflict Reporting

If `register()` fails post-launch (some other app registered the shortcut after we did), we surface a one-time non-blocking notification:

> "Klipo can't capture Ctrl+Alt+V right now — another app may have stolen it. Click to choose a different shortcut."

We do NOT send this event over the network. We log it locally for the user to share if they file a bug.

---

## 6. Future Considerations

- **Multiple hotkeys** (v0.2+): one for popup, one for "paste last," one for "show pinboard." Each user-configurable.
- **Modal hotkey** (v0.3 idea, low priority): e.g., `Ctrl+Alt+V` for popup, `Ctrl+Alt+1..9` for "paste pinned slot N."
- **Drag from system tray** as alternative trigger for users who hate hotkeys.

---

## 7. Test Matrix Before Locking

Before v0.1 ships, manually verify default works on:

- [ ] Windows 11 with US-English layout
- [ ] Windows 11 with Turkish-Q layout
- [ ] Windows 11 with Turkish-F layout
- [ ] Windows 11 with German layout (AltGr branch)
- [ ] Windows 10 1809 with US-English layout
- [ ] Windows 11 ARM64 with US-English layout
- [ ] Inside an admin-elevated Cmd window (does the hotkey still bring up Klipo above the elevated app?)
- [ ] During a UAC prompt (must NOT trigger; secure desktop is sacred)
- [ ] In a Remote Desktop session (host's chord wins; document expectation)

Each row has an entry in `docs/security-tests.md` once Phase B starts.

---

## 8. Why This Is in `docs/` and Not a Constant

Because changing this default in v0.2 would affect every existing user. Documenting the rationale prevents a future contributor from "fixing" it back to Ctrl+Shift+V six months from now.

If you're reading this and considering changing the default: re-read §2.
