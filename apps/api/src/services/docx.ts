import { randomBytes } from "node:crypto";
import JSZip from "jszip";
import sharp from "sharp";
import { XMLValidator } from "fast-xml-parser";
import type { Novedad } from "@parte/shared";
import { stripAsterisks } from "./ai";
import { formatDdMmYyyy, formatLongEs } from "../lib/dates";

const EMU_PER_INCH = 914400;
export const DEFAULT_PHOTO_WIDTH_IN = 2.36;
export const DEFAULT_PHOTO_MAX_HEIGHT_IN = 3.15;

export interface BuildOptions {
  fontFamily: string;
  fontSizePt: number;
  photoWidthIn: number;
  photoMaxHeightIn: number;
  appVersion: string;
}

export const DEFAULT_BUILD_OPTIONS: BuildOptions = {
  fontFamily: "Tahoma",
  fontSizePt: 13,
  photoWidthIn: DEFAULT_PHOTO_WIDTH_IN,
  photoMaxHeightIn: DEFAULT_PHOTO_MAX_HEIGHT_IN,
  appVersion: "3.0.0",
};

export interface BuildResult {
  data: Buffer;
  warnings: string[];
  imagesEmbedded: number;
  novedadesIncluidas: number;
}

/* ------------------------------------------------------------------ */
/* XML helpers                                                         */
/* ------------------------------------------------------------------ */

export function esc(value: string): string {
  const clean = [...value]
    .filter((c) => {
      const code = c.codePointAt(0) ?? 0;
      return (
        code === 0x9 ||
        code === 0xa ||
        code === 0xd ||
        (code >= 0x20 && code <= 0xd7ff) ||
        (code >= 0xe000 && code <= 0xfffd) ||
        (code >= 0x10000 && code <= 0x10ffff)
      );
    })
    .join("");
  return clean.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function xmlUnescape(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");
}

class ParaIdGen {
  private used: Set<number>;

  constructor(existing: Set<number>) {
    this.used = new Set(existing);
  }

  next(): string {
    for (let i = 0; i < 10_000; i += 1) {
      const value = randomBytes(4).readUInt32BE(0) & 0x7fff_ffff;
      if (value !== 0 && !this.used.has(value)) {
        this.used.add(value);
        return value.toString(16).toUpperCase().padStart(8, "0");
      }
    }
    let value = 0x1000_0001;
    while (this.used.has(value)) {
      value = (value + 1) & 0x7fff_ffff;
      if (value === 0) value = 1;
    }
    this.used.add(value);
    return value.toString(16).toUpperCase().padStart(8, "0");
  }
}

interface EmbeddedImage {
  rid: string;
  part: string;
  bytes: Buffer;
  contentType: string;
}

function tagHasAttr(tag: string, attr: string): boolean {
  let rest = tag;
  for (;;) {
    const pos = rest.indexOf(attr);
    if (pos < 0) return false;
    const before = pos === 0 ? "<" : rest[pos - 1];
    const beforeOk = pos === 0 || /\s/.test(before) || before === "<";
    if (beforeOk && rest[pos + attr.length] === "=") return true;
    rest = rest.slice(pos + attr.length);
  }
}

function ensureNamespaces(doc: string): string {
  const needed: Array<[string, string]> = [
    ["xmlns:w14", "http://schemas.microsoft.com/office/word/2010/wordml"],
    ["xmlns:wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"],
    ["xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main"],
    ["xmlns:pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"],
    ["xmlns:r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"],
  ];
  const start = doc.indexOf("<w:document");
  if (start < 0) return doc;
  const relEnd = doc.indexOf(">", start);
  if (relEnd < 0) return doc;
  const tag = doc.slice(start, relEnd);
  let add = "";
  for (const [key, value] of needed) {
    if (!tagHasAttr(tag, key)) add += ` ${key}="${value}"`;
  }
  return add ? doc.slice(0, relEnd) + add + doc.slice(relEnd) : doc;
}

function validateNamespaces(doc: string): void {
  const declared = new Set<string>();
  for (const m of doc.matchAll(/xmlns:([A-Za-z_][\w.-]*)=/g)) {
    declared.add(m[1]);
  }
  const check = (prefix: string) => {
    if (prefix === "xml" || prefix === "xmlns" || declared.has(prefix)) return;
    throw new Error(
      `el prefijo '${prefix}' se usa pero no está declarado con xmlns (Word rechazará el archivo)`,
    );
  };
  for (const m of doc.matchAll(/<\/?([A-Za-z_][\w.-]*):/g)) check(m[1]);
  for (const tag of doc.matchAll(/<[^>]*>/g)) {
    for (const m of tag[0].matchAll(/[\s<]([A-Za-z_][\w.-]*):[A-Za-z_]/g)) {
      check(m[1]);
    }
  }
}

function replaceInParagraphs(
  doc: string,
  re: RegExp,
  replacement: string,
  maxParas: number,
): { doc: string; count: number } {
  const paraRe = /<w:p(\s[^>]*)?\/>|<w:p(\s[^>]*)?>([\s\S]*?)<\/w:p>/g;
  const tRe = /<w:t(\s[^>]*)?\/>|<w:t(\s[^>]*)?>([\s\S]*?)<\/w:t>/g;
  const rprRe = /<w:rPr>[\s\S]*?<\/w:rPr>/;
  const replaceRe = new RegExp(re.source, re.flags.includes("g") ? re.flags : `${re.flags}g`);

  let out = "";
  let last = 0;
  let nParas = 0;
  let count = 0;

  for (const caps of doc.matchAll(paraRe)) {
    const whole = caps[0];
    const start = caps.index ?? 0;
    out += doc.slice(last, start);

    if (caps[1] !== undefined) {
      out += whole;
      last = start + whole.length;
      continue;
    }

    nParas += 1;
    if (nParas > maxParas) {
      out += whole;
    } else {
      const attrs = caps[2] ?? "";
      const inner = caps[3] ?? "";
      let full = "";
      for (const tc of inner.matchAll(tRe)) {
        if (tc[1] === undefined && tc[3] !== undefined) full += xmlUnescape(tc[3]);
      }
      if (replaceRe.test(full)) {
        count += 1;
        const newText = full.replace(replaceRe, () => replacement);
        const rpr = rprRe.exec(inner)?.[0] ?? "";
        out += `<w:p${attrs}><w:r>${rpr}<w:t xml:space="preserve">${esc(
          newText,
        )}</w:t></w:r></w:p>`;
      } else {
        out += whole;
      }
    }
    last = start + whole.length;
  }
  out += doc.slice(last);
  return { doc: out, count };
}

function insertBefore(haystack: string, marker: string, insertion: string): string {
  const pos = haystack.indexOf(marker);
  if (pos < 0) return haystack;
  return haystack.slice(0, pos) + insertion + haystack.slice(pos);
}

function topLevelTables(doc: string): Array<[number, number]> {
  const openPat = "<w:tbl";
  const closePat = "</w:tbl>";
  const out: Array<[number, number]> = [];
  let i = 0;
  while (i < doc.length) {
    const open = doc.indexOf(openPat, i);
    const close = doc.indexOf(closePat, i);
    if (open < 0 && close < 0) break;
    if (open >= 0 && (close < 0 || open < close)) {
      const after = doc[open + openPat.length];
      const isTblTag = after === ">" || (after !== undefined && /\s/.test(after));
      if (!isTblTag) {
        i = open + openPat.length;
        continue;
      }
      const tagEndOffset = doc.indexOf(">", open);
      if (tagEndOffset < 0) break;
      const tagEnd = tagEndOffset + 1;
      if (doc.slice(open, tagEnd).trimEnd().endsWith("/>")) {
        out.push([open, tagEnd]);
        i = tagEnd;
        continue;
      }
      let depth = 1;
      let j = tagEnd;
      let end: number | null = null;
      while (j < doc.length) {
        const nextOpen = doc.indexOf(openPat, j);
        const nextClose = doc.indexOf(closePat, j);
        if (nextOpen >= 0 && (nextClose < 0 || nextOpen < nextClose)) {
          const ch = doc[nextOpen + openPat.length];
          if (ch === ">" || (ch !== undefined && /\s/.test(ch))) depth += 1;
          j = nextOpen + openPat.length;
        } else if (nextClose >= 0) {
          depth -= 1;
          j = nextClose + closePat.length;
          if (depth === 0) {
            end = j;
            break;
          }
        } else {
          break;
        }
      }
      if (end === null) break;
      out.push([open, end]);
      i = end;
    } else if (close >= 0) {
      i = close + closePat.length;
    } else {
      break;
    }
  }
  return out;
}

interface NovedadTableStyle {
  tblPrInner: string;
  grid: [string, string];
}

function novedadTableStyle(templateXml: string | undefined): NovedadTableStyle {
  if (templateXml) {
    const pr = /<w:tblPr>([\s\S]*?)<\/w:tblPr>/.exec(templateXml)?.[1];
    const grid = [...templateXml.matchAll(/<w:gridCol w:w="(\d+)"/g)].map((m) => m[1]);
    if (pr && grid.length >= 2) {
      return { tblPrInner: pr, grid: [grid[0], grid[1]] };
    }
  }
  // Si no se encuentra un estilo de tabla en la plantilla, se devuelve un estilo vacío
  // para que se herede el formato del documento principal.
  return { tblPrInner: "", grid: ["4082", "5273"] }; // Valores por defecto para que no falle
}

function tahomaPara(gen: ParaIdGen, text: string, _center: boolean, _opts: BuildOptions): string {
  const pid = gen.next();
  // Eliminamos el forzado de Tahoma 13 para que herede el estilo de la plantilla
  return (
    `<w:p w14:paraId="${pid}"><w:pPr></w:pPr>` +
    `<w:r><w:rPr></w:rPr>` +
    `<w:t xml:space="preserve">${esc(text)}</w:t></w:r></w:p>`
  );
}

function emptyPara(gen: ParaIdGen): string {
  const pid = gen.next();
  // Eliminamos la alineación forzada para que herede el estilo de la plantilla
  return `<w:p w14:paraId="${pid}"><w:pPr></w:pPr></w:p>`;
}

interface PreparedImage {
  bytes: Buffer;
  ext: string;
  contentType: string;
  cx: number;
  cy: number;
}

async function prepareImage(bytes: Buffer, opts: BuildOptions): Promise<PreparedImage | null> {
  try {
    const meta = await sharp(bytes).metadata();
    if (!meta.width || !meta.height) return null;

    let cx = Math.max(0.1, opts.photoWidthIn) * EMU_PER_INCH;
    let cy = (cx * meta.height) / meta.width;
    const maxCy = Math.max(0.1, opts.photoMaxHeightIn) * EMU_PER_INCH;
    if (cy > maxCy) {
      cy = maxCy;
      cx = (cy * meta.width) / meta.height;
    }

    if (meta.format === "jpeg") {
      return {
        bytes,
        ext: "jpg",
        contentType: "image/jpeg",
        cx: Math.round(cx),
        cy: Math.round(cy),
      };
    }
    if (meta.format === "png") {
      return {
        bytes,
        ext: "png",
        contentType: "image/png",
        cx: Math.round(cx),
        cy: Math.round(cy),
      };
    }
    const png = await sharp(bytes).png().toBuffer();
    return {
      bytes: png,
      ext: "png",
      contentType: "image/png",
      cx: Math.round(cx),
      cy: Math.round(cy),
    };
  } catch {
    return null;
  }
}

function drawingXml(rid: string, name: string, cx: number, cy: number, docId: number): string {
  return (
    `<w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">` +
    `<wp:extent cx="${cx}" cy="${cy}"/>` +
    `<wp:docPr id="${docId}" name="${name}"/>` +
    `<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>` +
    `<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">` +
    `<pic:pic><pic:nvPicPr><pic:cNvPr id="${docId}" name="${name}"/><pic:cNvPicPr/></pic:nvPicPr>` +
    `<pic:blipFill><a:blip r:embed="${rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>` +
    `<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm>` +
    `<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>` +
    `</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>`
  );
}

async function novedadTable(args: {
  n: Novedad;
  gen: ParaIdGen;
  images: EmbeddedImage[];
  warnings: string[];
  media: Map<string, Buffer>;
  docId: number;
  relsSoFar: string;
  usedParts: Set<string>;
  style: NovedadTableStyle;
  opts: BuildOptions;
}): Promise<string> {
  const { n, gen, images, warnings, media, relsSoFar, usedParts, style, opts } = args;
  let docId = args.docId;

  let left = "";
  if (n.headers.length === 0) {
    left += tahomaPara(gen, `Novedad #${n.id}`, true, opts);
  } else {
    for (const h of n.headers) left += tahomaPara(gen, stripAsterisks(h), true, opts);
  }

  for (const photo of n.photos) {
    const bytes = media.get(photo.toLowerCase());
    if (!bytes) {
      warnings.push(`La foto '${photo}' (novedad #${n.id}) no está dentro del .zip; se omitió.`);
      continue;
    }
    const prep = await prepareImage(bytes, opts);
    if (!prep) {
      warnings.push(
        `No se pudo leer la foto '${photo}' (novedad #${n.id}); se omitió la imagen pero se conserva el texto.`,
      );
      continue;
    }
    const { rid, part, idx } = (() => {
      let idx = images.length + 1;
      for (;;) {
        const rid = `rIdFoto${idx}`;
        const part = `word/media/img${idx}.${prep.ext}`;
        if (!relsSoFar.includes(`Id="${rid}"`) && !usedParts.has(part)) {
          return { rid, part, idx };
        }
        idx += 1;
      }
    })();
    usedParts.add(part);
    images.push({
      rid,
      part,
      bytes: prep.bytes,
      contentType: prep.contentType,
    });
    const dxml = drawingXml(rid, `Foto ${idx}`, prep.cx, prep.cy, docId);
    docId += 1;
    const pid = gen.next();
    left +=
      `<w:p w14:paraId="${pid}"><w:pPr><w:jc w:val="center"/></w:pPr>` +
      `<w:r><w:rPr><w:noProof/></w:rPr>${dxml}</w:r></w:p>`;
  }

  let right = "";
  const formal = n.bodyFormal.trim();
  const raw = n.bodyRaw.trim();
  let body = stripAsterisks(formal.length > 0 ? formal : raw);
  if (n.closing) {
    const clt = stripAsterisks(n.closing.trim());
    if (clt) {
      const pos = body.indexOf(clt);
      if (pos >= 0) body = body.slice(0, pos) + body.slice(pos + clt.length);
    }
  }

  let any = false;
  for (const paragraph of body.split(/\n\s*\n/)) {
    const text = paragraph
      .split(/\s+/)
      .filter((w) => w.length > 0)
      .join(" ");
    if (text) {
      right += tahomaPara(gen, text, false, opts);
      any = true;
    }
  }
  if (n.closing) {
    const clt = stripAsterisks(n.closing.trim());
    if (clt) {
      if (any) right += emptyPara(gen);
      right += tahomaPara(gen, clt, false, opts);
      any = true;
    }
  }
  if (!any) right += tahomaPara(gen, "(sin texto)", false, opts);

  return (
    `<w:tbl><w:tblPr>${style.tblPrInner}</w:tblPr>` +
    `<w:tblGrid><w:gridCol w:w="${style.grid[0]}"/><w:gridCol w:w="${style.grid[1]}"/></w:tblGrid>` +
    `<w:tr><w:tc><w:tcPr><w:tcW w:w="${style.grid[0]}" w:type="dxa"/></w:tcPr>${left}</w:tc>` +
    `<w:tc><w:tcPr><w:tcW w:w="${style.grid[1]}" w:type="dxa"/></w:tcPr>${right}</w:tc></w:tr></w:tbl>`
  );
}

/* ------------------------------------------------------------------ */
/* Validación                                                          */
/* ------------------------------------------------------------------ */

function validateWellFormed(name: string, xml: string): void {
  const result = XMLValidator.validate(xml);
  if (result !== true) {
    throw new Error(`${name}: XML mal formado: ${JSON.stringify(result)}`);
  }
}

function validateRelationships(doc: string, rels: string): void {
  const defined = new Set([...rels.matchAll(/Id="(rId[^"]+)"/g)].map((m) => m[1]));
  for (const m of doc.matchAll(/r:embed="(rId[^"]+)"/g)) {
    if (!defined.has(m[1])) {
      throw new Error(`La imagen con relación '${m[1]}' no está registrada en document.xml.rels`);
    }
  }
}

function validateParaIds(doc: string): void {
  const seen = new Set<number>();
  for (const m of doc.matchAll(/w14:paraId="([0-9A-Fa-f]{1,8})"/g)) {
    const value = Number.parseInt(m[1], 16);
    if (Number.isNaN(value)) throw new Error(`w14:paraId inválido: ${m[1]}`);
    if (value >= 0x8000_0000) {
      throw new Error(`w14:paraId fuera de rango de 31 bits: ${m[1]}`);
    }
    if (seen.has(value)) throw new Error(`w14:paraId duplicado: ${m[1]}`);
    seen.add(value);
  }
}

/* ------------------------------------------------------------------ */
/* Relaciones e imágenes                                               */
/* ------------------------------------------------------------------ */

interface RelInfo {
  start: number;
  end: number;
  id: string;
  relType: string;
  target: string;
  external: boolean;
}

function parseRelationships(rels: string): RelInfo[] {
  const out: RelInfo[] = [];
  for (const m of rels.matchAll(/<Relationship\b[^>]*>/g)) {
    let id = "";
    let relType = "";
    let target = "";
    let external = false;
    for (const a of m[0].matchAll(/([A-Za-z_:][\w:.-]*)\s*=\s*"([^"]*)"/g)) {
      if (a[1] === "Id") id = a[2];
      else if (a[1] === "Type") relType = a[2];
      else if (a[1] === "Target") target = a[2];
      else if (a[1] === "TargetMode") external = a[2].toLowerCase() === "external";
    }
    if (id)
      out.push({
        start: m.index ?? 0,
        end: (m.index ?? 0) + m[0].length,
        id,
        relType,
        target,
        external,
      });
  }
  return out;
}

function removeUnusedImageRels(rels: string, _doc: string): string {
  // Ya no eliminamos las relaciones de imagen existentes en la plantilla del usuario.
  // El usuario desea que todos los logos, membretes e imágenes de la plantilla se conserven.
  return rels;
}

function normalizeMediaTarget(target: string): string | null {
  let t = target.trim().replace(/^\/+/, "");
  if (t.startsWith("word/")) t = t.slice(5);
  if (t.startsWith("../")) t = t.slice(3);
  return t.startsWith("media/") ? `word/${t}` : null;
}

function collectMediaTargets(rels: string, out: Set<string>): void {
  for (const rel of parseRelationships(rels)) {
    if (rel.external) continue;
    const part = normalizeMediaTarget(rel.target);
    if (part) out.add(part);
  }
}

function referencedMediaParts(rels: string, entries: Map<string, Buffer>): Set<string> {
  const out = new Set<string>();
  collectMediaTargets(rels, out);
  for (const [name, bytes] of entries) {
    if (name.startsWith("word/_rels/") && name.endsWith(".rels")) {
      collectMediaTargets(bytes.toString("utf8"), out);
    }
  }
  return out;
}

function applyVersionProperties(
  entries: Map<string, Buffer>,
  ctype: string,
  version: string,
): string {
  const description = `Generado por Parte Diario v${version}`;
  const coreKey = "docProps/core.xml";
  const existing = entries.get(coreKey)?.toString("utf8");
  let core: string;
  if (existing) {
    const re = /<dc:description>[\s\S]*?<\/dc:description>/;
    core = re.test(existing)
      ? existing
      : insertBefore(
          existing,
          "</cp:coreProperties>",
          `<dc:description>${esc(description)}</dc:description>`,
        );
  } else {
    core =
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
      '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" ' +
      'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" ' +
      'xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">' +
      `<dc:description>${esc(description)}</dc:description></cp:coreProperties>`;
  }
  entries.set(coreKey, Buffer.from(core, "utf8"));

  const corePart = 'PartName="/docProps/core.xml"';
  if (!ctype.includes(corePart)) {
    ctype = insertBefore(
      ctype,
      "</Types>",
      '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>',
    );
  }

  const pkgRels = entries.get("_rels/.rels")?.toString("utf8");
  if (pkgRels && !pkgRels.includes("docProps/core.xml")) {
    entries.set(
      "_rels/.rels",
      Buffer.from(
        insertBefore(
          pkgRels,
          "</Relationships>",
          '<Relationship Id="rIdCoreProps" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>',
        ),
        "utf8",
      ),
    );
  }
  return ctype;
}

/* ------------------------------------------------------------------ */
/* Construcción                                                        */
/* ------------------------------------------------------------------ */

export async function buildReport(params: {
  template?: Buffer | null;
  novedades: Novedad[];
  fechaDesde: string;
  fechaHasta: string;
  media: Map<string, Buffer>;
  options?: Partial<BuildOptions>;
}): Promise<BuildResult> {
  const opts: BuildOptions = { ...DEFAULT_BUILD_OPTIONS, ...params.options };
  const warnings: string[] = [];

  const templateBytes = params.template;
  if (!templateBytes) {
    throw new Error(
      "Es obligatorio proporcionar una plantilla de Word (.docx) cargada por el usuario.",
    );
  }
  const zip = await JSZip.loadAsync(templateBytes);

  const entries = new Map<string, Buffer>();
  for (const file of Object.values(zip.files)) {
    if (file.dir) continue;
    entries.set(file.name, await file.async("nodebuffer"));
  }
  const usedParts = new Set(entries.keys());

  const docEntry = entries.get("word/document.xml");
  if (!docEntry) throw new Error("La plantilla no contiene word/document.xml");
  let doc = docEntry.toString("utf8");
  entries.delete("word/document.xml");

  validateWellFormed("word/document.xml (plantilla)", doc);
  doc = ensureNamespaces(doc);
  validateNamespaces(doc);

  const existing = new Set<number>();
  for (const m of doc.matchAll(/w14:paraId="([0-9A-Fa-f]{1,8})"/g)) {
    const value = Number.parseInt(m[1], 16);
    if (!Number.isNaN(value)) existing.add(value);
  }
  const gen = new ParaIdGen(existing);

  const reFecha =
    /\d{1,2}\s+de\s+(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre)\s*,?\s*\d{4}\.?/i;
  const dateReplace = replaceInParagraphs(doc, reFecha, formatLongEs(params.fechaDesde), 30);
  doc = dateReplace.doc;
  if (dateReplace.count === 0) {
    warnings.push(
      "No se encontró la fecha del encabezado en la plantilla; revísela manualmente en el .docx generado.",
    );
  }

  const rango = `desde las 08:00 horas del día ${formatDdMmYyyy(
    params.fechaDesde,
  )} hasta las 08:00 horas del día ${formatDdMmYyyy(params.fechaHasta)}.`;
  const reRango = /desde las 08:00[^<>\n]*?hasta las 08:00[^<>\n]*?\d{4}[^<>\n.]*\.?/i;
  const rangeReplace = replaceInParagraphs(doc, reRango, rango, Number.MAX_SAFE_INTEGER);
  doc = rangeReplace.doc;
  if (rangeReplace.count === 0) {
    warnings.push(
      'No se encontró el rango del "Asunto" (08:00...hasta las 08:00) en la plantilla; revíselo manualmente.',
    );
  }

  const relsEntry = entries.get("word/_rels/document.xml.rels");
  if (!relsEntry) throw new Error("La plantilla no contiene word/_rels/document.xml.rels");
  const relsOriginal = relsEntry.toString("utf8");
  entries.delete("word/_rels/document.xml.rels");

  const incluidas = params.novedades.filter(
    (n) => n.include && !(n.isOrphan && n.bodyRaw.trim() === "" && n.headers.length === 0),
  );

  const tables = topLevelTables(doc);
  if (tables.length === 0) {
    throw new Error("La plantilla no tiene tabla (membrete) donde anclar las novedades.");
  }
  const firstEnd = tables[0][1];
  const removed = tables.length - 1;
  if (removed > 0) {
    warnings.push(
      `La plantilla traía ${removed} tabla(s) con novedades antiguas: se eliminaron. El reporte solo incluye las novedades del .zip de WhatsApp.`,
    );
  }
  const style = novedadTableStyle(tables[1] ? doc.slice(tables[1][0], tables[1][1]) : undefined);

  const mediaLower = new Map<string, Buffer>();
  for (const [key, value] of params.media) mediaLower.set(key.toLowerCase(), value);

  const images: EmbeddedImage[] = [];
  let docId =
    Math.max(
      0,
      ...[...doc.matchAll(/<wp:docPr\b[^>]*\bid="(\d+)"/g)].map((m) => Number.parseInt(m[1], 10)),
    ) + 1;

  let tablesXml = "";
  for (const n of incluidas) {
    tablesXml += await novedadTable({
      n,
      gen,
      images,
      warnings,
      media: mediaLower,
      docId,
      relsSoFar: relsOriginal,
      usedParts,
      style,
      opts,
    });
    docId += n.photos.length;
  }

  const resumeAt = tables.length > 1 ? tables[tables.length - 1][1] : firstEnd;
  doc = doc.slice(0, firstEnd) + tablesXml + doc.slice(resumeAt);

  const ctypeEntry = entries.get("[Content_Types].xml");
  if (!ctypeEntry) throw new Error("La plantilla no contiene [Content_Types].xml");
  let ctype = ctypeEntry.toString("utf8");
  entries.delete("[Content_Types].xml");

  let rels = relsOriginal;
  for (const img of images) {
    entries.set(img.part, img.bytes);
    const target = img.part.startsWith("word/") ? img.part.slice(5) : img.part;
    rels = insertBefore(
      rels,
      "</Relationships>",
      `<Relationship Id="${img.rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="${target}"/>`,
    );
    const override = `PartName="/${img.part}"`;
    if (!ctype.includes(override)) {
      ctype = insertBefore(
        ctype,
        "</Types>",
        `<Override PartName="/${img.part}" ContentType="${img.contentType}"/>`,
      );
    }
  }

  rels = removeUnusedImageRels(rels, doc);
  const referenced = referencedMediaParts(rels, entries);
  const beforeMedia = entries.size;
  for (const name of [...entries.keys()]) {
    if (name.startsWith("word/media/") && !referenced.has(name)) entries.delete(name);
  }
  const removedMedia = beforeMedia - entries.size;
  if (removedMedia > 0) {
    warnings.push(
      `Se eliminaron ${removedMedia} imagen(es) de la plantilla que ya no estaban referenciadas por el documento.`,
    );
  }

  ctype = applyVersionProperties(entries, ctype, opts.appVersion);
  entries.set("word/_rels/document.xml.rels", Buffer.from(rels, "utf8"));
  entries.set("[Content_Types].xml", Buffer.from(ctype, "utf8"));
  entries.set("word/document.xml", Buffer.from(doc, "utf8"));

  validateWellFormed("word/document.xml", doc);
  validateNamespaces(doc);
  validateWellFormed("word/_rels/document.xml.rels", rels);
  validateWellFormed("[Content_Types].xml", ctype);
  validateRelationships(doc, rels);
  validateParaIds(doc);

  const outZip = new JSZip();
  for (const name of [...entries.keys()].sort()) {
    outZip.file(name, entries.get(name)!);
  }
  const data = await outZip.generateAsync({
    type: "nodebuffer",
    compression: "DEFLATE",
    compressionOptions: { level: 6 },
  });

  return {
    data,
    warnings,
    imagesEmbedded: images.length,
    novedadesIncluidas: incluidas.length,
  };
}
