import type { Novedad, ParsedChat } from "@parte/shared";
import { makeUtcDate, monthNumberFromSpanish, ymd } from "../lib/dates";

const RE_NOTE_START =
  /^(siendo las|respetuosamente (le )?informo|por medio de la presente|cumplo con informar(le)?|me permito informar(le)?|por este medio (le )?informo)/i;

const RE_CLOSING =
  /(es lo que informo( para( su conocimiento( y fines? (de lugar|correspondientes)?)?)?)?|para su conocimiento( y fines? (de lugar|correspondientes)?)?|para los fines (de lugar|correspondientes)|lo que comunico para (su )?conocimiento)/gi;

const RE_FECHA_NUM = /(\d{1,2})[/\-.](\d{1,2})[/\-.](\d{2,4})/g;
const RE_FECHA_TXT =
  /(\d{1,2})\s+de\s+(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre)\s*(?:del?\s+|,?\s*)(\d{4})/gi;

const HEADER_KEYWORDS = [
  "proc",
  "operativo",
  "incaut",
  "seguimiento",
  "visita",
  "denuncia",
  "contaminac",
  "sónic",
  "sonica",
  "ruido",
  "anti",
  "p.n",
  "comandancia",
  "destacamento",
  "departamento",
  "garantía de paz",
  "fuerza de tarea",
  "supervisión",
  "patrulla",
];

function normalizeHeader(line: string): string {
  return line.trim().replace(/^\*+/, "").replace(/\*+$/, "").trim();
}

export function isHeaderLine(line: string): boolean {
  const t = normalizeHeader(line);
  if ([...t].length < 4) return false;
  if (line.trim().startsWith("**") && t.length > 4) return true;

  const letters = [...t].filter((c) => /\p{L}/u.test(c));
  if (letters.length >= 10) {
    const upper = letters.filter((c) => c === c.toUpperCase() && c !== c.toLowerCase()).length;
    if (upper / letters.length > 0.8) {
      const lower = t.toLowerCase();
      if (HEADER_KEYWORDS.some((k) => lower.includes(k))) return true;
      if (letters.length <= 45) return true;
    }
  }
  return false;
}

export function isNoteStartBlock(text: string): boolean {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  let i = 0;
  let sawHeader = false;
  while (i < lines.length && isHeaderLine(lines[i])) {
    sawHeader = true;
    i += 1;
  }
  if (sawHeader) return true;
  if (i < lines.length) return RE_NOTE_START.test(lines[i]);
  return false;
}

export function splitHeadersBody(text: string): {
  headers: string[];
  body: string;
} {
  const headers: string[] = [];
  const bodyLines: string[] = [];
  let inHeaders = true;
  for (const line of text.split("\n")) {
    const t = line.trim();
    if (inHeaders && t && isHeaderLine(t)) {
      headers.push(normalizeHeader(t));
    } else {
      inHeaders = false;
      bodyLines.push(line);
    }
  }
  return { headers, body: bodyLines.join("\n").trim() };
}

export function extractClosing(text: string): string | null {
  const re = new RegExp(RE_CLOSING.source, "gi");
  let last: RegExpExecArray | null = null;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    last = m;
    if (m.index === re.lastIndex) re.lastIndex += 1;
  }
  if (!last) return null;

  let start = last.index;
  let end = last.index + last[0].length;
  while (start > 0 && text[start - 1] !== "." && text[start - 1] !== "\n") start -= 1;
  while (end < text.length && text[end] !== "." && text[end] !== "\n") end += 1;
  if (text[end] === ".") end += 1;
  const s = text.slice(start, end).trim();
  return s.length >= 10 ? s : null;
}

export function mostFrequentDate(text: string): string | null {
  const counts = new Map<string, number>();
  const add = (year: number, month: number, day: number) => {
    const date = makeUtcDate(year, month, day);
    if (!date) return;
    const key = ymd(date);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  };

  for (const m of text.matchAll(RE_FECHA_NUM)) {
    const day = Number(m[1]);
    const month = Number(m[2]);
    const rawYear = m[3];
    const year =
      rawYear.length === 2
        ? Number(rawYear) < 70
          ? 2000 + Number(rawYear)
          : 1900 + Number(rawYear)
        : Number(rawYear);
    add(year, month, day);
  }
  for (const m of text.matchAll(RE_FECHA_TXT)) {
    const day = Number(m[1]);
    const month = monthNumberFromSpanish(m[2]);
    const year = Number(m[3]);
    if (month) add(year, month, day);
  }

  let best: { date: string; count: number } | null = null;
  for (const [date, count] of counts) {
    if (!best || count > best.count || (count === best.count && date > best.date)) {
      best = { date, count };
    }
  }
  return best?.date ?? null;
}

export function refreshMetadata(n: Novedad): void {
  const joined = `${n.headers.join("\n")}\n${n.bodyRaw}`;
  n.fechaMencionada = mostFrequentDate(joined);
  n.closing = extractClosing(n.bodyRaw);
}

function finalizeNovedad(n: Novedad): void {
  refreshMetadata(n);
  if (!n.bodyFormal) n.bodyFormal = n.bodyRaw;
}

function newNovedad(id: number, sender: string): Novedad {
  return {
    id,
    headers: [],
    bodyRaw: "",
    closing: null,
    photos: [],
    videosSkipped: [],
    fechaMencionada: null,
    include: true,
    isOrphan: false,
    bodyFormal: "",
    sender,
  };
}

export function groupNovedades(chat: ParsedChat, mediaNames: Set<string>): Novedad[] {
  const mediaLower = new Set([...mediaNames].map((n) => n.toLowerCase()));
  const novedades: Novedad[] = [];
  let current: Novedad | null = null;
  let pendingPhotos: string[] = [];
  let pendingVideos: string[] = [];
  let sinceText = 0;
  let idc = 0;

  const flush = () => {
    if (current) {
      finalizeNovedad(current);
      novedades.push(current);
      current = null;
    }
  };

  for (const msg of chat.messages) {
    const text = msg.text.trim();
    const hasText = text.length > 0;

    if (msg.attachment) {
      const att = msg.attachment;
      const present = mediaLower.has(att.filename.toLowerCase());
      if (att.kind === "image") {
        if (current) {
          if (sinceText <= 5 && present) current.photos.push(att.filename);
        } else if (present) {
          pendingPhotos.push(att.filename);
        }
      } else if (att.kind === "video") {
        if (current) current.videosSkipped.push(att.filename);
        else pendingVideos.push(att.filename);
      }
      if (!hasText) continue;
    }
    if (!hasText) continue;

    if (isNoteStartBlock(text)) {
      flush();
      idc += 1;
      const n = newNovedad(idc, msg.sender);
      const { headers, body } = splitHeadersBody(text);
      n.headers = headers;
      n.bodyRaw = body;
      n.photos.push(...pendingPhotos);
      n.videosSkipped.push(...pendingVideos);
      pendingPhotos = [];
      pendingVideos = [];
      current = n;
      sinceText = 0;
    } else if (!current) {
      idc += 1;
      const n = newNovedad(idc, msg.sender);
      n.bodyRaw = text;
      n.photos.push(...pendingPhotos);
      n.videosSkipped.push(...pendingVideos);
      pendingPhotos = [];
      pendingVideos = [];
      current = n;
      sinceText = 0;
    } else {
      const c = current as Novedad;
      if (c.bodyRaw) c.bodyRaw += "\n\n";
      c.bodyRaw += text;
      sinceText += 1;
    }
  }
  flush();

  if (pendingPhotos.length > 0) {
    idc += 1;
    const n = newNovedad(idc, "(sin remitente)");
    n.isOrphan = true;
    n.photos = pendingPhotos;
    n.videosSkipped = pendingVideos;
    n.include = true;
    novedades.push(n);
  }

  return novedades;
}

export function inferReportRange(novedades: Novedad[]): {
  majority: string | null;
  outliers: number[];
} {
  const counts = new Map<string, number>();
  for (const n of novedades) {
    if (n.fechaMencionada) {
      counts.set(n.fechaMencionada, (counts.get(n.fechaMencionada) ?? 0) + 1);
    }
  }
  let majority: { date: string; count: number } | null = null;
  for (const [date, count] of counts) {
    if (!majority || count > majority.count || (count === majority.count && date > majority.date)) {
      majority = { date, count };
    }
  }
  const majorityDate = majority?.date ?? null;
  const outliers = majorityDate
    ? novedades
        .filter((n) => n.fechaMencionada && n.fechaMencionada !== majorityDate)
        .map((n) => n.id)
    : [];
  return { majority: majorityDate, outliers };
}
