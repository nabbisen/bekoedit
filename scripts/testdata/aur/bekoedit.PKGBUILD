# Arch Linux PKGBUILD for bekoedit.
# makepkg downloads the source directly from GitHub's own per-tag archive.

pkgname=bekoedit
pkgver=9.9.9
pkgrel=1
pkgdesc="A source-preserving Markdown editor"
arch=('x86_64')
url="https://github.com/nabbisen/bekoedit"
license=('Apache-2.0')
# xdotool provides libxdo, which the binary links. A source build links
# the host's own libxdo, for example libxdo.so.4 on Arch (RFC-045: readelf
# -d on a release build confirms it) -- without xdotool installed the
# package builds and installs cleanly and then fails to start. Temporary:
# once Dioxus 0.8 drops the libxdo linkage, this dependency should be
# removed.
depends=('webkit2gtk-4.1' 'gtk3' 'xdotool' 'openssl')
makedepends=('cargo' 'pkg-config')
source=("$pkgname-$pkgver.tar.gz::https://github.com/nabbisen/bekoedit/archive/refs/tags/$pkgver.tar.gz")
sha256sums=('09fb22c5bc55515ac2d22b241d2dde1b1e5f3eb0d226bb6003442060bc6f407e')

prepare() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release -p bekoedit
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm755 "target/release/bekoedit" "$pkgdir/usr/bin/bekoedit"
    install -Dm644 "packaging/linux/bekoedit.desktop" \
        "$pkgdir/usr/share/applications/bekoedit.desktop"
    install -Dm644 "packaging/windows/Assets/Square150x150Logo.png" \
        "$pkgdir/usr/share/pixmaps/bekoedit.png"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 "NOTICE" "$pkgdir/usr/share/licenses/$pkgname/NOTICE"
}
