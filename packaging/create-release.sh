#!/bin/bash
# Create a release tarball for GitHub releases
# Run this from the protonic project directory after building with cargo build --release

set -e

VERSION="0.2.0"
RELEASE_NAME="protonic-${VERSION}-x86_64-linux"
RELEASE_DIR="$(pwd)/release-build/${RELEASE_NAME}"

echo "=== Creating Protonic Release Tarball ==="

# Check if binary exists
if [ ! -f "target/release/protonic" ]; then
    echo "Error: Binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Clean previous build
rm -rf "$(pwd)/release-build"
mkdir -p "$RELEASE_DIR"

# Download protonhax if not present
if [ ! -f "protonhax" ]; then
    echo "Downloading protonhax..."
    curl -L -o protonhax "https://raw.githubusercontent.com/jcnils/protonhax/master/protonhax"
fi

# Copy files
echo "Copying files..."
cp target/release/protonic "$RELEASE_DIR/"
cp protonhax "$RELEASE_DIR/"
cp packaging/protonic.desktop "$RELEASE_DIR/"
cp ui/icon.png "$RELEASE_DIR/"
cp README.md "$RELEASE_DIR/"

# Create install script
cat > "$RELEASE_DIR/install.sh" << 'EOF'
#!/bin/bash
# Protonic installer script
set -e

echo "Installing Protonic..."

# Check for root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./install.sh)"
    exit 1
fi

# Install binaries
install -Dm755 protonic /usr/bin/protonic
install -Dm755 protonhax /usr/bin/protonhax

# Install desktop file and icon
install -Dm644 protonic.desktop /usr/share/applications/protonic.desktop
install -Dm644 icon.png /usr/share/icons/hicolor/256x256/apps/protonic.png

# Update icon cache
gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true

echo "Protonic installed successfully!"
echo "You can now launch it from your application menu or by running 'protonic'"
EOF
chmod 755 "$RELEASE_DIR/install.sh"

# Create uninstall script
cat > "$RELEASE_DIR/uninstall.sh" << 'EOF'
#!/bin/bash
# Protonic uninstaller script
set -e

echo "Uninstalling Protonic..."

# Check for root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./uninstall.sh)"
    exit 1
fi

# Remove files
rm -f /usr/bin/protonic
rm -f /usr/bin/protonhax
rm -f /usr/share/applications/protonic.desktop
rm -f /usr/share/icons/hicolor/256x256/apps/protonic.png

# Update icon cache
gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true

echo "Protonic uninstalled successfully!"
EOF
chmod 755 "$RELEASE_DIR/uninstall.sh"

# Create tarball
cd "$(pwd)/release-build"
tar -czvf "${RELEASE_NAME}.tar.gz" "${RELEASE_NAME}"

# Move to project root
mv "${RELEASE_NAME}.tar.gz" "../"

echo ""
echo "=== Release tarball created successfully ==="
echo "Output: ${RELEASE_NAME}.tar.gz"
echo ""
echo "Contents:"
tar -tvf "../${RELEASE_NAME}.tar.gz"
