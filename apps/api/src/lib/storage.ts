import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { env } from "../env";

export const dataDir = path.resolve(env.DATA_DIR);
export const sessionsDir = path.join(dataDir, "sessions");
export const templatesDir = path.join(dataDir, "templates");
export const reportsDir = path.join(dataDir, "reports");

export async function ensureDataDirs(): Promise<void> {
  await Promise.all(
    [dataDir, sessionsDir, templatesDir, reportsDir].map((dir) => mkdir(dir, { recursive: true })),
  );
}

export function sha256(bytes: Buffer | Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export function normalizeTxt(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .join("\n");
}

/** Evita que un nombre de archivo del ZIP salga de su carpeta. */
export function safeFileBase(name: string): string {
  return (
    path
      .basename(name)
      // eslint-disable-next-line no-control-regex -- limpieza de caracteres de control
      .replace(/[<>:"/\\|?*\x00-\x1f]/g, "_")
      .replace(/^\.+/, "_")
      .slice(0, 180) || "archivo"
  );
}

export async function saveSessionMedia(
  sessionId: string,
  files: Map<string, Buffer>,
): Promise<string> {
  const dir = path.join(sessionsDir, sessionId, "media");
  await mkdir(dir, { recursive: true });
  for (const [name, bytes] of files) {
    await writeFile(path.join(dir, safeFileBase(name)), bytes);
  }
  return dir;
}

export async function readSessionMedia(
  sessionId: string,
  filename: string,
): Promise<Buffer | null> {
  try {
    return await readFile(path.join(sessionsDir, sessionId, "media", safeFileBase(filename)));
  } catch {
    return null;
  }
}

export async function saveTemplateFile(
  originalName: string,
  bytes: Buffer,
): Promise<{ id: string; storagePath: string; filename: string }> {
  const id = randomUUID();
  const filename = safeFileBase(originalName || "plantilla.docx");
  const storagePath = path.join(templatesDir, `${id}.docx`);
  await writeFile(storagePath, bytes);
  return { id, storagePath, filename };
}

export function reportFileName(start: string, end: string): string {
  const fmt = (v: string) => v.split("-").reverse().join("-");
  return `Parte_Diario_${fmt(start)}_${fmt(end)}.docx`;
}

export async function saveReportFile(filename: string, bytes: Buffer): Promise<string> {
  const storagePath = path.join(reportsDir, `${randomUUID()}__${safeFileBase(filename)}`);
  await writeFile(storagePath, bytes);
  return storagePath;
}
