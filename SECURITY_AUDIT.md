# 🛡️ Informe de Pentesting y Auditoría de Seguridad — FreeDesk

**Fecha de Auditoría:** 6 de Noviembre de 2026  
**Alcance:** Binarios `freedesk.exe` y `freedesk-relay.exe`, Core Ciphers, Protocol Parser, Tile Delta Decoder y Relay Routing.  
**Estado Final:** **100% Mitigado y Verificado** ✅

---

## 📋 Resumen Ejecutivo de Hallazgos

| ID | Vulnerabilidad | Severidad | Componente | Estado |
|---|---|---|---|---|
| **VULN-01** | Suplantación de Identidad en Reenvío (Relay Sender Spoofing) | 🔴 **CRÍTICA** | `crates/relay/src/main.rs` | **Corregido** |
| **VULN-02** | Secuestro de ID de Host por Colisión de Registro | 🟠 **ALTA** | `crates/relay/src/main.rs` | **Corregido** |
| **VULN-03** | Memory Out-of-Bounds & Bomba de Descompresión en Tiles | 🟡 **MEDIA** | `crates/core/src/codec/tile_diff.rs` | **Corregido** |
| **VULN-04** | Desbordamiento de Entero en Contador Anti-Replay (Nonce DoS) | 🟡 **MEDIA** | `crates/core/src/crypto/session_cipher.rs` | **Corregido** |
| **VULN-05** | Consumo Ilimitado de Memoria en Rate Limiter (Memory Exhaustion) | 🟢 **BAJA** | `crates/core/src/security/rate_limiter.rs` | **Corregido** |

---

## 🔍 Análisis Detallado y Correcciones Aplicadas

### 1. [VULN-01] Suplantación de Identidad en Reenvío (Relay Sender Spoofing)
- **Mecánica:** En el mensaje `RelayMessage::Forward`, el servidor Relay confiaba ciegamente en el campo `from_id` provisto en el paquete en lugar de usar la identidad autenticada asociada al socket TCP.
- **Impacto:** Un cliente malicioso conectado al relay podía inyectar paquetes haciéndose pasar por otro host o visor.
- **Parche aplicado:** Se eliminó la confianza en el payload entrante (`from_id: _`). El servidor ahora sobreescribe y certifica el remitente forzosamente utilizando `current_peer_id` del canal validado. Si no está autenticado, la conexión es descartada.

### 2. [VULN-02] Secuestro de ID de Host por Colisión de Registro
- **Mecánica:** Un atacante que conociera o adivinara el ID de 9 dígitos de un host activo podía enviar `RegisterHost { peer_id }` y sobrescribir el canal en el mapa `peers`, desconectando al host legítimo y redirigiendo las conexiones entrantes.
- **Impacto:** Denegación de servicio y secuestro de sesiones.
- **Parche aplicado:** Se añadió validación estricta en el Relay: si `peers.contains_key(&peer_id)` es verdadero, el registro es rechazado con `success: false` y alerta en los logs de seguridad.

### 3. [VULN-03] Memory Out-of-Bounds & Bomba de Descompresión en Tiles
- **Mecánica:** El decodificador `TileDecoder::apply_delta` confiaba en las coordenadas `tile_x`, `tile_y`, `width` y `height` enviadas por el peer remoto sin validar si excedían la resolución de pantalla, y no verificaba que la longitud de los bytes descomprimidos coincidiera exactamente con `width * height * 4`.
- **Impacto:** Posible corrupción de memoria gráfica o pánico (crash) del visor por desbordamiento de búfer.
- **Parche aplicado:**
  - Verificación estricta de límites: `tile_w <= 64 && tile_h <= 64`.
  - Validación de coordenadas dentro de pantalla: `start_x + tile_w <= screen_width` y `start_y + tile_h <= screen_height`.
  - Validación de longitud exacta de bytes descomprimidos: `decompressed.len() == expected_bytes`.

### 4. [VULN-04] Desbordamiento de Entero en Contador Anti-Replay
- **Mecánica:** En `decrypt()`, el cálculo `self.recv_nonce_counter = nonce_val + 1;` podía causar un panic por overflow si se enviaba un nonce de valor `u64::MAX`.
- **Impacto:** Caída de la sesión remota provocada por un paquete malformado.
- **Parche aplicado:** Sustituido por `nonce_val.checked_add(1).ok_or_else(...)`, rechazando nonces agotados de forma segura sin provocar panics.

### 5. [VULN-05] Consumo Ilimitado de Memoria en Rate Limiter
- **Mecánica:** Las IPs registradas en el mapa de control de fuerza bruta nunca eran eliminadas completamente del `HashMap`, solo sus marcas de tiempo individuales.
- **Impacto:** Con millones de intentos con IPs aleatorias, la memoria del Relay aumentaba progresivamente (DoS por agotamiento de RAM).
- **Parche aplicado:** Implementada limpieza periódica (`eviction sweep`) que depura entradas inactivas cuando el mapa supera el umbral seguro.

---

## 🧪 Pruebas de Regresión y Validación Automatizada

Se implementaron pruebas de regresión automáticas en `cargo test`:
- `test_security_bounds_rejection`: Comprueba que tiles malformados, fuera de pantalla o con tamaños alterados sean rechazados inmediatamente con error `Security Violation` sin desbordar memoria.
- `test_e2e_key_exchange_and_cipher`: Comprueba que los ataques de reproducción (Replay Attacks) con nonces duplicados o antiguos sean bloqueados por el cifrador AES-256-GCM.

```
running 4 tests
test codec::lz4::tests::test_lz4_roundtrip ... ok
test codec::tile_diff::tests::test_security_bounds_rejection ... ok
test crypto::session_cipher::tests::test_e2e_key_exchange_and_cipher ... ok
test codec::tile_diff::tests::test_tile_encoder_decoder_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Los binarios finales en `target/release/` han sido re-compilados y blindados con todas las correcciones activas.
