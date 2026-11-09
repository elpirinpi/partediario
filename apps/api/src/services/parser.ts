import type {
  Attachment,
  AttachmentKind,
  ChatFormat,
  ChatMessage,
  ParsedChat,
} from "@parte/shared";

const IMAGE_EXTS = new Set(["jpg", "jpeg", "png", "webp", "gif", "bmp"]);
const VIDEO_EXTS = new Set(["mp4", "mov", "3gp", "avi", "mkv", "webm"]);

const SYSTEM_PATTERNS = [
  "cifrados de extremo a extremo",
  "cifrado de extremo a extremo",
  "los mensajes y las llamadas",
  "los mensajes que envías a este chat",
  "creó el grupo",
  "cambió el nombre del grupo",
  "cambió la foto del grupo",
  "cambió la descripción del grupo",
  "añadió a ",
  "eliminó a ",
  "salió del grupo",
  "se unió usando el enlace",
  "ahora eres administrador",
  "cambió los ajustes del grupo",
  "activó los mensajes temporales",
  "desactivó los mensajes temporales",
];

const RE_ADJUNTO_BRACKET = /<adjunto:\s*([^<>\s]+\.[A-Za-z0-9]{2,4})\s*>/gi;
const RE_ADJUNTO_IOS = /([^<>\s]+\.[A-Za-z0-9]{2,4})\s*\((?:archivo adjunto|file attached)\)/gi;
const RE_ADJUNTO_OMITIDO =
  /(imagen omitida|video omitido|gif omitido|\(archivo omitido\)|image omitted|video omitted)/gi;

const RE_BRACKET =
  /^\[(\d{1,2})\/(\d{1,2})\/(\d{2,4}),\s*(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([ap])?\.?\s*m?\.?\s*\]\s*([^:]+):\s?(.*)$/i;
const RE_DASH =
  /^(\d{1,2})\/(\d{1,2})\/(\d{2,4}),\s*(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([ap])?\.?\s*m?\.?\s*-\s*([^:]+):\s?(.*)$/i;

export function attachmentKind(name: string): AttachmentKind {
  const ext = name.toLowerCase().split(".").pop() ?? "";
  if (IMAGE_EXTS.has(ext)) return "image";
  if (VIDEO_EXTS.has(ext)) return "video";
  return "other";
}

export function cleanInvisible(value: string): string {
  return value.replace(/[\u200E\u200F\uFEFF]/g, "").trim();
}

function replaceAll(re: RegExp, text: string, value: string): string {
  return text.replace(re, value);
}

function extractAttachment(text: string): {
  attachment: Attachment | null;
  text: string;
} {
  let found: string | null = null;
  RE_ADJUNTO_BRACKET.lastIndex = 0;
  const bracket = RE_ADJUNTO_BRACKET.exec(text);
  if (bracket) {
    found = bracket[1];
  } else {
    RE_ADJUNTO_IOS.lastIndex = 0;
    const ios = RE_ADJUNTO_IOS.exec(text);
    if (ios) found = ios[1];
  }

  if (found) {
    const name = cleanInvisible(found);
    let rest = replaceAll(RE_ADJUNTO_BRACKET, text, "");
    rest = replaceAll(RE_ADJUNTO_IOS, rest, "");
    return {
      attachment: { filename: name, kind: attachmentKind(name) },
      text: cleanInvisible(rest),
    };
  }

  const rest = replaceAll(RE_ADJUNTO_OMITIDO, text, "");
  return { attachment: null, text: cleanInvisible(rest) };
}

function normYear(raw: string): number {
  if (raw.length === 2) {
    const v = Number(raw);
    return v < 70 ? 2000 + v : 1900 + v;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : new Date().getFullYear();
}

function makeTs(
  year: number,
  month: number,
  day: number,
  hour: number,
  minute: number,
): string | null {
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const date = new Date(
    Date.UTC(year, month - 1, day, Math.min(hour, 23), Math.min(minute, 59), 0),
  );
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function isSystemText(text: string, sender: string): boolean {
  if (!sender.trim()) return true;
  const t = text.toLowerCase();
  return SYSTEM_PATTERNS.some((p) => t.includes(p));
}

export function parseChatTxt(content: string): ParsedChat {
  const messages: ChatMessage[] = [];
  let format: ChatFormat = "unknown";
  const normalized = content.replace(/^\uFEFF/, "");

  for (const rawLine of normalized.split(/\r?\n/)) {
    const line = rawLine.trimEnd();
    if (!line.trim()) {
      const last = messages[messages.length - 1];
      if (last) last.text += "\n";
      continue;
    }

    let parsed: { ts: string | null; sender: string; body: string } | null = null;

    let m = RE_BRACKET.exec(line);
    if (m) {
      if (format === "unknown") format = "bracket";
      parsed = buildParsed(m);
    } else {
      m = RE_DASH.exec(line);
      if (m) {
        if (format === "unknown") format = "dash";
        parsed = buildParsed(m);
      }
    }

    if (parsed) {
      const { attachment, text } = extractAttachment(parsed.body);
      messages.push({
        ts: parsed.ts,
        sender: parsed.sender,
        text,
        attachment,
        isSystem: isSystemText(text, parsed.sender),
      });
    } else {
      const last = messages[messages.length - 1];
      if (last) {
        if (last.text && !last.text.endsWith("\n")) last.text += "\n";
        last.text += line.trim();
      }
    }
  }

  return { messages: messages.filter((msg) => !msg.isSystem), format };
}

function buildParsed(m: RegExpExecArray): {
  ts: string | null;
  sender: string;
  body: string;
} {
  const day = Number(m[1]) || 1;
  const month = Number(m[2]) || 1;
  const year = normYear(m[3]);
  let hour = Number(m[4]) || 0;
  const minute = Number(m[5]) || 0;
  const ampm = (m[7] ?? "").toLowerCase();
  if (ampm === "p" && hour < 12) hour += 12;
  if (ampm === "a" && hour === 12) hour = 0;
  return {
    ts: makeTs(year, month, day, hour, minute),
    sender: m[8].trim(),
    body: m[9],
  };
}
