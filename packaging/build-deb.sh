#!/bin/bash
# Build script for Protonic Debian package
# Run this from the protonic project directory after building with cargo build --release

set -e

VERSION="0.2.0"
PKG_NAME="protonic_${VERSION}_amd64"
BUILD_DIR="$(pwd)/deb-build"
PKG_DIR="${BUILD_DIR}/${PKG_NAME}"

echo "=== Building Protonic Debian Package ==="

# Check if binary exists
if [ ! -f "target/release/protonic" ]; then
    echo "Error: Binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Clean previous build
rm -rf "$BUILD_DIR"
mkdir -p "$PKG_DIR"

# Create directory structure
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/256x256/apps"

# Download protonhax if not present
if [ ! -f "protonhax" ]; then
    echo "Downloading protonhax..."
    curl -L -o protonhax "https://raw.githubusercontent.com/jcnils/protonhax/master/protonhax"
fi

# Copy files
echo "Copying files..."
cp target/release/protonic "$PKG_DIR/usr/bin/"
cp protonhax "$PKG_DIR/usr/bin/"
chmod 755 "$PKG_DIR/usr/bin/protonic"
chmod 755 "$PKG_DIR/usr/bin/protonhax"

cp packaging/protonic.desktop "$PKG_DIR/usr/share/applications/"
cp ui/icon.png "$PKG_DIR/usr/share/icons/hicolor/256x256/apps/protonic.png"

# Create control file
cat > "$PKG_DIR/DEBIAN/control" << EOF
Package: protonic
Version: ${VERSION}
Section: games
Priority: optional
Architecture: amd64
Depends: libc6, libgcc-s1, python3
Recommends: steam-installer | steam
Maintainer: LunaBaloona
Description: Launch Windows programs inside Steam Proton environments.
 Protonic is a GUI application that allows users to easily launch
 Windows programs inside Steam games' Proton environments using
 protonhax. Useful if you want to run various mods with your game.
Homepage: https://github.com/LunaBaloona/protonic
EOF

# Set correct permissions
find "$PKG_DIR" -type d -exec chmod 755 {} \;
find "$PKG_DIR/DEBIAN" -type f -exec chmod 644 {} \;

# Build the package
echo "Building .deb package..."
dpkg-deb --build "$PKG_DIR"

# Move to project root
mv "${PKG_DIR}.deb" "./${PKG_NAME}.deb"

echo ""
echo "=== Package built successfully ==="
echo "Output: ${PKG_NAME}.deb"
echo ""
echo "Install with: sudo dpkg -i ${PKG_NAME}.deb"
echo "Or:           sudo apt install ./${PKG_NAME}.deb"
