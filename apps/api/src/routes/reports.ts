import { Hono } from "hono";
import { and, desc, eq } from "drizzle-orm";
import { readFile } from "node:fs/promises";
import { db } from "../db/client";
import { reports } from "../db/schema";
import type { AppVariables } from "../lib/http";

export const reportRoutes = new Hono<{ Variables: AppVariables }>();

reportRoutes.get("/", async (c) => {
  const user = c.get("user");
  const rows = await db
    .select()
    .from(reports)
    .where(eq(reports.userId, user.id))
    .orderBy(desc(reports.createdAt))
    .limit(50);
  return c.json(
    rows.map((row) => ({
      id: row.id,
      filename: row.filename,
      reportStart: row.reportStart,
      reportEnd: row.reportEnd,
      novedades: row.novedades,
      images: row.images,
      createdAt: row.createdAt,
      downloadUrl: `/api/reports/${row.id}/download`,
    })),
  );
});

reportRoutes.get("/:id/download", async (c) => {
  const user = c.get("user");
  const rows = await db
    .select()
    .from(reports)
    .where(and(eq(reports.id, c.req.param("id")), eq(reports.userId, user.id)))
    .limit(1);
  if (!rows[0]) return c.json({ error: "Reporte no encontrado." }, 404);

  let bytes: Buffer;
  try {
    bytes = await readFile(rows[0].storagePath);
  } catch {
    return c.json({ error: "El archivo del reporte ya no está disponible." }, 404);
  }

  c.header(
    "Content-Type",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  );
  c.header(
    "Content-Disposition",
    `attachment; filename="${rows[0].filename.replace(/[^\w.\- ]/g, "_")}"`,
  );
  return c.newResponse(new Uint8Array(bytes), 200);
});
