import { Hono } from "hono";
import { desc, eq } from "drizzle-orm";
import path from "node:path";
import { db } from "../db/client";
import { historyEntries } from "../db/schema";
import type { AppVariables } from "../lib/http";
import type { HistoryEntry } from "@parte/shared";

export const historyRoutes = new Hono<{ Variables: AppVariables }>();

historyRoutes.get("/", async (c) => {
  const user = c.get("user");
  const rows = await db
    .select()
    .from(historyEntries)
    .where(eq(historyEntries.userId, user.id))
    .orderBy(desc(historyEntries.generatedAt))
    .limit(200);

  const items: HistoryEntry[] = rows.map((row) => ({
    id: row.id,
    zipName: row.zipName,
    reportStart: row.reportStart,
    reportEnd: row.reportEnd,
    generatedAt: row.generatedAt,
    novedades: row.novedades,
    filename: path.basename(row.outputPath),
  }));

  let lastEnd: string | null = null;
  for (const row of rows) {
    if (!lastEnd || row.reportEnd > lastEnd) lastEnd = row.reportEnd;
  }

  return c.json({ entries: items, lastEnd });
});
