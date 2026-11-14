# 🖥️ FreeDesk — High-Performance Open Source Remote Desktop

> **FreeDesk** es una alternativa moderna, ultrarrápida, 100% gratuita y open-source a AnyDesk y TeamViewer, construida íntegramente en **Rust** para máximo rendimiento, mínimo uso de CPU/RAM, seguridad de grado militar y **soporte multiplataforma completo (Windows & Linux)**.

---

## ⚡ Especificaciones de Rendimiento

| Métrica | AnyDesk / Software tradicional | FreeDesk (Rust) |
|---|---|---|
| **Tamaño del Ejecutable** | ~20 - 40 MB | **~4.5 MB** (.exe / ELF portable) |
| **Consumo de Memoria RAM** | ~70 - 150 MB | **~15 - 25 MB** |
| **Uso de CPU** | 10% - 25% | **~2% - 5%** |
| **Tasa de Refresco** | 30 - 60 FPS | **60 FPS** continuo (DXGI / X11) |
| **Compresión** | H.264/zstd (~200 MB/s) | **LZ4 + Tile Delta 128×128** (5 GB/s) |
| **Audio** | Retraso audible (~200ms) | **WASAPI Loopback** (~15ms) |
| **Cifrado** | Cerrado / Propietario | **X25519 + AES-256-GCM (Zero-Knowledge)** |
| **Licencia** | Comercial / Restrictiva | **MIT (100% Libre y Gratuito)** |

---

## 🐧 Instalación en Linux

FreeDesk se puede instalar en cualquier distribución Linux (Ubuntu, Debian, Fedora, Arch, Manjaro, Mint, etc.) de dos formas:

### Opción 1: Un solo comando en la Terminal (Recomendado)
Abre tu terminal y ejecuta:
```bash
curl -fsSL https://raw.githubusercontent.com/Rmatix/FreeDesk/main/install.sh | bash
```
> Esto descarga el binario `freedesk-linux-x86_64`, le da permisos de ejecución, lo instala en `~/.local/bin/freedesk` y crea automáticamente el lanzador en tu menú de aplicaciones.

O descarga y ejecuta directamente sin instalación:
```bash
curl -fsSL https://github.com/Rmatix/FreeDesk/releases/latest/download/freedesk-linux-x86_64 -o freedesk && chmod +x freedesk && ./freedesk
```

### Opción 2: Descarga de archivo
1. Ve a [Releases en GitHub](https://github.com/Rmatix/FreeDesk/releases).
2. Descarga `freedesk-linux-x86_64` o `freedesk-linux-x86_64.tar.gz`.
3. Dale permisos de ejecución:
   ```bash
   chmod +x freedesk-linux-x86_64
   ./freedesk-linux-x86_64
   ```

---

## 🪟 Instalación en Windows

1. Descarga el ejecutable `freedesk.exe` desde [Releases en GitHub](https://github.com/Rmatix/FreeDesk/releases).
2. Haz doble clic para abrirlo (aplicación 100% portable, no requiere instalación ni drivers de terceros).
3. Opcionalmente haz clic en **"🛡️ Iniciar como Admin"** para gestionar pantallas de UAC y enviar combinaciones especiales como Ctrl+Alt+Del.

---

## 🔄 Conectividad Multiplataforma (Windows ⟷ Linux)

FreeDesk permite conectarse y controlar de forma indistinta y totalmente fluida entre ambos sistemas:

- 🖥️ **Windows ➡️ Linux**: Conéctate desde tu PC con Windows al escritorio de un equipo con Linux. Puedes ver su pantalla en tiempo real (X11) y controlar su mouse y teclado.
- 🐧 **Linux ➡️ Windows**: Conéctate desde Linux para controlar un PC con Windows, ver su pantalla a 60 FPS (DXGI) y gestionar el sistema remoto.
- 🪟 **Windows ➡️ Windows** y 🐧 **Linux ➡️ Linux**: Total compatibilidad.

Ambos clientes utilizan el mismo servidor Relay en la nube (`wss://freedesk-relay.onrender.com`) y el mismo protocolo de cifrado de extremo a extremo (E2EE) con clave efímera **X25519** y **AES-256-GCM**.

---

## 🚀 Características Principales

- 🖥️ **Captura de Pantalla Tile-Delta 128×128**:
  - En Windows: **DXGI Desktop Duplication** directo en GPU (< 0.5 ms de latencia) con fallback a GDI.
  - En Linux: Captura nativa **X11 ZPixmap / SHM**.
  - Hash de tiles ultrarrápido con **xxHash3** y compresión en paralelo con **Rayon** + **LZ4 Flex**. Solo transmite los bloques modificados ahorrando el 95% del ancho de banda.
- 🎯 **Dual Cursor y Prioridad de Host**:
  - El usuario anfitrión siempre mantiene el control físico de su mouse (si mueve el mouse, la entrada remota se pausa por 900 ms).
  - El anfitrión puede ver un "cursor fantasma" semitransparente que indica dónde apunta su invitado sin interferir en su trabajo.
- 💬 **Chat Integrado en Sesión**: Chat lateral deslizable en tiempo real durante la conexión.
- 📋 **Sincronización de Portapapeles (Clipboard)**: Copia texto en un equipo y pégalo al instante en el otro.
- 🛡️ **Elevación de Privilegios Remota**:
  - En Windows: Soporte de elevación de token UAC (`ShellExecuteW` con `runas`).
  - En Linux: Elevación mediante `pkexec` / PolicyKit.
- 🔒 **Seguridad Militar Zero-Knowledge**:
  - Claves criptográficas efímeras X25519 por sesión.
  - El servidor Relay jamás puede ver la pantalla, el audio, las teclas ni el chat.
  - Borrado seguro de claves en RAM con `zeroize`.
  - Hardening contra inyecciones y DLL Hijacking (`SetDefaultDllDirectories` / `PR_SET_DUMPABLE`).

---

## 📂 Estructura del Proyecto

```
FreeDesk/
├── Cargo.toml                  # Workspace raíz de Cargo
├── install.sh                  # Instalador one-line para Linux
├── .github/workflows/          # CI/CD automatizado para Windows y Linux
│   └── release.yml
├── crates/
│   ├── core/                   # Núcleo de alto rendimiento
│   │   ├── src/
│   │   │   ├── capture/        # Captura DXGI/GDI (Windows) y X11 (Linux)
│   │   │   ├── codec/          # Compresor LZ4 y codificador Tile-Delta 128×128
│   │   │   ├── crypto/         # X25519 + AES-256-GCM + Zeroize
│   │   │   ├── input/          # Win32 SendInput y Linux X11 XTest
│   │   │   ├── protocol/       # Protocolo binario de red y serialización
│   │   │   └── security/       # UAC / pkexec, Hardening, permisos y tokens
│   ├── relay/                  # Servidor de retransmisión Zero-Knowledge
│   │   └── src/main.rs         # Manejador de IDs de 9 dígitos y enrutamiento
│   └── app/                    # Aplicación cliente GUI (egui/eframe)
│       └── src/
│           ├── ui/             # Panel estilo AnyDesk, visor 60 FPS, chat y panel de control
│           └── main.rs         # Entry point multiplataforma
```

---

## 🎯 Licencia

Distribuido bajo la licencia **MIT**. Consulta `LICENSE` para más información. 100% Software Libre para uso personal y comercial.
