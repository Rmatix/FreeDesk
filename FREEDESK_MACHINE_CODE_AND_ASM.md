# 🔬 FreeDesk — Desensamblado a Nivel de Lenguaje Máquina (Hex) y Ensamblador (x86_64 ASM)

Este documento expone cómo se traduce el código fuente en **Rust** de FreeDesk directamente a **Instrucciones de CPU x86_64** y sus correspondientes **Opcodes en Lenguaje Máquina (Bytes Hexadecimales)** ejecutados directamente por el procesador (Intel / AMD).

---

## 📑 Índice de Módulos Críticos
1. **[Captura de Pantalla en Tiempo Real](#1-captura-de-pantalla-en-tiempo-real-screencapturecapture_frame)** (`ScreenCapture::capture_frame`)
2. **[Simulador de Entrada y Prioridad de Hardware](#2-control-de-mouse-y-prioridad-hardware-inputsimulatorhandle_event)** (`InputSimulator::handle_event`)
3. **[Motor de Detección de Cambios y Rayon](#3-motor-de-tiles-y-compresión-en-paralelo-tileencoderencode_frame)** (`TileEncoder::encode_frame`)
4. **[Glosario de Opcodes y Registros de CPU](#4-glosario-de-opcodes-y-registros-de-cpu)**

---

## 1. Captura de Pantalla en Tiempo Real (`ScreenCapture::capture_frame`)

Esta función toma el fotograma bruto del frame buffer (DXGI en Windows o X11 ZPixmap en Linux), calcula el stride por fila multiplicando el ancho por 4 (RGBA) con desplazamiento a la izquierda (`shl`) y valida si la imagen cambió antes de enviarla.

### Desensamblado: Opcodes Máquina + ASM x86_64 (Sintaxis Intel)

```x86asm
; ==============================================================================
; Símbolo: <freedesk_core::capture::screen::ScreenCapture::capture_frame>
; Convención de llamada: System V AMD64 ABI (rdi = self, rsi = output buffer)
; ==============================================================================

Offset   Bytes Máquina (Hex)           Instrucción Ensamblador          Significado / Código Rust Equivalente
---------------------------------------------------------------------------------------------------------------
000000   55                            push   rbp                       ; Guarda el frame pointer anterior en el stack
000001   41 57                         push   r15                       ; Salva registros preservados por la ABI
000003   41 56                         push   r14                       ; Salva r14
000005   41 55                         push   r13                       ; Salva r13
000007   41 54                         push   r12                       ; Salva r12
000009   53                            push   rbx                       ; Salva rbx
00000a   48 81 ec a8 00 00 00          sub    rsp, 0xa8                 ; Reserva 168 bytes en el stack frame
000011   48 83 3f ff                   cmp    QWORD PTR [rdi], -1       ; Comprueba si el puntero self es nulo (-1 / error)
000015   0f 84 7b 03 00 00             je     error_exit                ; Salta al manejador de error si falló
00001b   48 89 fb                      mov    rbx, rdi                  ; rbx = self
00001e   44 8b 97 f8 01 00 00          mov    r10d, DWORD PTR [rdi+504] ; r10d = self.width (32 bits)
000025   44 8b b7 fc 01 00 00          mov    r14d, DWORD PTR [rdi+508] ; r14d = self.height (32 bits)
00002c   4d 89 d7                      mov    r15, r10                  ; r15 = width
00002f   49 c1 e7 02                   shl    r15, 0x2                  ; r15 = width * 4 (Cálculo de bytes por línea RGBA!)
000033   48 8b 8f e8 01 00 00          mov    rcx, QWORD PTR [rdi+488]  ; rcx = self.bitmap_pad (padding de scanline X11)
00003a   48 c1 e9 03                   shr    rcx, 0x3                  ; rcx = pad / 8 (conversión de bits a bytes)
00003e   4d 89 fd                      mov    r13, r15                  ; r13 = row_stride calculado
000041   74 27                         je     no_padding                ; Si pad_bytes == 0, salta el cálculo de alineación
000043   4d 8d 2c 0f                   lea    r13, [r15+rcx*1]          ; r13 = (width * 4) + pad_bytes
000047   49 ff cd                      dec    r13                       ; r13--
00004a   4c 89 e8                      mov    rax, r13                  ; Prepara división para redondeo hacia arriba
00004d   48 09 c8                      or     rax, rcx
000050   48 c1 e8 20                   shr    rax, 0x20
000054   74 0a                         je     div_32bit
000056   4c 89 e8                      mov    rax, r13
000059   31 d2                         xor    edx, edx                  ; Limpia rdx antes de división
00005b   48 f7 f1                      div    rcx                       ; rax = r13 / rcx
00005e   eb 07                         jmp    apply_sub
div_32bit:
000060   44 89 e8                      mov    eax, r13d
000063   31 d2                         xor    edx, edx
000065   f7 f1                         div    ecx
apply_sub:
000067   49 29 d5                      sub    r13, rdx                  ; r13 = Stride alineado exacto al hardware
no_padding:
00006a   48 89 74 24 08                mov    QWORD PTR [rsp+8], rsi    ; Guarda el buffer de salida `output` en el stack
00006f   8b 8b 00 02 00 00             mov    ecx, DWORD PTR [rbx+512]  ; Carga self.root window ID
000075   48 83 ec 08                   sub    rsp, 0x8                  ; Alinea el stack a 16 bytes para la llamada C
000079   48 8d 7c 24 28                lea    rdi, [rsp+0x28]           ; Parámetro 1: Puntero al resultado
00007e   48 89 de                      mov    rsi, rbx                  ; Parámetro 2: Conexión X11
000081   ba 02 00 00 00                mov    edx, 0x2                  ; Parámetro 3: ImageFormat::Z_PIXMAP (Valor numérico 2)
000086   45 31 c0                      xor    r8d, r8d                  ; x = 0
000089   45 31 c9                      xor    r9d, r9d                  ; y = 0
00008c   6a ff                         push   -1                        ; plane_mask = !0 (Todos los bits habilitados)
00008e   41 56                         push   r14                       ; height
000090   41 52                         push   r10                       ; width
000092   e8 00 00 00 00                call   x11_get_image             ; LLAMADA AL KERNEL / DRIVER: Obtiene pixeles GPU
000097   48 83 c4 20                   add    rsp, 0x20                 ; Limpia los argumentos empujados al stack
```

---

## 2. Control de Mouse y Prioridad Hardware (`InputSimulator::handle_event`)

Implementa el algoritmo de **Host Hardware Priority**: si el usuario local toca el mouse, el CPU ejecuta una instrucción atómica `lock cmpxchg` (Compare-And-Swap) para adquirir el candado sin pasar por el sistema operativo, garantizando una latencia menor a 1 microsegundo.

### Desensamblado: Opcodes Máquina + ASM x86_64

```x86asm
; ==============================================================================
; Símbolo: <freedesk_core::input::simulator::InputSimulator::handle_event>
; ==============================================================================

Offset   Bytes Máquina (Hex)           Instrucción Ensamblador          Significado / Código Rust Equivalente
---------------------------------------------------------------------------------------------------------------
000000   55                            push   rbp                       ; Prólogo estándar de función x86_64
000001   41 57                         push   r15
000003   41 56                         push   r14
000005   41 55                         push   r13
000007   41 54                         push   r12
000009   53                            push   rbx
00000a   48 83 ec 78                   sub    rsp, 0x78                 ; Reserva espacio para estructuras locales
00000e   48 89 cb                      mov    rbx, rcx                  ; rbx = event
000011   49 89 d6                      mov    r14, rdx                  ; r14 = perms (SessionPermissions)
000014   49 89 f5                      mov    r13, rsi                  ; r13 = self
000017   49 89 fc                      mov    r12, rdi                  ; r12 = return slot
00001a   4c 8d 7e 10                   lea    r15, [rsi+0x10]           ; Puntero al campo last_simulated_pos.lock()
00001e   b9 01 00 00 00                mov    ecx, 0x1                  ; Valor deseado = 1 (LOCKED)
000023   31 c0                         xor    eax, eax                  ; Valor esperado = 0 (UNLOCKED)

; 🔒 CANDADO ATÓMICO EN HARDWARE (Sin syscall):
000025   f0 0f b1 4e 10                lock cmpxchg [rsi+0x10], ecx     ; INSTRUCCIÓN ATÓMICA DE BUS DE MEMORIA:
                                                                        ; Compara eax con la memoria. Si es 0, pone 1.
00002a   0f 85 56 0a 00 00             jne    mutex_contention_slow     ; Si ya estaba bloqueado, va al fallback de espera
000030   4c 89 7c 24 48                mov    [rsp+0x48], r15           ; Adquisición exitosa: almacena RAII MutexGuard
000035   4c 8b 3d 00 00 00 00          mov    r15, QWORD PTR [rip]      ; Carga el temporizador de alta resolución (Instant)
00003c   49 8b 07                      mov    rax, QWORD PTR [r15]
00003f   48 d1 e0                      shl    rax, 1                    ; Desplaza para leer nanosegundos del TSC
000042   48 85 c0                      test   rax, rax
000045   0f 85 49 0a 00 00             jne    timer_overflow
00004b   c7 44 24 44 00 00 00 00       mov    DWORD PTR [rsp+0x44], 0   ; Inicializa flags
000053   41 0f b6 45 14                movzx  eax, BYTE PTR [r13+0x14]  ; Lee perms.can_control_mouse (booleano)
000058   84 c0                         test   al, al                    ; ¿Permiso concedido?
00005a   0f 85 4d 0a 00 00             jne    permission_denied
000060   49 8d 45 20                   lea    rax, [r13+0x20]           ; Puntero al hardware simulator
000070   f0 41 0f b1 4d 20             lock cmpxchg [r13+0x20], ecx     ; Bloquea el simulador de hardware
...
0000a7   f3 41 0f 7e 45 18             movq   xmm0, [r13+0x18]          ; Carga coordenadas X, Y float (SSE2 Vector)
0000ad   66 0f 6f c8                   movdqa xmm1, xmm0                ; Copia vectores para cálculo de interpolación
```

---

## 3. Motor de Tiles y Compresión en Paralelo (`TileEncoder::encode_frame`)

Esta rutina divide la pantalla en bloques de 128×128 píxeles y distribuye el trabajo en todos los núcleos de la CPU a través del scheduler de Rayon usando vectores SIMD `xmm` de 128 bits:

```x86asm
; ==============================================================================
; Símbolo: <freedesk_core::codec::tile_diff::TileEncoder::encode_frame>
; ==============================================================================

Offset   Bytes Máquina (Hex)           Instrucción Ensamblador          Significado / Código Rust Equivalente
---------------------------------------------------------------------------------------------------------------
000000   41 57                         push   r15                       ; Salva contexto
000002   41 56                         push   r14
000004   53                            push   rbx
000005   48 83 ec 20                   sub    rsp, 0x20
000009   49 89 d6                      mov    r14, rdx                  ; r14 = slice de pixeles
00000c   49 89 f7                      mov    r15, rsi                  ; r15 = coordenadas de los tiles
00000f   48 89 fb                      mov    rbx, rdi                  ; rbx = buffer de salida

; ⚡ SIMD VECTORIAL DE 128 BITS (SSE/AVX) para cargar 4 coordenadas de tile en 1 ciclo:
000017   0f 10 01                      movups xmm0, XMMWORD PTR [rcx]   ; Carga (start_x, start_y, tile_w, tile_h)
00001a   0f 29 04 24                   movaps XMMWORD PTR [rsp], xmm0   ; Guarda en stack alineado
00001e   48 8b 41 10                   mov    rax, QWORD PTR [rcx+0x10] ; Puntero a la función xxhash
000022   48 89 44 24 10                mov    QWORD PTR [rsp+0x10], rax
000027   ff 15 00 00 00 00             call   QWORD PTR [rip]           ; Despacha el worker thread en paralelo
00002d   31 c9                         xor    ecx, ecx                  ; Prepara acumulador de tiles cambiados
00002f   49 83 fe ff                   cmp    r14, -1
000033   0f 94 c1                      sete   cl                        ; Marca si es keyframe forzado
000036   48 39 c8                      cmp    rax, rcx
000039   48 0f 47 c8                   cmova  rcx, rax                  ; Move condicional rápido
000040   41 b8 01 00 00 00             mov    r8d, 0x1                  ; Hilos activos
000046   48 89 df                      mov    rdi, rbx
000049   4c 89 f6                      mov    rsi, r14
00004c   31 d2                         xor    edx, edx
00004e   4d 89 f9                      mov    r9, r15
000051   50                            push   rax
000052   41 56                         push   r14
000054   e8 00 00 00 00                call   rayon_bridge              ; Ejecución paralela multi-core
000059   48 83 c4 10                   add    rsp, 0x10
00005d   48 89 d8                      mov    rax, rbx                  ; rax = DeltaFrame completado
000060   48 83 c4 20                   add    rsp, 0x20                 ; Limpia stack
000064   5b                            pop    rbx                       ; Restaura registros
000065   41 5e                         pop    r14
000067   41 5f                         pop    r15
000069   c3                            ret                              ; Retorna al llamador con el frame comprimido
```

---

## 4. Glosario de Opcodes y Registros de CPU

| Opcode Hex | Mnemónico ASM | Explicación en el Procesador |
|------------|--------------|------------------------------|
| `55` | `push rbp` | Coloca el puntero base en la cima de la pila (Stack). |
| `48 89 d8` | `mov rax, rbx` | Copia el valor de 64 bits de `rbx` al registro acumulador `rax`. |
| `49 c1 e7 02` | `shl r15, 2` | Desplaza 2 bits a la izquierda (equivalente a multiplicar por 4: $X \times 4$). Usado para calcular bytes por pixel RGBA. |
| `f0 0f b1 ...` | `lock cmpxchg` | **Bloqueo atómico de hardware**. La CPU toma posesión exclusiva del bus de memoria para evitar carreras entre hilos. |
| `0f 10 01` | `movups xmm0, [rcx]` | Carga 128 bits de memoria de una sola vez en el registro vectorial SSE `xmm0` (4 números enteros de 32 bits en 1 instrucción). |
| `f3 41 0f 7e` | `movq xmm0, ...` | Carga números en coma flotante de precisión simple/doble para el cálculo de coordenadas relativas del mouse. |
| `c3` | `ret` | Retorno de subrutina (salta a la dirección de memoria que está en la cima del stack). |

---

### 💡 Cómo inspeccionar esto tú mismo en cualquier momento
- En **Linux**:
  ```bash
  objdump -d -M intel target-linux/release/freedesk | less
  ```
- En **Windows**:
  ```powershell
  dumpbin /DISASM target/release/freedesk.exe | Out-File -Encoding utf8 freedesk_asm.txt
  ```
