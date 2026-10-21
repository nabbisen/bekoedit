# Arch Linux PKGBUILD for bekoedit-bin: the prebuilt Linux binary, built in an
# Arch Linux container and published with each release (RFC-045 slice 3a).

pkgname=bekoedit-bin
pkgver=9.9.9
pkgrel=1
pkgdesc="A source-preserving Markdown editor (prebuilt binary)"
arch=('x86_64')
url="https://github.com/nabbisen/bekoedit"
license=('Apache-2.0')
# xdotool provides libxdo, which the binary links (libxdo.so.4, as built on
# Arch). It is listed only because Dioxus 0.7 links libxdo: without it the
# package installs cleanly and then fails to start. Temporary: once Dioxus 0.8
# drops the libxdo linkage, this dependency should be removed.
depends=('webkit2gtk-4.1' 'gtk3' 'xdotool' 'openssl')
provides=('bekoedit')
conflicts=('bekoedit')
# Each source gets a local name that includes $pkgname-$pkgver, so a cached file
# from another version is never reused. The archive is the release's Arch build,
# which is flat (the binary, LICENSE and NOTICE at its root); the desktop file
# and the icon are taken from the same tag.
source=("$pkgname-$pkgver-x86_64.tar.gz::https://github.com/nabbisen/bekoedit/releases/download/$pkgver/bekoedit-$pkgver-x86_64-unknown-linux-gnu-arch.tar.gz"
        "$pkgname-$pkgver.desktop::https://raw.githubusercontent.com/nabbisen/bekoedit/$pkgver/packaging/linux/bekoedit.desktop"
        "$pkgname-$pkgver.png::https://raw.githubusercontent.com/nabbisen/bekoedit/$pkgver/packaging/windows/Assets/Square150x150Logo.png")
sha256sums=('09fb22c5bc55515ac2d22b241d2dde1b1e5f3eb0d226bb6003442060bc6f407e' '1a79cf84db44f847d2341652d13a4410dcc83f2109d2ea9ab89869103022d186' '942cf5a343e968a56440bae1374513006a84c1f2ae9906e16b133ca612aa2b3b')

package() {
    install -Dm755 "$srcdir/bekoedit" "$pkgdir/usr/bin/bekoedit"
    install -Dm644 "$srcdir/$pkgname-$pkgver.desktop" \
        "$pkgdir/usr/share/applications/bekoedit.desktop"
    install -Dm644 "$srcdir/$pkgname-$pkgver.png" \
        "$pkgdir/usr/share/pixmaps/bekoedit.png"
    install -Dm644 "$srcdir/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 "$srcdir/NOTICE" "$pkgdir/usr/share/licenses/$pkgname/NOTICE"
}
