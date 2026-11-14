#!/usr/bin/env bash
# FreeDesk — One-line installer / updater for Linux (x86_64)
# Usage: curl -fsSL https://github.com/Rmatix/FreeDesk/releases/latest/download/install.sh | bash
#   or:  curl -fsSL https://raw.githubusercontent.com/Rmatix/FreeDesk/main/install.sh | bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}${BOLD}"
echo "=========================================================="
echo "   ⚡ FreeDesk — Ultra-Fast Remote Desktop for Linux     "
echo "   100% Free, Open Source (MIT), Zero-Knowledge E2EE    "
echo "=========================================================="
echo -e "${NC}"

# ── 1. Architecture & OS check ───────────────────────────────────────────────
ARCH=$(uname -m)
OS=$(uname -s)

if [ "$OS" != "Linux" ]; then
    echo -e "${RED}✗ Error: Este instalador es para Linux (detectado: $OS).${NC}"
    exit 1
fi

if [ "$ARCH" != "x86_64" ]; then
    echo -e "${RED}✗ Error: Sólo se soporta x86_64 por ahora (detectado: $ARCH).${NC}"
    echo "  Compila manualmente: https://github.com/Rmatix/FreeDesk"
    exit 1
fi

echo -e "${GREEN}✓ Sistema operativo: Linux x86_64${NC}"

# ── 2. Check for curl or wget ────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
else
    echo -e "${RED}✗ Error: Se necesita 'curl' o 'wget'. Instala uno:${NC}"
    echo "  sudo apt install curl   # Debian/Ubuntu"
    echo "  sudo dnf install curl   # Fedora/RHEL"
    exit 1
fi

echo -e "${GREEN}✓ Descargador: $DOWNLOADER${NC}"

# ── 3. Resolve latest release version via GitHub API ─────────────────────────
echo -e "${YELLOW}>> Consultando última versión en GitHub...${NC}"

REPO="Rmatix/FreeDesk"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"

if [ "$DOWNLOADER" = "curl" ]; then
    LATEST_JSON=$(curl -fsSL "$API_URL" 2>/dev/null) || true
else
    LATEST_JSON=$(wget -qO- "$API_URL" 2>/dev/null) || true
fi

# Parse tag_name from JSON (no jq needed)
VERSION=$(echo "$LATEST_JSON" | grep -o '"tag_name": *"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')

if [ -z "$VERSION" ]; then
    # Fallback to known latest version if API fails
    VERSION="v0.10.1"
    echo -e "${YELLOW}  (No se pudo consultar la API, usando versión de respaldo: ${VERSION})${NC}"
else
    echo -e "${GREEN}✓ Última versión disponible: ${BOLD}${VERSION}${NC}"
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/freedesk-linux-x86_64"

# ── 4. Check if already installed and show current version ───────────────────
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/128x128/apps"
BIN_PATH="$INSTALL_DIR/freedesk"

if [ -f "$BIN_PATH" ]; then
    echo -e "${CYAN}  FreeDesk ya está instalado. Actualizando a ${VERSION}...${NC}"
else
    echo -e "${CYAN}  Instalando FreeDesk ${VERSION} por primera vez...${NC}"
fi

mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR" "$ICON_DIR"

# ── 5. Download binary ────────────────────────────────────────────────────────
echo -e "${YELLOW}>> Descargando binario Linux desde:${NC}"
echo "   $DOWNLOAD_URL"
echo ""

TMP_BIN=$(mktemp /tmp/freedesk_XXXXXX)
trap 'rm -f "$TMP_BIN"' EXIT

DOWNLOAD_OK=0
if [ "$DOWNLOADER" = "curl" ]; then
    if curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$TMP_BIN"; then
        DOWNLOAD_OK=1
    fi
else
    if wget --show-progress -qO "$TMP_BIN" "$DOWNLOAD_URL"; then
        DOWNLOAD_OK=1
    fi
fi

if [ "$DOWNLOAD_OK" = "0" ] || [ ! -s "$TMP_BIN" ]; then
    echo ""
    echo -e "${RED}✗ Error: No se pudo descargar el binario.${NC}"
    echo ""
    echo "  Posibles causas:"
    echo "  1. Sin conexión a Internet o GitHub no disponible"
    echo "  2. El release ${VERSION} no existe aún en GitHub"
    echo ""
    echo "  Verifica manualmente: https://github.com/${REPO}/releases"
    echo ""
    echo "  Descarga directa alternativa:"
    echo "    wget '${DOWNLOAD_URL}' -O ~/.local/bin/freedesk"
    echo "    chmod +x ~/.local/bin/freedesk"
    exit 1
fi

# Verify it's actually an ELF binary (not an HTML error page)
FILE_TYPE=$(file "$TMP_BIN" 2>/dev/null | head -1)
if ! echo "$FILE_TYPE" | grep -q "ELF"; then
    echo ""
    echo -e "${RED}✗ Error: El archivo descargado no es un ejecutable válido.${NC}"
    echo "  Tipo detectado: $FILE_TYPE"
    echo "  Verifica: https://github.com/${REPO}/releases/tag/${VERSION}"
    exit 1
fi

# Move to final location
mv "$TMP_BIN" "$BIN_PATH"
chmod +x "$BIN_PATH"
trap - EXIT  # cancel tmp cleanup since file was moved

echo -e "${GREEN}✓ Binario instalado en: $BIN_PATH${NC}"

# ── 6. Create desktop launcher ────────────────────────────────────────────────
DESKTOP_FILE="$DESKTOP_DIR/freedesk.desktop"
cat > "$DESKTOP_FILE" << DESKTOPEOF
[Desktop Entry]
Name=FreeDesk
GenericName=Remote Desktop
Comment=Ultra-Performance Zero-Knowledge Remote Desktop - v${VERSION}
Exec=${BIN_PATH}
Terminal=false
Type=Application
Categories=Network;RemoteAccess;Utility;
Keywords=remote;desktop;anydesk;teamviewer;vnc;rdp;
StartupNotify=true
DESKTOPEOF

echo -e "${GREEN}✓ Acceso directo del escritorio creado${NC}"

# Update desktop database if available
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# ── 7. Ensure ~/.local/bin is on PATH ────────────────────────────────────────
PATH_ADDED=0
for SHELL_RC in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [ -f "$SHELL_RC" ]; then
        if ! grep -q '\.local/bin' "$SHELL_RC" 2>/dev/null; then
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_RC"
            PATH_ADDED=1
        fi
        break
    fi
done

export PATH="$HOME/.local/bin:$PATH"

if [ "$PATH_ADDED" = "1" ]; then
    echo -e "${YELLOW}  Nota: Se añadió ~/.local/bin al PATH en tu shell rc.${NC}"
    echo -e "${YELLOW}  Ejecuta: source ~/.bashrc   (o abre una terminal nueva)${NC}"
fi

# ── 8. Quick sanity check: can the binary be executed? ───────────────────────
echo ""
echo -e "${YELLOW}>> Verificando que el binario se puede ejecutar...${NC}"

# Check required libraries
if command -v ldd >/dev/null 2>&1; then
    MISSING=$(ldd "$BIN_PATH" 2>&1 | grep "not found" || true)
    if [ -n "$MISSING" ]; then
        echo -e "${YELLOW}  Advertencia: Algunas librerías pueden faltar:${NC}"
        echo "$MISSING" | sed 's/^/    /'
        echo ""
        echo "  Instala dependencias:"
        echo "    Ubuntu/Debian: sudo apt install libxcb1 libxcb-xtest0 libx11-6 libgl1"
        echo "    Fedora/RHEL:   sudo dnf install libxcb xcb-util libX11 mesa-libGL"
    else
        echo -e "${GREEN}✓ Todas las librerías necesarias están presentes${NC}"
    fi
fi

# ── 9. Done! ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}=========================================================="
echo "   ✓ FreeDesk ${VERSION} instalado correctamente!"
echo "==========================================================${NC}"
echo ""
echo -e "  Para iniciar ahora:  ${BOLD}${BLUE}${BIN_PATH}${NC}"
echo -e "  O simplemente:       ${BOLD}${BLUE}freedesk${NC}"
echo ""
echo "  Para conectarte a Windows:"
echo "    1. Abre FreeDesk en Linux y anota tu ID"
echo "    2. Abre FreeDesk en Windows, ingresa el ID y PIN de Linux"
echo "    3. Acepta la conexión en Linux → ¡listo!"
echo ""
echo "  Repositorio:  https://github.com/${REPO}"
echo "  Releases:     https://github.com/${REPO}/releases"
echo ""
