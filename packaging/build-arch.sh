#!/bin/bash
# Build script for Protonic Arch Linux package
# Run this from the protonic project directory

set -e

VERSION="0.2.2"
BUILD_DIR="$(pwd)/arch-build"

echo "=== Building Protonic Arch Package ==="

# Clean previous build
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# Copy PKGBUILD
cp packaging/arch/PKGBUILD "$BUILD_DIR/"

# Build the package
cd "$BUILD_DIR"
makepkg -sf

echo ""
echo "=== Package built successfully ==="
echo "Output: $(ls *.pkg.tar.zst 2>/dev/null || ls *.pkg.tar.xz)"
echo ""
echo "Install with: sudo pacman -U protonic-*.pkg.tar.zst"
