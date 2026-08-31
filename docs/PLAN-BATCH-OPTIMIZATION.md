# Plan de implementación — Modo Lote (optimización/conversión masiva)

Estado: **fases 1, 2 y 3 implementadas** (2026-08-28). Plan original: 2026-08-17.

## 0. Estado de implementación

Implementado: `src-tauri/src/batch/` (`mod.rs`, `ffmpeg_args.rs`, `runner.rs`,
`video_backend.rs`, `estimate.rs`, `checkpoint.rs`),
`FfmpegTools::encode` con progreso y cancelación, 16 comandos Tauri,
`UndoAction::ConvertMedia`, presets persistidos, y la UI completa
(`BatchPanel`, `BatchSelectGrid`, `BatchSettingsForm`, `BatchProgress`,
`BatchReplaceConfirmDialog`) con atajo Ctrl/⌘+B. La validación actual suma
71 tests Rust y 22 tests de frontend.

Añadido sobre el plan, tras revisar el flujo real "carpeta entera → carpeta nueva":

- `scan_folder_media(path, recursive, exclude_dirs, exclude_dir_names)`: coge
  una carpeta sin abrirla como álbum (no escribe `session.json` dentro).
  `exclude_dir_names` salta subcarpetas por nombre a cualquier profundidad, que
  es la única forma de excluir la subcarpeta de salida (`_optimized`).
- Los archivos que ya están dentro de la carpeta de salida se saltan, para no
  recomprimir el resultado de una pasada anterior.
- Reintento automático con AAC cuando `-c:a copy` falla (audio PCM de `.avi`,
  Vorbis de `.mkv`: no caben en MP4).
- `get_active_batch_job()` para reengancharse a un job tras recargar la ventana.
- La selección expande todos los paths del `MediaItem`, así una Live Photo
  (`.heic` + `.mov`) se convierte entera y no se descabala.

Fase 3 completa: AV1, aceleración por hardware, estimación previa, informes
`.md`/`.csv`, reenganche tras recargar la ventana y reanudación tras cerrar el
proceso. La aceleración detecta un dispositivo utilizable mediante una
codificación real de un frame, admite NVENC/QSV/AMF/VideoToolbox/VAAPI, limita
a uno los encodes GPU y aplica fallback tipado a CPU/AAC. El backend efectivo
y el motivo del fallback quedan en el progreso y el informe.

La estimación codifica hasta tres muestras de vídeo de tres segundos y cinco
imágenes completas, con los mismos ajustes y fallbacks del job. El checkpoint
se actualiza después de cada archivo terminal, conserva una generación previa
por si la escritura se interrumpe y, al arrancar, elimina el temporal incompleto
y reencola solo los archivos pendientes. La finalización y el registro de undo
son idempotentes para que un cierre entre ambos pasos no duplique la acción.

Las cadenas de argumentos **sí** se ejecutan contra un binario real:
`batch/ffmpeg_smoke.rs` codifica clips e imágenes de prueba con el ffmpeg que
encuentre, y CI lo instala en Linux y Windows. Los tests se saltan solos cuando
no hay ffmpeg, así que un checkout sin él sigue en verde.

Añadido después: ffmpeg no escribe EXIF al codificar imágenes, así que el
bloque se copia del original al convertido en `src-tauri/src/metadata.rs`
(JPEG/PNG/WebP; AVIF no puede y la UI lo avisa).

## 1. Objetivo

Añadir, sin tocar el flujo actual de organizar-uno-a-uno, un **modo lote**:

1. Seleccionar N archivos (vídeos y/o imágenes) de la cola actual o vía diálogo del sistema.
2. Configurar ajustes de optimización/conversión (códec/formato, calidad, resolución máx., audio, metadatos, salida).
3. Ejecutar todo con progreso por archivo, cancelación, y resumen de ahorro (MB antes → después).

Casos de uso guía:
- "Coger 200 vídeos de móvil y dejarlos en H.265 CRF 28 a 1080p → pesan 4x menos."
- "Coger 500 HEIC y convertir a JPEG/WebP de golpe."

## 2. Estado actual relevante

| Pieza | Ubicación | Nota para el lote |
|---|---|---|
| `FfmpegTools` (locate/probe/trim/remux/poster) | `src-tauri/src/video.rs` | Base a extender con encode + progreso. ffmpeg ya es requisito blando (`check_ffmpeg`). |
| `AppState` tras `Mutex` | `src-tauri/src/state.rs:23` | **Nunca** mantener este mutex bloqueado durante un encode largo: congelaría toda la UI. El lote necesita su propio estado. |
| Cola de un solo item | `FrontendState.item` (`models.rs:102`) | La selección múltiple no cabe aquí; se resuelve en el frontend + lista de rutas explícita al backend. |
| Escaneo/agrupado (live photos) | `src-tauri/src/media.rs` | Reutilizable para la rejilla de selección (`scan_folder`, `MediaItem`). |
| Preservar timestamps y mover seguro | `src-tauri/src/fs_util.rs` | Reutilizar `read_timestamps`/`apply_timestamps`/`move_file_preserve`. |
| Undo por sesión | `UndoAction` (`models.rs:178`) | Encaja para "reemplazar original" (backup → original). |
| Ajustes persistidos | `src-tauri/src/session.rs` (`load/save_app_settings`) | Donde guardar presets y últimos ajustes de lote. |
| i18n en/es | `src/lib/i18n.ts` | Toda cadena nueva en ambos idiomas. |
| Comandos expuestos | `src-tauri/src/lib.rs:31` | Registrar los nuevos aquí. |

Extensiones soportadas hoy: imágenes en `IMAGE_EXTENSIONS` (`media.rs:11`), vídeo `mp4, mov, m4v, avi, mkv, 3gp` (`media.rs:14`).

## 3. Decisiones de diseño (y por qué)

1. **Un solo motor: ffmpeg, también para imágenes.** Evita nuevas dependencias Rust (`image`, `libavif`, `libheif`) y mantiene un único punto de fallo/diagnóstico. Coste: HEIC/HEIF sólo se decodifica si el build de ffmpeg trae soporte → detectar y avisar (ver §8 riesgos).
2. **Estado de lote separado de `AppState`.** Nuevo `SharedBatchState` gestionado en `lib.rs`. Los encodes corren en hilos de trabajo; el mutex del lote sólo se toma para leer/actualizar contadores (milisegundos).
3. **La selección vive en el frontend.** El backend recibe siempre una lista explícita de rutas. Así el lote funciona con archivos de fuera de la carpeta abierta y no hay que añadir `selected_ids` a `AppState`.
4. **Por defecto, no destructivo.** Salida a subcarpeta (`_optimized/`) con nombre original. "Reemplazar originales" es opt-in, con **diálogo de confirmación explícito** (§7.1), backup + verificación previa + entrada de undo.
5. **Progreso por evento, no por polling.** `AppHandle::emit` con canales `batch://*`; el frontend escucha con `@tauri-apps/api/event`.
6. **Paralelismo conservador.** Por defecto `max(1, available_parallelism/2)`; x265/AV1 ya son multihilo internamente y saturar la CPU empeora el total. Configurable 1–8.

## 4. Modelo de datos (Rust + TS espejo)

Nuevo `src-tauri/src/batch/mod.rs` (o `batch.rs` si queda <400 líneas) con:

```rust
pub enum BatchMediaType { Video, Image }

pub enum VideoCodec { H264, H265, Av1, Copy }        // Copy = solo remux/faststart
pub enum ImageFormat { Jpeg, Webp, Avif, Png, Keep }
pub enum HardwareAcceleration {
    Auto, Software, Nvidia, Intel, Amd, VideoToolbox, Vaapi
}

pub struct VideoSettings {
    codec: VideoCodec,
    crf: u8,                    // 18–35, default 23 (h264) / 28 (h265) / 32 (av1)
    speed_preset: String,       // "medium" | "slow" | ... (svt-av1: 4–8)
    max_height: Option<u32>,    // 1080, 1440, 2160, None = original
    max_fps: Option<u32>,       // 30, 60, None
    audio: AudioSettings,       // Copy | Aac { bitrate_kbps } | Drop
    faststart: bool,            // default true
    keep_metadata: bool,        // default true (-map_metadata 0)
}

pub struct ImageSettings {
    format: ImageFormat,
    quality: u8,                // 1–100 → mapeado por formato
    max_edge: Option<u32>,      // lado largo, p.ej. 2560
    keep_metadata: bool,        // EXIF/orientación
}

pub enum OutputMode {
    Subfolder { name: String },     // default "_optimized"
    CustomFolder { path: String },
    ReplaceOriginal { backup: bool, confirmed: bool }, // backup default true; confirmed lo pone
                                                       // sólo el diálogo de §7.1 y el backend lo exige
}

pub struct BatchSettings {
    video: VideoSettings,
    image: ImageSettings,
    output: OutputMode,
    name_suffix: Option<String>,        // p.ej. "-opt"
    on_conflict: ConflictPolicy,        // Skip | Rename | Overwrite
    skip_if_larger: bool,               // default true: si sale más grande, descartar
    skip_if_savings_below_pct: Option<u8>, // default 5
    concurrency: usize,
    preserve_timestamps: bool,          // default true
}

pub struct BatchItemStatus {
    id: String, source_path: String, media_type: BatchMediaType,
    state: BatchItemState,   // Pending | Running | Done | Skipped | Failed | Cancelled
    progress: f32,           // 0..1 (vídeo: out_time/duration; imagen: 0 o 1)
    size_before: u64, size_after: Option<u64>,
    output_path: Option<String>, error: Option<String>,
}

pub struct BatchJobStatus {
    job_id: String, running: bool, cancelled: bool,
    total: usize, done: usize, failed: usize, skipped: usize,
    bytes_before: u64, bytes_after: u64,
    started_at: String, finished_at: Option<String>,
    items: Vec<BatchItemStatus>,
}

pub struct BatchPreset { id: String, name: String, settings: BatchSettings }
```

Espejo en `src/lib/types.ts` con los mismos nombres en `snake_case` (serde ya usa `rename_all = "snake_case"` para enums en este repo).

Persistencia: extender `AppSettings` (`models.rs:144`) con
`#[serde(default)] batch_presets: Vec<BatchPreset>` y `#[serde(default)] last_batch_settings: Option<BatchSettings>`.
Todos los campos con `#[serde(default)]` para no romper ajustes existentes.

## 5. Construcción de comandos ffmpeg

Módulo `batch/ffmpeg_args.rs`, funciones **puras** `Vec<String>` → testeables sin ffmpeg.

Vídeo (ejemplo H.265 1080p):
```
-y -i <in>
-map 0:v:0 -map 0:a? -map_metadata 0
-c:v libx265 -crf 28 -preset medium -tag:v hvc1
-vf scale=-2:'min(1080,ih)'      # sólo si max_height y ih > max_height
-r 30                            # sólo si max_fps
-c:a aac -b:a 128k               # o -c:a copy / -an
-movflags +faststart
-progress pipe:1 -nostats
<out>.mp4
```
- H.264: `-c:v libx264 -crf 23 -preset medium -pix_fmt yuv420p`.
- AV1: `-c:v libsvtav1 -crf 32 -preset 6 -svtav1-params tune=0`.
- `Copy`: `-c copy -movflags +faststart` (remux barato, ya probado en `remux_for_web_preview`).
- Contenedor de salida siempre `.mp4` salvo `Copy` sobre `.mkv` (mantener extensión).
- `scale=-2:` fuerza dimensiones pares (requisito de yuv420p).

Imagen:
- JPEG: `-q:v <2..31>` (mapa `quality` 100→2, 50→10, 1→31), `-pix_fmt yuvj420p`.
- WebP: `-c:v libwebp -quality <1..100> -compression_level 6`.
- AVIF: `-c:v libaom-av1 -crf <63-quality*0.63> -still-picture -cpu-used 6`.
- PNG: `-c:v png -compression_level 9` (sin calidad).
- Escalado: `-vf scale='if(gt(iw,ih),min(<max>,iw),-2)':'if(gt(iw,ih),-2,min(<max>,ih))'` (lado largo).
- Metadatos: `-map_metadata 0` vs `-map_metadata -1`. Nota: al reescalar se pierde el thumbnail EXIF; documentarlo.

Progreso: leer stdout línea a línea buscando `out_time_us=` y dividir por la duración de `probe_duration`. Requiere pasar de `Command::output()` a `Command::spawn()` + `BufReader` sobre stdout, y `child.kill()` para cancelar. En Windows añadir `.creation_flags(CREATE_NO_WINDOW)` (`std::os::windows::process::CommandExt`) para no abrir consolas — aplicarlo también a las llamadas existentes de `video.rs` de paso.

## 6. Runner y comandos Tauri

`batch/runner.rs`:
- `BatchRunner { jobs: HashMap<String, BatchJobStatus>, cancel_flags: HashMap<String, Arc<AtomicBool>>, active_job: Option<String> }`.
- `start(app: AppHandle, paths, settings) -> String (job_id)`: valida ffmpeg, clasifica cada ruta (imagen/vídeo por extensión), rellena `items` con `size_before`, guarda el job y lanza **un** hilo coordinador (`tauri::async_runtime::spawn_blocking`).
- Coordinador: pool de `concurrency` hilos consumiendo un canal `crossbeam`/`std::sync::mpsc` de índices. Un solo job activo a la vez (rechazar `start` si ya hay uno: error claro en UI).
- Por item: comprobar flag de cancelación → construir args → spawn ffmpeg a archivo temporal `.qmo-tmp-<n>.<ext>` en el directorio destino → parsear progreso (emitir como máx. 4 eventos/s por item) → al terminar:
  1. Verificar salida: existe, `len() > 0`, y para vídeo `probe_duration` OK.
  2. Aplicar `skip_if_larger` / `skip_if_savings_below_pct` → si no compensa, borrar temporal y marcar `Skipped` con razón.
  3. `preserve_timestamps` → `read_timestamps(src)` + `apply_timestamps(out)`.
  4. `ReplaceOriginal`: copiar original a `<carpeta>/.quick-media-organizer/batch-backups/<stem>_<stamp>.<ext>` (reutilizar patrón de `trim_backup_path`), borrar original, `fs::rename` temporal → original, y registrar par `{from: backup, to: original}` para undo. En cualquier fallo, restaurar desde backup (misma lógica de rollback que `trim_current_video`, `state.rs:474-490`).
  5. Otros modos: `fs::rename` temporal → nombre final aplicando `on_conflict`.
- Eventos: `batch://item` (BatchItemStatus), `batch://progress` (agregado ligero: done/total/bytes), `batch://done` (BatchJobStatus completo). Nombres de canal con prefijo para no colisionar.
- `cancel(job_id)`: activa el `AtomicBool`, mata los hijos vivos, borra temporales, marca pendientes como `Cancelled`. Los items ya terminados **no** se revierten (documentado en UI).

Comandos nuevos (`commands.rs` + registro en `lib.rs:31`), todos envueltos con `wrap(&log, ...)` como el resto:

| Comando | Firma | Uso |
|---|---|---|
| `list_queue_items` | `() -> Vec<MediaItem>` | Poblar la rejilla de selección desde la cola abierta (clona `state.items`). |
| `pick_media_files` | `() -> Vec<String>` | `dialog().file().add_filter(...).blocking_pick_files()`. |
| `pick_output_folder` | `() -> Option<String>` | Reutilizable con `pick_folder`. |
| `probe_batch_candidates` | `(paths) -> Vec<BatchCandidate>` | Tamaño, duración, resolución, códec; permite estimar y avisar de no soportados. |
| `start_batch_job` | `(paths, settings) -> String` | Arranca el job. |
| `cancel_batch_job` | `(job_id) -> ()` | Cancela. |
| `get_batch_job` | `(job_id) -> BatchJobStatus` | Rehidratar si la ventana se recarga / fallback sin eventos. |
| `get_batch_presets` | `() -> Vec<BatchPreset>` | Presets guardados + built-ins. |
| `save_batch_preset` / `delete_batch_preset` | `(preset)` / `(id)` | Persistir en `AppSettings`. |
| `ffmpeg_capabilities` | `() -> FfmpegCapabilities` | Parsear `ffmpeg -encoders` / `-decoders` una vez y cachear: qué códecs ofrecer y si hay HEIC. |

Capabilities: verificar que `dialog:default` incluye el permiso de apertura múltiple; si no, añadir `dialog:allow-open` a `src-tauri/capabilities/default.json`. Para "Abrir carpeta de salida" ya está `opener:default`.

## 7. Frontend

Componentes nuevos en `src/lib/components/`:
- `BatchPanel.svelte` — contenedor a pantalla completa (overlay sobre el workspace, mismo patrón que `OptionsPanel`), con 3 pasos: **Seleccionar → Ajustar → Ejecutar**.
- `BatchSelectGrid.svelte` — rejilla de miniaturas con checkbox, "seleccionar todo / ninguno / sólo vídeos / sólo imágenes", shift-click para rango, contador y suma de tamaño. Miniaturas: para imágenes `convertFileSrc` directo; para vídeo, el poster de `resolve_video_preview` (ya existe cache) o placeholder por extensión para no disparar remuxes masivos.
- `BatchSettingsForm.svelte` — dos pestañas (Vídeo / Imagen) sólo con la relevante según la selección, presets built-in ("Ahorro máximo 1080p H.265", "Equilibrado H.264", "Solo remux rápido", "HEIC→JPEG 90", "Web WebP 2560px") + guardar preset propio.
- `BatchProgress.svelte` — barra global, lista virtualizada por item (estado, %, antes→después), botón Cancelar, y al terminar: resumen (`X archivos, 12,4 GB → 3,1 GB, −75 %`), lista de fallos copiable, "Abrir carpeta de salida".

Integración:
- `src/routes/+page.svelte`: `let showBatch = $state(false)`; atajo **Ctrl/⌘+B** (registrar en `shortcuts.ts`, `ShortcutBar.svelte` y `HelpOverlay.svelte`); entrada también en `OptionsPanel` y en la pantalla de "sesión completada" ("Optimizar esta carpeta").
- Mientras el lote corre: bloquear acciones de renombrado/borrado que toquen archivos del job (`workspaceDisabled` ya centraliza esto) y avisar al cerrar el panel de que el job sigue en segundo plano.
- `listen("batch://item")` en `onMount` del panel, con `unlisten` en cleanup; `get_batch_job` al montar para reengancharse a un job en curso.
- Errores: usar `invokeLogged`/`reportError` como el resto.
- i18n: nuevo bloque `batch.*` en `en` y `es` (~70 claves, incluidas las del diálogo de §7.1).

### 7.1 Diálogo de confirmación de "Reemplazar originales"

Activar el toggle **no** cambia el ajuste directamente: abre un diálogo modal y `output.mode` sólo pasa a `ReplaceOriginal` si el usuario confirma. Al cancelar, el toggle vuelve visualmente a su estado previo (patrón: `pendingOutputMode` + `showReplaceConfirm` en `$state`, nunca `bind:checked` directo sobre el settings).

Componente `BatchReplaceConfirmDialog.svelte` (mismo esqueleto que `OptionsPanel.svelte`: backdrop + `role="dialog" aria-modal="true"`, `Esc` cierra = cancelar, foco inicial en **Cancelar**, no en el botón destructivo). No usar `dialog().message()` del plugin nativo: hace falta contenido con lista y checkbox.

Contenido (todo vía i18n `batch.replaceConfirm.*`):
- Título: "¿Reemplazar los archivos originales?" / "Replace the original files?"
- Resumen dinámico: "N archivos, X,X GB serán sustituidos por su versión optimizada."
- Implicaciones, en lista explícita:
  1. Cada original se copia a `<carpeta>/.quick-media-organizer/batch-backups/` **antes** de sustituirlo.
  2. Los backups no se borran solos y ocupan espacio hasta que los elimines a mano (mostrar la ruta exacta, seleccionable).
  3. Un archivo sólo se sustituye si la salida se verifica correcta; si algo falla, se restaura el original.
  4. La conversión **con re-encode pierde calidad** de forma irreversible respecto al original (a diferencia del recorte sin pérdidas que ya hace la app).
  5. Deshacer (`Ctrl+Z`) recupera los archivos desde el backup, pero sólo durante esta sesión y en el orden inverso.
  6. Si cambia la extensión (p. ej. `.avi` → `.mp4`, `.heic` → `.jpg`), el nombre del archivo en la cola cambia y la posición de sesión puede reiniciarse.
- Checkbox obligatorio: "Entiendo que los archivos originales se sustituyen" → habilita el botón de confirmar (evita el clic reflejo en un `Enter`).
- Botones: **Cancelar** (primario visual) y **Reemplazar originales** (estilo destructivo, `disabled` hasta marcar el checkbox).
- No hay "no volver a preguntar": el diálogo aparece cada vez que se activa el modo, incluso si venía de un preset guardado o de `last_batch_settings` (al rehidratar ajustes con `ReplaceOriginal`, degradar a `Subfolder` y exigir confirmación de nuevo).

Segunda barrera en el momento de ejecutar: si al pulsar **Ejecutar** el modo es `ReplaceOriginal`, el botón muestra el texto destructivo ("Optimizar y reemplazar N archivos") y `start_batch_job` valida en el backend que el frontend haya enviado `output.mode.replace_original.confirmed = true`; si no, devuelve error. Así una llamada al comando sin pasar por la UI no puede borrar originales por accidente.

## 8. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| HEIC no decodificable por el ffmpeg del usuario | `ffmpeg_capabilities` detecta el decoder; si falta, marcar esos items como no soportados **antes** de arrancar y mostrar cómo instalar un ffmpeg completo. |
| Pérdida de datos al reemplazar originales | Opt-in + backup + verificación de salida (probe) + rollback + entrada de undo. Nunca borrar el original antes de validar el temporal. |
| Bloqueo de UI por mutex | Estado de lote independiente; jamás llamar a ffmpeg con el lock de `AppState` tomado. |
| Recalentar/saturar la máquina | Concurrencia por defecto = mitad de cores; documentar que x265/AV1 ya son multihilo. |
| Job huérfano tras recarga o cierre del proceso | `get_batch_job` para recarga de ventana y checkpoint en disco por archivo terminal para reanudar tras abrir la aplicación. |
| Rutas con caracteres raros / Unicode en Windows | Pasar `Path` a `Command::arg` (no interpolar strings), como ya hace `video.rs`. |
| Consolas negras en Windows | `CREATE_NO_WINDOW` en todos los spawns. |
| Sesión/undo desincronizados si se reemplazan archivos de la cola | Tras el job, refrescar tamaños (`refresh_item_size`) y re-escanear si `ReplaceOriginal` cambió extensiones; si cambian nombres, invalidar posición de sesión igual que hace `open_folder`. |

## 9. Fases de entrega

**Fase 1 — Núcleo vídeo (MVP útil).**
`batch/ffmpeg_args.rs` + `batch/runner.rs` con H.264/H.265/Copy, CRF, `max_height`, audio aac/copy, salida a subcarpeta, progreso, cancelación. Comandos `start/cancel/get`, `list_queue_items`, `pick_media_files`. UI: `BatchPanel` con rejilla básica (sin miniaturas de vídeo), formulario mínimo, progreso. i18n en/es.

**Fase 2 — Imágenes + presets + reemplazo seguro.**
JPEG/WebP/AVIF/PNG, `max_edge`, metadatos; `OutputMode::ReplaceOriginal` con backup+undo y el diálogo de confirmación de §7.1 (entra en la misma fase que el modo destructivo, no después); presets built-in y guardados; `skip_if_larger`; `ffmpeg_capabilities` y aviso HEIC; miniaturas en la rejilla; resumen de ahorro.

**Fase 3 — Pulido.**
Completada: AV1; NVENC, QSV, AMF, VideoToolbox y VAAPI con detección real y
fallback a software; estimación previa con muestras de tres segundos; informes
`.md`/`.csv` en `logs/`; checkpoint y reanudación del job; README en inglés y
español; capturas regeneradas con `scripts/capture-screenshots.mjs`.

## 10. Tests y QA

Unitarios Rust (sin ffmpeg, siguiendo el patrón de `media.rs:436`):
- `ffmpeg_args`: cada combinación de settings produce los flags esperados; sin `-vf` cuando no hay límite; extensión de salida correcta; mapeo `quality` → `-q:v`/`-quality`/`-crf`.
- Resolución de nombres de salida: sufijo, colisiones con `Skip|Rename|Overwrite`, no pisar el origen cuando destino == origen.
- Clasificación de rutas (vídeo/imagen/no soportado) y filtrado de `_deleted` / carpeta `.quick-media-organizer`.
- Gate de confirmación: `ReplaceOriginal { confirmed: false }` rechazado en `start_batch_job` antes de crear cualquier archivo o backup.
- Runner con un "encoder" inyectado (trait `Encoder` mockeado): progreso agregado, cancelación deja pendientes en `Cancelled`, `skip_if_larger` descarta, rollback restaura el original.

Validación de cierre:

- [x] Job real con fuentes MP4, MOV y AVI; todos los resultados se vuelven a
  abrir con ffprobe.
- [x] Cancelación durante un encode x265 real; el proceso termina, no quedan
  temporales y el original permanece byte a byte.
- [x] `ReplaceOriginal` y undo; restaura bytes y fecha de modificación. También
  se cubren colisiones, rollback y finalización idempotente.
- [x] Barreras del diálogo y del backend: sin confirmación no se crea ningún
  archivo; los presets destructivos pierden `confirmed` al persistirse.
- [x] 100 HEIC reales a JPEG en un solo job mediante el smoke test opcional
  `QMO_HEIC_TEST_FILE`; el gate de capacidades cubre el caso sin decoder.
- [x] FFmpeg ausente devuelve capacidades no disponibles y la UI desactiva el
  arranque con un mensaje específico.
- [x] Rutas con acentos, emoji y rutas Windows de más de 260 caracteres.
- [x] `pnpm test`, `pnpm check`, `pnpm build:web`, `cargo test` y
  `cargo clippy --all-targets -- -D warnings` en verde.

## 11. Fuera de alcance (por ahora)

Recorte/rotación por lote, watermarks, subida a la nube, integración de libav
dentro del proceso y colas programadas. La edición Standard ya distribuye
FFmpeg como ejecutable separado; la Lite usa la instalación del sistema.

## 12. Release

Al terminar cada fase, seguir `.cursor/rules/git-release.mdc`: subir patch en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`, commit `Release vX.Y.Z: …`, push a `origin/main`, tag `vX.Y.Z` para disparar `release.yml`.
