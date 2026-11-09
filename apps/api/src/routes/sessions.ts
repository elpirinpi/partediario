import { Hono } from "hono";
import { z } from "zod";
import { and, desc, eq, or } from "drizzle-orm";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { db } from "../db/client";
import { chatSessions, historyEntries, reports, templates } from "../db/schema";
import { processWhatsappZip, applyPreselection } from "../services/zip";
import { refreshMetadata } from "../services/grouping";
import { cleanupOffline, formalizeWithPrivacy } from "../services/ai";
import { getOrCreateSettings } from "../services/settings";
import { decryptSecret } from "../lib/secrets";
import { readSessionMedia, reportFileName, saveReportFile, saveSessionMedia } from "../lib/storage";
import { addDays, formatDdMmYyyy, parseYmd, ymd } from "../lib/dates";
import type { AppVariables } from "../lib/http";
import type {
  ChatFormat,
  ChatSessionSummary,
  GenerateRequest,
  Novedad,
  ReportResult,
  SessionWarning,
} from "@parte/shared";

const novedadSchema = z.object({
  id: z.number().int(),
  headers: z.array(z.string()),
  bodyRaw: z.string(),
  closing: z.string().nullable(),
  photos: z.array(z.string()),
  videosSkipped: z.array(z.string()),
  fechaMencionada: z.string().nullable(),
  include: z.boolean(),
  isOrphan: z.boolean(),
  bodyFormal: z.string(),
  sender: z.string(),
});

const patchSchema = z.object({
  novedades: z.array(novedadSchema),
  dateFrom: z.string().nullable().optional(),
  dateTo: z.string().nullable().optional(),
});

const formalizeSchema = z.object({
  ids: z.array(z.number().int()).min(1),
  useAi: z.boolean(),
});

const generateSchema = z.object({
  dateFrom: z.string(),
  dateTo: z.string(),
  templateId: z.string().nullable().optional(),
  confirmReplace: z.boolean().optional(),
  novedades: z.array(novedadSchema),
});

export const sessionRoutes = new Hono<{ Variables: AppVariables }>();

type SessionRow = typeof chatSessions.$inferSelect;

function toSummary(row: SessionRow): ChatSessionSummary {
  return {
    id: row.id,
    zipName: row.zipName,
    zipSha256: row.zipSha256,
    txtSha256: row.txtSha256,
    txtName: row.txtName,
    format: row.format as ChatFormat,
    messageCount: row.messageCount,
    createdAt: row.createdAt,
    dateFrom: row.dateFrom,
    dateTo: row.dateTo,
    novedades: JSON.parse(row.payload),
    media: JSON.parse(row.mediaFiles),
    warnings: JSON.parse(row.warnings),
    duplicateOf: null,
  };
}

async function loadSession(id: string, userId: string): Promise<SessionRow | null> {
  const rows = await db
    .select()
    .from(chatSessions)
    .where(and(eq(chatSessions.id, id), eq(chatSessions.userId, userId)))
    .limit(1);
  return rows[0] ?? null;
}

function daysBetween(a: string, b: string): number {
  const first = parseYmd(a);
  const second = parseYmd(b);
  if (!first || !second) return 999;
  return Math.round((first.getTime() - second.getTime()) / 86_400_000);
}

sessionRoutes.post("/", async (c) => {
  const user = c.get("user");
  const body = await c.req.parseBody();
  const file = body["file"];
  if (!file || typeof file === "string" || !("arrayBuffer" in file)) {
    return c.json({ error: "Adjunte el .zip exportado de WhatsApp en 'file'." }, 400);
  }
  const buffer = Buffer.from(await file.arrayBuffer());
  const zipName = file.name || "chat.zip";

  let processed;
  try {
    processed = await processWhatsappZip(buffer, zipName);
  } catch (error) {
    return c.json({ error: error instanceof Error ? error.message : String(error) }, 400);
  }

  const lastRows = await db
    .select({ reportEnd: historyEntries.reportEnd })
    .from(historyEntries)
    .where(eq(historyEntries.userId, user.id))
    .orderBy(desc(historyEntries.reportEnd))
    .limit(1);
  const lastEnd = lastRows[0]?.reportEnd ?? null;

  const { newlyIncluded } = applyPreselection(processed.novedades, lastEnd);

  const duplicateRows = await db
    .select()
    .from(historyEntries)
    .where(
      and(
        eq(historyEntries.userId, user.id),
        or(
          eq(historyEntries.zipSha256, processed.zipSha256),
          eq(historyEntries.txtSha256, processed.txtSha256),
        ),
      ),
    )
    .limit(1);
  const duplicate = duplicateRows[0] ?? null;

  const warnings: SessionWarning[] = [...processed.warnings];

  if (duplicate) {
    warnings.push({
      kind: "warning",
      message: `¡ATENCIÓN! Este .zip (o su contenido de texto) YA fue procesado el ${new Date(duplicate.generatedAt).toISOString()} y generó el reporte ${formatDdMmYyyy(
        duplicate.reportStart,
      )} → ${formatDdMmYyyy(
        duplicate.reportEnd,
      )} (${duplicate.novedades} novedades). Si lo regenera, verifique que no esté duplicando trabajo.`,
    });
  }

  if (lastEnd) {
    warnings.push({
      kind: "info",
      message: `Pre-selección automática: ${newlyIncluded} de ${
        processed.novedades.length
      } novedades son posteriores al último reporte (${formatDdMmYyyy(
        lastEnd,
      )}). Las anteriores quedaron desmarcadas; puede volver a marcarlas si las necesita.`,
    });
  }

  if (processed.majority) {
    const major = processed.majority;
    const from = formatDdMmYyyy(major);
    const to = formatDdMmYyyy(ymd(addDays(parseYmd(major)!, 1)));
    warnings.push({
      kind: "info",
      message: `Fecha inferida del cuerpo de las notas: ${from} (reporte desde las 08:00 del ${from} hasta las 08:00 del ${to}).`,
    });

    const byDate = new Map<string, number[]>();
    for (const id of processed.outliers) {
      const n = (processed.novedades as Novedad[]).find((item) => item.id === id);
      if (!n?.fechaMencionada) continue;
      const list = byDate.get(n.fechaMencionada) ?? [];
      list.push(id);
      byDate.set(n.fechaMencionada, list);
    }
    for (const [date, ids] of byDate) {
      const fmtIds = ids.map((i) => `#${i}`).join(", ");
      if (Math.abs(daysBetween(date, major)) === 1) {
        warnings.push({
          kind: "info",
          message: `Turno nocturno: ${ids.length} novedad(es) (${fmtIds}) mencionan el ${formatDdMmYyyy(
            date,
          )} (día adyacente a la mayoría, ${from}); probablemente son del mismo operativo que cruzó la medianoche. Verifique que correspondan a este reporte.`,
        });
      } else {
        warnings.push({
          kind: "warning",
          message: `Posible inconsistencia: ${ids.length} novedad(es) (${fmtIds}) mencionan la fecha ${formatDdMmYyyy(
            date,
          )} (distinta a la mayoría, ${from}). ¿Error de dedo del agente? Desmarque las que no correspondan o corrija la fecha en el texto.`,
        });
      }
    }
  }

  const inserted = await db
    .insert(chatSessions)
    .values({
      id: randomUUID(),
      userId: user.id,
      zipName,
      zipSha256: processed.zipSha256,
      txtSha256: processed.txtSha256,
      txtName: processed.txtName,
      format: processed.format,
      messageCount: processed.messageCount,
      payload: JSON.stringify(processed.novedades),
      mediaFiles: JSON.stringify(processed.mediaFiles),
      warnings: JSON.stringify(warnings),
      dateFrom: processed.dateFrom,
      dateTo: processed.dateTo,
      createdAt: new Date().toISOString(),
    })
    .returning();

  const row = inserted[0];
  await saveSessionMedia(row.id, processed.mediaBuffers);

  const summary: ChatSessionSummary = {
    ...toSummary(row),
    duplicateOf: duplicate
      ? {
          reportStart: duplicate.reportStart,
          reportEnd: duplicate.reportEnd,
          generatedAt: new Date(duplicate.generatedAt).toISOString(),
          novedades: duplicate.novedades,
        }
      : null,
  };
  return c.json(summary, 201);
});

sessionRoutes.get("/:id", async (c) => {
  const user = c.get("user");
  const row = await loadSession(c.req.param("id"), user.id);
  if (!row) return c.json({ error: "Sesión no encontrada." }, 404);
  return c.json(toSummary(row));
});

sessionRoutes.patch("/:id", async (c) => {
  const user = c.get("user");
  const row = await loadSession(c.req.param("id"), user.id);
  if (!row) return c.json({ error: "Sesión no encontrada." }, 404);

  const parsed = patchSchema.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) {
    return c.json({ error: "Datos inválidos", detail: parsed.error.issues }, 400);
  }

  const novedades = parsed.data.novedades.map((n) => ({
    ...n,
    bodyFormal: n.bodyFormal ?? "",
  })) as Novedad[];

  const updated = await db
    .update(chatSessions)
    .set({
      payload: JSON.stringify(novedades),
      dateFrom: parsed.data.dateFrom === undefined ? row.dateFrom : parsed.data.dateFrom,
      dateTo: parsed.data.dateTo === undefined ? row.dateTo : parsed.data.dateTo,
    })
    .where(eq(chatSessions.id, row.id))
    .returning();

  return c.json(toSummary(updated[0]));
});

sessionRoutes.post("/:id/formalize", async (c) => {
  const user = c.get("user");
  const row = await loadSession(c.req.param("id"), user.id);
  if (!row) return c.json({ error: "Sesión no encontrada." }, 404);

  const parsed = formalizeSchema.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) {
    return c.json({ error: "Datos inválidos", detail: parsed.error.issues }, 400);
  }

  const settings = await getOrCreateSettings(user.id);
  const apiKey = decryptSecret(settings.apiKeyEncrypted);
  if (parsed.data.useAi && !apiKey) {
    return c.json(
      {
        error:
          "No hay API key configurada. Use la limpieza sin conexión o configure su key en Ajustes.",
      },
      400,
    );
  }

  const novedades = JSON.parse(row.payload) as Novedad[];
  const results: Array<{ id: number; bodyFormal: string; error: string | null }> = [];

  for (const id of parsed.data.ids) {
    const novedad = novedades.find((n) => n.id === id);
    if (!novedad) {
      results.push({ id, bodyFormal: "", error: "Novedad no encontrada." });
      continue;
    }
    if (!novedad.bodyRaw.trim()) {
      results.push({ id, bodyFormal: novedad.bodyFormal, error: null });
      continue;
    }
    try {
      const text = parsed.data.useAi
        ? await formalizeWithPrivacy(
            apiKey,
            settings.provider as "openrouter" | "anthropic" | "freellmapi",
            settings.model,
            novedad.bodyRaw,
            settings.anonymize,
          )
        : cleanupOffline(novedad.bodyRaw);
      novedad.bodyFormal = text;
      results.push({ id, bodyFormal: text, error: null });
      if (parsed.data.useAi && parsed.data.ids.length > 1) {
        await new Promise((resolve) => setTimeout(resolve, 1500));
      }
    } catch (error) {
      results.push({
        id,
        bodyFormal: novedad.bodyFormal,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  await db
    .update(chatSessions)
    .set({ payload: JSON.stringify(novedades) })
    .where(eq(chatSessions.id, row.id));

  return c.json(results);
});

sessionRoutes.post("/:id/reports", async (c) => {
  const user = c.get("user");
  const row = await loadSession(c.req.param("id"), user.id);
  if (!row) return c.json({ error: "Sesión no encontrada." }, 404);

  const parsed = generateSchema.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) {
    return c.json({ error: "Datos inválidos", detail: parsed.error.issues }, 400);
  }
  const input = parsed.data as GenerateRequest;

  const from = parseYmd(input.dateFrom);
  const to = parseYmd(input.dateTo);
  if (!from) return c.json({ error: "La fecha 'desde' no es válida (DD-MM-AAAA)." }, 400);
  if (!to) return c.json({ error: "La fecha 'hasta' no es válida (DD-MM-AAAA)." }, 400);
  if (to.getTime() < from.getTime()) {
    return c.json({ error: "La fecha 'hasta' es anterior a la fecha 'desde'." }, 400);
  }
  if (!input.novedades.some((n) => n.include)) {
    return c.json({ error: "No hay ninguna novedad incluida." }, 400);
  }

  const novedades = input.novedades as Novedad[];
  for (const n of novedades) refreshMetadata(n);

  const existing = await db
    .select()
    .from(historyEntries)
    .where(
      and(
        eq(historyEntries.userId, user.id),
        eq(historyEntries.reportStart, input.dateFrom),
        eq(historyEntries.reportEnd, input.dateTo),
      ),
    )
    .limit(1);

  if (existing[0] && !input.confirmReplace) {
    return c.json(
      {
        error: `Ya existe un reporte para ${formatDdMmYyyy(
          input.dateFrom,
        )} → ${formatDdMmYyyy(input.dateTo)}. Confirme para reemplazarlo.`,
        existing: {
          generatedAt: existing[0].generatedAt,
          outputPath: existing[0].outputPath,
        },
      },
      409,
    );
  }

  let templateBytes: Buffer | null = null;
  if (input.templateId) {
    const found = await db
      .select()
      .from(templates)
      .where(and(eq(templates.id, input.templateId), eq(templates.userId, user.id)))
      .limit(1);
    if (!found[0]) return c.json({ error: "Plantilla no encontrada." }, 404);
    templateBytes = await readFile(found[0].storagePath);
  }

  const included = novedades.filter((n) => n.include);
  const media = new Map<string, Buffer>();
  const seen = new Set<string>();
  for (const n of included) {
    for (const photo of n.photos) {
      const key = photo.toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      const bytes = await readSessionMedia(row.id, key);
      if (bytes) media.set(key, bytes);
    }
  }

  let built;
  try {
    const { buildReport } = await import("../services/docx");
    built = await buildReport({
      template: templateBytes ?? undefined,
      novedades,
      fechaDesde: input.dateFrom,
      fechaHasta: input.dateTo,
      media,
    });
  } catch (error) {
    return c.json(
      {
        error: `GENERAR: ${error instanceof Error ? error.message : String(error)}`,
      },
      400,
    );
  }

  const filename = reportFileName(input.dateFrom, input.dateTo);
  const storagePath = await saveReportFile(filename, built.data);
  const outputPath = path.relative(process.cwd(), storagePath);

  const inserted = await db
    .insert(reports)
    .values({
      id: randomUUID(),
      userId: user.id,
      sessionId: row.id,
      filename,
      storagePath,
      reportStart: input.dateFrom,
      reportEnd: input.dateTo,
      novedades: built.novedadesIncluidas,
      images: built.imagesEmbedded,
      warnings: JSON.stringify(built.warnings),
      createdAt: new Date().toISOString(),
    })
    .returning();

  await db
    .delete(historyEntries)
    .where(
      and(
        eq(historyEntries.userId, user.id),
        eq(historyEntries.reportStart, input.dateFrom),
        eq(historyEntries.reportEnd, input.dateTo),
      ),
    );

  await db.insert(historyEntries).values({
    id: randomUUID(),
    userId: user.id,
    sessionId: row.id,
    zipName: row.zipName,
    zipSha256: row.zipSha256,
    txtSha256: row.txtSha256,
    reportStart: input.dateFrom,
    reportEnd: input.dateTo,
    outputPath,
    generatedAt: new Date().toISOString(),
    novedades: built.novedadesIncluidas,
  });

  await db
    .update(chatSessions)
    .set({ payload: JSON.stringify(novedades), dateFrom: input.dateFrom, dateTo: input.dateTo })
    .where(eq(chatSessions.id, row.id));

  const result: ReportResult = {
    id: inserted[0].id,
    filename,
    reportStart: input.dateFrom,
    reportEnd: input.dateTo,
    novedadesIncluidas: built.novedadesIncluidas,
    imagesEmbedded: built.imagesEmbedded,
    warnings: built.warnings,
    downloadUrl: `/api/reports/${inserted[0].id}/download`,
  };
  return c.json(result, 201);
});

sessionRoutes.get("/:id/media/:filename", async (c) => {
  const user = c.get("user");
  const row = await loadSession(c.req.param("id"), user.id);
  if (!row) return c.json({ error: "Sesión no encontrada." }, 404);

  const bytes = await readSessionMedia(row.id, c.req.param("filename"));
  if (!bytes) return c.json({ error: "Archivo no encontrado." }, 404);

  const ext = path.extname(c.req.param("filename")).toLowerCase();
  const contentType =
    ext === ".png"
      ? "image/png"
      : ext === ".gif"
        ? "image/gif"
        : ext === ".webp"
          ? "image/webp"
          : ext === ".bmp"
            ? "image/bmp"
            : "image/jpeg";
  c.header("Content-Type", contentType);
  c.header("Cache-Control", "private, max-age=86400");
  return c.newResponse(new Uint8Array(bytes), 200);
});
