# Getting Started

## Install / build

Prebuilt archives are published per release as
`bekoedit-<version>.tar.gz`. To build from source you need Rust 1.85+
(edition 2024):

```sh
cargo run -p bekoedit-app
```

On Linux, install the WebView dependencies first:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev
```

## Open a workspace

bekoedit edits Markdown files inside a folder you choose — the
*workspace*. On the start screen, enter a folder path (or pick a recent
workspace). The explorer lists Markdown files (`.md`, `.markdown`),
skipping noisy directories like `.git` and `node_modules`.

From the explorer you can create files (`.md` is appended automatically),
rename, and delete (to the system trash). All operations are confined to
the workspace; paths can never escape it.

## Language

The GUI ships in English and Japanese; toggle with the language button in
the header.
