# Installing bekoedit

bekoedit binaries are **unsigned** for early releases. This page explains
what that means on each platform and how to run the app anyway.

---

## macOS — Gatekeeper

Because the binary is not signed with an Apple Developer ID, macOS will
block it by default.

**First-run workaround:**

1. Download `bekoedit-<version>-aarch64-apple-darwin.tar.gz` (or the
   `x86_64` variant for Intel Macs).
2. Extract the archive. Move `bekoedit` to `/Applications` or `~/Applications`
   if you prefer.
3. Right-click (or Ctrl-click) the `bekoedit` icon in Finder and choose
   **Open**.
4. A dialog appears warning that the developer cannot be verified. Click
   **Open** to proceed.

Subsequent launches work normally — you only need the right-click workaround
once.

If you see *"bekoedit is damaged and can't be opened"*, run:

```sh
xattr -d com.apple.quarantine /path/to/bekoedit
```

This clears the quarantine attribute that macOS set when you downloaded
the file.

---

## Windows — SmartScreen

Because the binary lacks an Authenticode signature, Windows SmartScreen
will display a warning on first run.

**How to proceed:**

1. Download `bekoedit-<version>-x86_64-pc-windows-msvc.zip` and extract it.
2. Double-click `bekoedit.exe`.
3. SmartScreen shows *"Windows protected your PC"*. Click **More info**,
   then **Run anyway**.

SmartScreen reputation builds over time as more users run the binary —
warnings will eventually disappear for signed/popular builds.

---

## Linux

Linux distributions do not impose a blanket code-signing requirement.
After extracting the archive, mark the binary executable if needed:

```sh
chmod +x bekoedit
./bekoedit
```

On distributions using AppArmor or SELinux you may need to allow the
binary or place it in a permitted path. The WebView requires
`libwebkit2gtk-4.1` (Debian/Ubuntu: `sudo apt install libwebkit2gtk-4.1-0`
if it is not already installed).

---

## Why unsigned?

Paid Apple Developer IDs (USD 99/year) and Windows EV certificates (several
hundred USD/year) are not included in the initial project budget. bekoedit
is fully open-source — you can audit the code, reproduce the build from
source, or verify checksums published alongside each release.

Once the project reaches a sustainable state, signed distribution through
official channels (Mac App Store, Microsoft Store) is on the future roadmap.

---

## Checksum verification

Each release page on GitHub includes SHA-256 checksums for all artifacts.
Verify before running:

```sh
# macOS / Linux
shasum -a 256 bekoedit-<version>-<target>.tar.gz

# Windows (PowerShell)
Get-FileHash bekoedit-<version>-<target>.zip -Algorithm SHA256
```

Compare against the `checksums.txt` file attached to the release.
