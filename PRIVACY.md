# Privacy Policy

**Effective:** 2026-10-18

This policy covers bekoedit on every platform, including the Microsoft Store
package.

## 1. No data collection

bekoedit does not collect, share or transmit any personal information or
document content. It has no account, no telemetry, no analytics, no
advertising, no crash reporting and no update check. Nothing about you or your
documents is sent to the developer or to anyone else.

## 2. Network access

**bekoedit itself does not contact any server.** It works fully offline.

A network request happens only when your document or your own action asks for
one:

- **Images in Preview.** If a document refers to an image by a web address
  (`http://` or `https://`), Preview loads that image from the address, as a
  web browser would. The site hosting the image can see the request, including
  your IP address.
- **Links.** Clicking a web link (`http://`, `https://`) opens it in your
  default browser, and an email link (`mailto:`) in your mail app. Other kinds
  of link do not open. From then on, that app's own privacy terms apply.

## 3. Data kept on your device

Your documents are read and written only where you choose.

To work, bekoedit also keeps these files on your device, outside your
documents' folders. None of them is ever uploaded:

| What | Contents |
|---|---|
| Settings | Your preferences, such as the interface language |
| Recent workspaces | The paths of folders you opened recently |
| Recovery snapshots | Copies of unsaved changes, so they survive a crash. Removed after the document is saved |
| Save history | A copy of each document at each of its last 50 saves, which you can restore |

The clipboard is read only when you paste.

**To remove them,** delete the `bekoedit` folder in your system's
configuration and application-data folders. In the Microsoft Store version,
Windows keeps them in the app's own package storage, and removes them when you
uninstall the app.

## 4. File and system access

On Windows, the Microsoft Store package requests the `runFullTrust`
capability. That is what a desktop application needs to open and save files
where you choose. All file operations and settings are processed locally on
your device. No data is sent to external servers.

## 5. Other software

- **The system WebView.** bekoedit displays its window through your system's
  web view component: the Microsoft Edge WebView2 Runtime on Windows, WebKitGTK
  on Linux, and WKWebView on macOS. Each is covered by its provider's own
  privacy terms.
- **Git.** If Git is installed and an opened folder is a Git repository,
  bekoedit runs `git status` locally, to show which files have changed.
  Nothing is sent.

## 6. Changes to this policy

Changes are published in this file, in the project's repository, with a new
effective date. Its full history is in the repository's commit log.

## 7. Contact

If you have any questions, please open an issue:
<https://github.com/nabbisen/bekoedit/issues>
