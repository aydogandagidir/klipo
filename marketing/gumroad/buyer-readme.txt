Klipo — by bluedev

Thanks for buying. Below is everything you need to get going. It's
shorter than it looks; most of this is troubleshooting tips you
hopefully won't need.


Your license key

Gumroad emailed it to the address you used at checkout. If you
can't find the email, the key is also visible in your Gumroad
library at gumroad.com/library. Click on "Klipo" and it's on
that page — a single line that starts with letters and dashes.


Activation

Step 1 — Download Klipo

Grab the latest installer from your Gumroad library, or from
bluedev.dev/products/klipo (the GitHub Releases mirror is also
linked there for archival download). The file is called
something like Klipo_0.1.3_x64-setup.exe and it's ~3.8 MB.

Step 2 — Install it

Double-click the .exe. Windows SmartScreen will warn you because
the cert is fresh — click "More info" then "Run anyway". This
is normal for indie apps; it goes away after enough downloads
build SmartScreen reputation.

Step 3 — First run = trial starts automatically

Klipo opens, the popup briefly previews itself, and your 14-day
free trial starts. Every Pro feature is on right now — capture,
search, sensitive flagging, drag-drop. Nothing locked.

Step 4 — Activate Pro forever

When you're ready (or when the trial countdown nudges you), open
the Klipo tray icon → Settings (gear) → License tab. Paste the
email you used at Gumroad checkout, paste your license key, then
click "Activate". You should see "Klipo Pro — Activated" within
about three seconds. The trial countdown disappears.

Step 5 — That's it

Hit Ctrl+Alt+V from anywhere to open the popup. Type to filter.
Hit Enter to paste back. The first 24 hours are when Klipo
quietly fills its history; by tomorrow you'll have a few hundred
clips ready to search.

If activation fails, it's almost always one of two things:
either the email doesn't match what you used to buy, or there's
a stray space in the key. If neither of those is it, write to
support@bluedev.dev with your Gumroad purchase email and I'll
sort it.


What Pro turns on

It's the same Klipo binary you already have. The key just
removes the 14-day expiry. Specifically:

The full clipboard history. Trial captures everything from day
one — Pro just keeps capturing past day 14 with no interruption.

Unlimited search. <50ms across your entire history, no item
count cap, no archival rollover.

Sensitive auto-flagging stays on. API keys, JWTs, AWS secrets,
Stripe keys, credit card numbers, and a few other patterns get
a red border + blurred preview. 13 detectors total, all running
locally.

Re-scan history button. If you upgrade after installing, the
Settings → Privacy tab has a button that retroactively scans
your existing clip history with the latest detector patterns.
Useful when v0.1.4+ adds new patterns.

Drag-and-drop, file paths, image clips, multi-device install,
priority support — same as trial.


Privacy — how the activation actually works

Klipo's license check goes from your machine straight to
api.gumroad.com/v2/licenses/verify. There is no bluedev server
in the middle proxying anything. The request body contains
exactly two fields: your product permalink ("klipo") and the
license key. That's it.

No clipboard data, no usage data, no machine fingerprint, no
analytics — none of it leaves your computer. The clipboard
itself never touches the network at all; it lives in a SQLite
file under %APPDATA%\Klipo\ on Windows.

You can verify all of this by watching network traffic during
activation, or by reading the source — license.rs is ~140 lines
and self-contained. The repo's at github.com/bluedev/klipo
(or bluedev.dev/products/klipo for the linked mirror).


License terms in plain language

One-time payment, lifetime use, all future v0.x and v1.x Pro
updates included. No subscription, no future charges.

You can install Klipo on up to 3 personal devices (laptop,
desktop, work PC, etc.) and activate Pro on each. Same key works
across all of them. Just don't share or resell the key, please.

The license verifies once when you activate, then re-checks
weekly in the background. If you go offline, Pro stays active
for 30 days from the last successful check, so traveling or
patchy wifi won't lock you out.

macOS build is coming in v0.2 (target Q3 2026). Your license
covers it — same key, no extra payment. You'll get an email
when the macOS build ships.

Refunds within 30 days, no questions, through Gumroad's normal
flow.


Help

support@bluedev.dev. I aim for a one-business-day response on
Pro support.

If something's broken — Klipo crashes, popup won't open, search
returns nothing — open Settings → About → Copy diagnostics. That
puts a JSON snippet on your clipboard with your Klipo version,
Windows version, and recent error messages. Paste that into the
email, saves us a few back-and-forths.

Web: bluedev.dev/products/klipo


Legal

Klipo is an independent project by bluedev. EULA, privacy policy,
and refund terms ship inside the install:

  EULA          - %INSTALLDIR%\LEGAL\EULA.md
  Privacy       - %INSTALLDIR%\LEGAL\PRIVACY.md
  Refund policy - %INSTALLDIR%\LEGAL\REFUND.md

Or the same docs at bluedev.dev/legal/klipo.

Klipo is not affiliated with, endorsed by, or sponsored by
Microsoft Corporation. Windows is a trademark of Microsoft
Corporation.


That's everything. Hope Klipo saves you the "wait, what did I
copy 5 minutes ago?" moment a thousand times over.

Aydoğan
bluedev
