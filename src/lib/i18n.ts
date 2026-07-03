import type { Locale } from "./types";

export type { Locale };

const messages = {
  en: {
    appTitle: "Quick Media Organizer",
    welcomeTitle: "Organize thousands of photos and videos with your keyboard",
    welcomeStep1: "Pick a folder full of phone media (photos, videos, IMG_1234…)",
    welcomeStep2: "Use Enter, Ctrl+F, Ctrl+D and ⌘⇧Space / Ctrl+Space — shortcuts stay visible at all times",
    welcomeStep3: "Rename in place or save into folders like gym/, trips/portugal/ or paperwork/",
    welcomeStart: "Got it, let's start",
    welcomeDontShow: "Don't show again",
    openFolder: "Open folder",
    noFolder: "Choose a folder to begin organizing your media.",
    nameLabel: "Name",
    namePlaceholder: "Type a descriptive name…",
    armedFolder: "Save to",
    armedBanner: "Folder mode: {folder} — Esc to cancel",
    undoHint: "{key} to undo",
    livePhoto: "Live Photo",
    progress: "{current} / {total}",
    emptyQueue: "All done! Every file in this folder has been processed.",
    sessionReachedEnd: "You've reached the last file in this folder.",
    restartQueue: "Start over",
    continueReviewing: "Stay on last file",
    sessionStats:
      "{renamed} renamed · {trashed} deleted · {moved} moved · {skipped} skipped",
    writeName: "Write a name before pressing Enter.",
    armedMoveEmpty: "Saving to folder with original filename.",
    chooseFolder: "Choose or type a folder path.",
    startupError: "Could not start the app. Check the error log.",
    resumingFolder: "Resuming last folder…",
    sessionPositionReset:
      "Could not find the last file you were on (it may have been renamed). Starting from the beginning.",
    sessionResumeBanner: "Resuming at photo {current} of {total}.",
    sessionResumeContinue: "Continue here",
    subfolderMediaNotice:
      "{count} more photos/videos are in subfolders. Enable “Include subfolders” in Options (Ctrl+O) to review them.",
    common: {
      ok: "OK",
      cancel: "Cancel",
      close: "Close",
    },
    renamed: "Renamed",
    trashed: "Moved to _deleted",
    savedFolder: "Saved to folder",
    undone: "Undone",
    shortcuts: {
      enter: "Save",
      enterTrimSave: "Trim + save",
      folder: "Folder",
      delete: "Delete",
      space: "Skip",
      nav: "Navigate",
      prev: "Previous",
      next: "Next",
      undo: "Undo",
      info: "Info",
      options: "Options",
      help: "Help",
    },
    hints: {
      enter: "Enter save",
      enterTrimSave: "Enter trim + save",
      folder: "Ctrl+F folder",
      delete: "Ctrl+D delete",
    },
    folderPicker: {
      title: "Save to folder",
      search: "Search or create…",
      favorites: "Favorites",
      recent: "Recent",
      existing: "Existing folders",
      confirm: "Confirm",
      cancel: "Cancel",
      createHint: "Use paths like trips/portugal/2024",
      discard: "Discard the folder path you typed?",
      keyboardHint: "Type to filter · Tab to complete · Enter to confirm · ↑↓ to browse",
      matches: "Matching folders",
      noMatches: "No matching folders — Enter creates a new path",
    },
    metadata: {
      toggle: "Press {key} for details",
      file: "File",
      date: "Date",
      size: "Size",
      dimensions: "Dimensions",
    },
    options: {
      title: "Options",
      layout: "Layout",
      layoutSidebar: "Preview left, controls right (portrait)",
      layoutBottom: "Preview top, controls below",
      sort: "Sort order",
      sortExif: "Capture date (EXIF)",
      sortName: "File name",
      sortModified: "Modified date",
      recursive: "Include subfolders",
      recursiveHint: "Re-scans all subfolders and may change queue order.",
      videoSound: "Play videos with sound",
      videoSoundHint: "Autoplay with audio when reviewing clips.",
      renameMode: "Rename mode",
      renameFree: "Free names with auto counter",
      renamePrefix: "Prefix + counter",
      language: "Language",
      errorLog: "Error log",
      errorLogHint: "Saved for debugging. Cleared after fixes.",
      errorLogPath: "Log file",
    },
    help: {
      title: "Keyboard shortcuts",
      faqDeleted: "Ctrl+D moves files to _deleted/ inside the album (not the system Trash). Use Ctrl+Z to undo.",
      faqDates: "Original EXIF dates and file timestamps are preserved.",
      modifierHint: "On Mac use ⌘ instead of Ctrl. Modifier shortcuts work while typing a name.",
    },
    preview: {
      preparing: "Preparing preview…",
      unsupportedFormat:
        "This format cannot play in the built-in preview (common with MKV and some AVI files).",
      proxyHint: "Compatible preview copy. The original file is unchanged.",
      openExternal: "Open in default app",
      imageFailed: "This file could not be previewed.",
      imageEmpty: "The file is empty (0 bytes). It may be a failed download.",
      imageTooSmall: "The file is too small to be a valid image.",
      imageContentMismatch:
        "The .jpg extension does not match the file contents. It may be a broken download or cache placeholder.",
      imageUnknown:
        "The built-in preview could not decode this image. Try opening it in Preview or Photos.",
    },
    trim: {
      title: "Trim video",
      lossless: "Lossless",
      ffmpegMissing: "Install FFmpeg to trim videos (brew install ffmpeg). No quality is lost — streams are copied, not re-encoded.",
      start: "Start",
      end: "End",
      kept: "Keeping {duration}",
      setStart: "Cut front here",
      setEnd: "Cut back here",
      reset: "Reset",
      apply: "Apply trim only",
      enterSavesToo: "Enter applies the trim and saves the name together.",
      savedWithRename: "Trimmed and saved",
      hint: "Play the video, pause where you want to cut, then press {startKey} (front) or {endKey} (back).",
      keyframeNote: "Stream copy keeps 100% quality. Cuts may snap to the nearest keyframe (typical on phone videos).",
    },
    support: {
      message:
        "I built this because I couldn't find a fast way to organize my phone photos and videos. If it saves you time, a coffee helps me keep improving it.",
      coffee: "Buy me a coffee",
      email: "Email",
      linkedin: "LinkedIn",
    },
  },
  es: {
    appTitle: "Quick Media Organizer",
    welcomeTitle: "Organiza miles de fotos y vídeos con el teclado",
    welcomeStep1: "Elige una carpeta llena de archivos del móvil (fotos, vídeos, IMG_1234…)",
    welcomeStep2: "Usa Enter, Ctrl+F, Ctrl+D y ⌘⇧Space / Ctrl+Space — los atajos siempre están visibles",
    welcomeStep3: "Renombra en la raíz o guarda en carpetas como gym/, viajes/portugal/ o documentos/",
    welcomeStart: "Entendido, empezar",
    welcomeDontShow: "No volver a mostrar",
    openFolder: "Abrir carpeta",
    noFolder: "Elige una carpeta para empezar a organizar tu contenido.",
    nameLabel: "Nombre",
    namePlaceholder: "Escribe un nombre descriptivo…",
    armedFolder: "Guardar en",
    armedBanner: "Modo carpeta: {folder} — Esc para cancelar",
    undoHint: "{key} para deshacer",
    livePhoto: "Live Photo",
    progress: "{current} / {total}",
    emptyQueue: "¡Terminado! Todos los archivos de esta carpeta han sido procesados.",
    sessionReachedEnd: "Has llegado al último archivo de esta carpeta.",
    restartQueue: "Empezar de nuevo",
    continueReviewing: "Quedarme en el último",
    sessionStats:
      "{renamed} renombrados · {trashed} eliminados · {moved} movidos · {skipped} saltados",
    writeName: "Escribe un nombre antes de pulsar Enter.",
    armedMoveEmpty: "Guardando en carpeta con el nombre original.",
    chooseFolder: "Elige o escribe una ruta de carpeta.",
    startupError: "No se pudo iniciar la app. Revisa el registro de errores.",
    resumingFolder: "Reanudando última carpeta…",
    sessionPositionReset:
      "No se encontró la última foto en la que estabas (puede que se haya renombrado). Empezando desde el inicio.",
    sessionResumeBanner: "Retomando en la foto {current} de {total}.",
    sessionResumeContinue: "Continuar aquí",
    subfolderMediaNotice:
      "Hay {count} fotos/vídeos más en subcarpetas. Activa «Incluir subcarpetas» en Opciones (Ctrl+O) para revisarlos.",
    common: {
      ok: "OK",
      cancel: "Cancelar",
      close: "Cerrar",
    },
    renamed: "Renombrado",
    trashed: "Movido a _deleted",
    savedFolder: "Guardado en carpeta",
    undone: "Deshecho",
    shortcuts: {
      enter: "Guardar",
      enterTrimSave: "Recortar + guardar",
      folder: "Carpeta",
      delete: "Eliminar",
      space: "Saltar",
      nav: "Navegar",
      prev: "Anterior",
      next: "Siguiente",
      undo: "Deshacer",
      info: "Info",
      options: "Opciones",
      help: "Ayuda",
    },
    hints: {
      enter: "Enter guardar",
      enterTrimSave: "Enter recortar + guardar",
      folder: "Ctrl+F carpeta",
      delete: "Ctrl+D eliminar",
    },
    folderPicker: {
      title: "Guardar en carpeta",
      search: "Buscar o crear…",
      favorites: "Favoritas",
      recent: "Recientes",
      existing: "Carpetas existentes",
      confirm: "Confirmar",
      cancel: "Cancelar",
      createHint: "Usa rutas como viajes/portugal/2024",
      discard: "¿Descartar la ruta de carpeta escrita?",
      keyboardHint: "Escribe para filtrar · Tab completa · Enter confirma · ↑↓ navega",
      matches: "Carpetas coincidentes",
      noMatches: "Ninguna carpeta coincide — Enter crea una ruta nueva",
    },
    metadata: {
      toggle: "Pulsa {key} para ver detalles",
      file: "Archivo",
      date: "Fecha",
      size: "Tamaño",
      dimensions: "Dimensiones",
    },
    options: {
      title: "Opciones",
      layout: "Disposición",
      layoutSidebar: "Preview izquierda, controles derecha (vertical)",
      layoutBottom: "Preview arriba, controles abajo",
      sort: "Orden",
      sortExif: "Fecha de captura (EXIF)",
      sortName: "Nombre de archivo",
      sortModified: "Fecha de modificación",
      recursive: "Incluir subcarpetas",
      recursiveHint: "Vuelve a escanear subcarpetas y puede cambiar el orden de la cola.",
      videoSound: "Reproducir vídeos con sonido",
      videoSoundHint: "Autoplay con audio al revisar clips.",
      renameMode: "Modo de renombrado",
      renameFree: "Nombres libres con contador",
      renamePrefix: "Prefijo + contador",
      language: "Idioma",
      errorLog: "Registro de errores",
      errorLogHint: "Guardado para depuración. Se borra tras corregirlos.",
      errorLogPath: "Archivo de log",
    },
    help: {
      title: "Atajos de teclado",
      faqDeleted: "Ctrl+D mueve archivos a _deleted/ dentro del álbum (no a la Papelera del sistema). Usa Ctrl+Z para deshacer.",
      faqDates: "Se conservan las fechas EXIF y las fechas originales del archivo.",
      modifierHint: "En Mac usa ⌘ en lugar de Ctrl. Los atajos con modificador funcionan mientras escribes.",
    },
    preview: {
      preparing: "Preparando vista previa…",
      unsupportedFormat:
        "Este formato no se reproduce en la vista previa integrada (habitual con MKV y algunos AVI).",
      proxyHint: "Copia compatible para la vista previa. El archivo original no cambia.",
      openExternal: "Abrir en app predeterminada",
      imageFailed: "No se pudo previsualizar este archivo.",
      imageEmpty: "El archivo está vacío (0 bytes). Puede ser una descarga fallida.",
      imageTooSmall: "El archivo es demasiado pequeño para ser una imagen válida.",
      imageContentMismatch:
        "La extensión .jpg no coincide con el contenido. Puede ser una descarga rota o un caché incompleto.",
      imageUnknown:
        "La vista previa integrada no pudo decodificar esta imagen. Prueba a abrirla en Vista Previa o Fotos.",
    },
    trim: {
      title: "Recortar vídeo",
      lossless: "Sin pérdida",
      ffmpegMissing: "Instala FFmpeg para recortar (brew install ffmpeg). No se pierde calidad — se copian los streams, sin re-codificar.",
      start: "Inicio",
      end: "Final",
      kept: "Se conserva {duration}",
      setStart: "Cortar delante aquí",
      setEnd: "Cortar detrás aquí",
      reset: "Restablecer",
      apply: "Solo recortar",
      enterSavesToo: "Enter aplica el recorte y guarda el nombre a la vez.",
      savedWithRename: "Recortado y guardado",
      hint: "Reproduce el vídeo, pausa donde quieras cortar y pulsa {startKey} (delante) o {endKey} (detrás).",
      keyframeNote: "La copia de stream mantiene el 100% de calidad. El corte puede ajustarse al fotograma clave más cercano.",
    },
    support: {
      message:
        "Hice esto porque no encontraba una forma ágil de organizar las fotos y vídeos de mi móvil. Si te ahorra tiempo, un café me ayuda a seguir mejorándolo.",
      coffee: "Invítame a un café",
      email: "Email",
      linkedin: "LinkedIn",
    },
  },
} as const;

export type Messages = (typeof messages)["en"];

export function t(locale: Locale, key: string): string {
  const table = messages[locale] as Record<string, unknown>;
  const parts = key.split(".");
  let value: unknown = table;
  for (const part of parts) {
    value = (value as Record<string, unknown> | undefined)?.[part];
  }
  return typeof value === "string" ? value : key;
}

export function format(
  locale: Locale,
  key: string,
  vars: Record<string, string | number>,
): string {
  let text = t(locale, key);
  for (const [name, val] of Object.entries(vars)) {
    text = text.replace(`{${name}}`, String(val));
  }
  return text;
}

export function detectLocale(): Locale {
  if (typeof navigator === "undefined") return "en";
  return navigator.language.toLowerCase().startsWith("es") ? "es" : "en";
}

export { messages };
