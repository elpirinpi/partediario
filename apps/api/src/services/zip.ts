import path from "node:path";
import JSZip from "jszip";
import type { ChatFormat, MediaFile, Novedad, SessionWarning } from "@parte/shared";
import { parseChatTxt } from "./parser";
import { groupNovedades, inferReportRange } from "./grouping";
import { addDays, ymd } from "../lib/dates";
import { normalizeTxt, sha256 } from "../lib/storage";

const MAX_ENTRIES = 20_000;
const MAX_TOTAL_UNCOMPRESSED = 1_500_000_000;
const MAX_FILE_UNCOMPRESSED = 200_000_000;
const MAX_RATIO = 1_000;

const IMAGE_EXTS = new Set(["jpg", "jpeg", "png", "webp", "gif", "bmp"]);
const VIDEO_EXTS = new Set(["mp4", "mov", "3gp", "avi", "mkv", "webm"]);

export interface ProcessedZip {
  zipSha256: string;
  txtSha256: string;
  txtName: string;
  format: ChatFormat;
  messageCount: number;
  novedades: Novedad[];
  mediaFiles: MediaFile[];
  mediaBuffers: Map<string, Buffer>;
  warnings: SessionWarning[];
  dateFrom: string | null;
  dateTo: string | null;
  majority: string | null;
  outliers: number[];
}

function extension(name: string): string {
  const base = path.basename(name).toLowerCase();
  const idx = base.lastIndexOf(".");
  return idx >= 0 ? base.slice(idx + 1) : "";
}

function isUnsafePath(name: string): boolean {
  if (!name || name.startsWith("/") || name.includes("\\")) return true;
  if (/^[A-Za-z]:/.test(name)) return true;
  return name.split("/").some((part) => part === "..");
}

interface ZipInternalData {
  uncompressedSize?: number;
  compressedSize?: number;
}

export async function processWhatsappZip(buffer: Buffer, _zipName: string): Promise<ProcessedZip> {
  let zip: JSZip;
  try {
    zip = await JSZip.loadAsync(buffer);
  } catch (error) {
    throw new Error(
      `El archivo no es un .zip válido: ${error instanceof Error ? error.message : String(error)}`,
      { cause: error },
    );
  }

  const files = Object.values(zip.files).filter((f) => !f.dir);
  if (files.length > MAX_ENTRIES) {
    throw new Error(
      `El .zip contiene demasiados archivos (${files.length}); el máximo permitido es ${MAX_ENTRIES}.`,
    );
  }

  let totalDeclared = 0;
  for (const file of files) {
    if (isUnsafePath(file.name)) {
      throw new Error(`El .zip contiene una ruta insegura: '${file.name}'. Se canceló la carga.`);
    }
    const data = (file as unknown as { _data?: ZipInternalData })._data ?? {};
    const size = data.uncompressedSize ?? 0;
    if (size > MAX_FILE_UNCOMPRESSED) {
      throw new Error(
        `El archivo '${file.name}' es demasiado grande (${Math.round(size / 1_000_000)} MB).`,
      );
    }
    if (size > 10_000_000 && data.compressedSize) {
      if (size / Math.max(1, data.compressedSize) > MAX_RATIO) {
        throw new Error(`El archivo '${file.name}' parece un zip bomb (compresión anómala).`);
      }
    }
    totalDeclared += size;
    if (totalDeclared > MAX_TOTAL_UNCOMPRESSED) {
      throw new Error(
        `El .zip descomprimido supera el límite de ${MAX_TOTAL_UNCOMPRESSED / 1_000_000} MB.`,
      );
    }
  }

  const warnings: SessionWarning[] = [];
  const txtCandidates: Array<{ name: string; bytes: Buffer }> = [];
  const mediaBuffers = new Map<string, Buffer>();
  const mediaFiles: MediaFile[] = [];
  const mediaNames = new Set<string>();
  let totalRead = 0;

  for (const file of files) {
    const base = path.basename(file.name).toLowerCase();
    if (!base || base.startsWith(".")) continue;

    if (base.endsWith(".txt")) {
      const bytes = await file.async("nodebuffer");
      txtCandidates.push({ name: file.name, bytes });
      continue;
    }

    const ext = extension(base);
    if (!IMAGE_EXTS.has(ext) && !VIDEO_EXTS.has(ext)) continue;

    const bytes = await file.async("nodebuffer");
    totalRead += bytes.length;
    if (bytes.length > MAX_FILE_UNCOMPRESSED) {
      throw new Error(`El archivo '${file.name}' es demasiado grande.`);
    }
    if (totalRead > MAX_TOTAL_UNCOMPRESSED) {
      throw new Error(
        `El .zip descomprimido supera el límite de ${MAX_TOTAL_UNCOMPRESSED / 1_000_000} MB.`,
      );
    }

    const kind = IMAGE_EXTS.has(ext) ? "image" : "video";
    mediaNames.add(base);
    mediaFiles.push({ name: base, size: bytes.length, kind });
    if (kind === "image") mediaBuffers.set(base, bytes);
  }

  if (txtCandidates.length === 0) {
    throw new Error("No se encontró ningún .txt dentro del .zip.");
  }
  txtCandidates.sort((a, b) => b.bytes.length - a.bytes.length);
  const chosen = txtCandidates[0];
  const txt = chosen.bytes.toString("utf8");
  const txtName = path.basename(chosen.name);

  const chat = parseChatTxt(txt);
  const novedades = groupNovedades(chat, mediaNames);

  const mediaLower = new Set(mediaBuffers.keys());
  const missing = new Set<string>();
  for (const msg of chat.messages) {
    if (msg.attachment?.kind === "image") {
      const key = msg.attachment.filename.toLowerCase();
      if (!mediaLower.has(key) && !missing.has(key)) {
        missing.add(key);
        warnings.push({
          kind: "warning",
          message: `La foto '${msg.attachment.filename}' está referenciada en el chat pero NO se encontró dentro del .zip. Exporte de nuevo con 'Adjuntar archivos multimedia'.`,
        });
      }
    }
  }

  const { majority, outliers } = inferReportRange(novedades);
  let dateFrom: string | null = null;
  let dateTo: string | null = null;
  if (majority) {
    const from = new Date(`${majority}T00:00:00.000Z`);
    dateFrom = majority;
    dateTo = ymd(addDays(from, 1));
  }

  const orphanCount = novedades.filter((n) => n.isOrphan).length;
  if (orphanCount > 0) {
    warnings.push({
      kind: "warning",
      message: `${orphanCount} grupo(s) de fotos sin texto asociado (posible mensaje borrado). Revise la sección de huérfanas y reasígnelas a la novedad correcta.`,
    });
  }
  const videoCount = novedades.reduce((sum, n) => sum + n.videosSkipped.length, 0);
  if (videoCount > 0) {
    warnings.push({
      kind: "info",
      message: `${videoCount} video(s) detectados: se excluyen del .docx, pero sus novedades se conservan.`,
    });
  }

  if (!majority) {
    warnings.push({
      kind: "warning",
      message: "No se pudo inferir la fecha del reporte desde el texto; ingrésela manualmente.",
    });
  }

  const formatLabel =
    chat.format === "bracket"
      ? "corchete (Android)"
      : chat.format === "dash"
        ? "guion (iOS)"
        : "desconocido (se intentó parsear igual)";
  warnings.unshift({
    kind: "info",
    message: `Formato detectado: ${formatLabel}. Mensajes útiles: ${chat.messages.length}.`,
  });

  return {
    zipSha256: sha256(buffer),
    txtSha256: sha256(normalizeTxt(txt)),
    txtName,
    format: chat.format,
    messageCount: chat.messages.length,
    novedades,
    mediaFiles,
    mediaBuffers,
    warnings,
    dateFrom,
    dateTo,
    majority,
    outliers,
  };
}

export function applyPreselection(
  novedades: Novedad[],
  lastReportEnd: string | null,
): { novedades: Novedad[]; newlyIncluded: number; lastEnd: string | null } {
  let newlyIncluded = 0;
  for (const n of novedades) {
    const include = !lastReportEnd || !n.fechaMencionada ? true : n.fechaMencionada > lastReportEnd;
    n.include = include;
    if (include) newlyIncluded += 1;
  }
  return { novedades, newlyIncluded, lastEnd: lastReportEnd };
}
