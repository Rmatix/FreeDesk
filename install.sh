#!/usr/bin/env bash
# FreeDesk — One-line installer for Linux (x86_64)
# Usage: curl -fsSL https://raw.githubusercontent.com/Rmatix/FreeDesk/main/install.sh | bash

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}"
echo "=========================================================="
echo "   ⚡ FreeDesk — Ultra-Fast Remote Desktop for Linux      "
echo "   100% Free, Open Source (MIT), Zero-Knowledge E2EE     "
echo "=========================================================="
echo -e "${NC}"

# 1. Architecture check
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    echo -e "${RED}Error: Currently only x86_64 Linux architecture is supported (detected: $ARCH).${NC}"
    exit 1
fi

OS=$(uname -s)
if [ "$OS" != "Linux" ]; then
    echo -e "${RED}Error: This installer is intended for Linux (detected: $OS).${NC}"
    exit 1
fi

# 2. Destination directories
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/128x128/apps"
mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR" "$ICON_DIR"

BIN_PATH="$INSTALL_DIR/freedesk"

echo -e "${YELLOW}>> Downloading latest FreeDesk Linux binary from GitHub Releases...${NC}"
LATEST_URL="https://github.com/Rmatix/FreeDesk/releases/latest/download/freedesk-linux-x86_64"

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$LATEST_URL" -o "$BIN_PATH"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$BIN_PATH" "$LATEST_URL"
else
    echo -e "${RED}Error: Neither curl nor wget was found. Please install one to continue.${NC}"
    exit 1
fi

chmod +x "$BIN_PATH"
echo -e "${GREEN}✓ Binary downloaded and made executable: $BIN_PATH${NC}"

# 3. Create desktop launcher shortcut
DESKTOP_FILE="$DESKTOP_DIR/freedesk.desktop"
cat << 'EOF' > "$DESKTOP_FILE"
[Desktop Entry]
Name=FreeDesk
Comment=Ultra-Performance Zero-Knowledge Remote Desktop
Exec=freedesk
Terminal=false
Type=Application
Categories=Network;RemoteAccess;Utility;
Keywords=remote;desktop;anydesk;teamviewer;vnc;
EOF

# Ensure ~/.local/bin is in PATH for the user session if not already
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo -e "${YELLOW}Note: Adding ~/.local/bin to ~/.bashrc for PATH access${NC}"
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
    export PATH="$HOME/.local/bin:$PATH"
fi

echo ""
echo -e "${GREEN}==========================================================${NC}"
echo -e "${GREEN}   ✓ FreeDesk installed successfully!                      ${NC}"
echo -e "${GREEN}==========================================================${NC}"
echo ""
echo "To start FreeDesk right now, run:"
echo -e "   ${BLUE}$BIN_PATH${NC}"
echo "or simply:"
echo -e "   ${BLUE}freedesk${NC}"
echo ""
echo "You can also launch FreeDesk directly from your application menu / dash."
