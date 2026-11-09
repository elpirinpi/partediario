#!/usr/bin/env tsx
/**
 * Smoke test: procesa testdata/chat.txt + plantilla_real.docx y genera un
 * .docx validado. No necesita base de datos ni contenedores.
 *
 * Uso: npx tsx scripts/smoke-docx.ts
 */
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import JSZip from "jszip";
import { parseChatTxt } from "../apps/api/src/services/parser";
import { groupNovedades } from "../apps/api/src/services/grouping";
import { buildReport } from "../apps/api/src/services/docx";

const root = process.cwd();

async function main() {
  const chat = await readFile(path.join(root, "testdata/chat.txt"), "utf8");
  const parsed = parseChatTxt(chat);
  console.log("format:", parsed.format, "messages:", parsed.messages.length);

  const novedades = groupNovedades(parsed, new Set(["foto1.png", "foto2.png"]));
  console.log("novedades:", novedades.length);
  for (const n of novedades) {
    console.log(
      " -",
      n.id,
      n.sender,
      "photos:",
      n.photos,
      "videos:",
      n.videosSkipped,
      "fecha:",
      n.fechaMencionada,
    );
    n.bodyFormal = n.bodyRaw;
  }

  const media = new Map<string, Buffer>();
  media.set("foto1.png", await readFile(path.join(root, "testdata/foto1.png")));
  media.set("foto2.png", await readFile(path.join(root, "testdata/foto2.png")));

  const template = await readFile(path.join(root, "testdata/plantilla_real.docx"));
  const result = await buildReport({
    template,
    novedades,
    fechaDesde: "2026-10-10",
    fechaHasta: "2026-10-11",
    media,
  });
  console.log(
    "bytes:",
    result.data.length,
    "images:",
    result.imagesEmbedded,
    "included:",
    result.novedadesIncluidas,
  );
  console.log("warnings:", result.warnings);

  const out = path.join(process.env.TEMP ?? ".", "opencode", "reporte-test.docx");
  await writeFile(out, result.data);

  const zip = await JSZip.loadAsync(result.data);
  const names = Object.keys(zip.files).filter((n) => !zip.files[n].dir);
  console.log("entries:", names.length);
  const doc = await zip.file("word/document.xml")!.async("string");
  console.log("doc xml chars:", doc.length, "has paraId:", /w14:paraId=/.test(doc));
  console.log("output:", out);

  if (result.imagesEmbedded !== 2) {
    throw new Error(
      `Se esperaban 2 imágenes incrustadas, se incrustaron ${result.imagesEmbedded}.`,
    );
  }
  if (result.novedadesIncluidas !== 1) {
    throw new Error(`Se esperaba 1 novedad incluida, se incluyeron ${result.novedadesIncluidas}.`);
  }
  console.log("OK: smoke test pasó.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
