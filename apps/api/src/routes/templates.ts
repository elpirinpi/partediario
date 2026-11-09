import { Hono } from "hono";
import { and, desc, eq } from "drizzle-orm";
import { unlink } from "node:fs/promises";
import { db } from "../db/client";
import { appSettings, templates } from "../db/schema";
import { saveTemplateFile } from "../lib/storage";
import { getOrCreateSettings } from "../services/settings";
import type { AppVariables } from "../lib/http";
import type { TemplateSummary } from "@parte/shared";

export const templateRoutes = new Hono<{ Variables: AppVariables }>();

templateRoutes.get("/", async (c) => {
  const user = c.get("user");
  const settings = await getOrCreateSettings(user.id);
  const rows = await db
    .select()
    .from(templates)
    .where(eq(templates.userId, user.id))
    .orderBy(desc(templates.createdAt));

  const items: TemplateSummary[] = rows.map((row) => ({
    id: row.id,
    name: row.name,
    filename: row.filename,
    isDefault: settings.defaultTemplateId === row.id,
    createdAt: row.createdAt,
  }));
  return c.json(items);
});

templateRoutes.post("/", async (c) => {
  const user = c.get("user");
  const body = await c.req.parseBody();
  const file = body["file"];
  if (!file || typeof file === "string" || !("arrayBuffer" in file)) {
    return c.json({ error: "Adjunte un archivo .docx en el campo 'file'." }, 400);
  }
  const bytes = Buffer.from(await file.arrayBuffer());
  if (bytes.length < 4 || bytes[0] !== 0x50 || bytes[1] !== 0x4b) {
    return c.json({ error: "El archivo no parece un .docx válido." }, 400);
  }

  const stored = await saveTemplateFile(file.name || "plantilla.docx", bytes);
  const rows = await db
    .insert(templates)
    .values({
      id: stored.id, // Usar el ID generado por saveTemplateFile
      userId: user.id,
      name: (file.name || "Plantilla").replace(/\.docx$/i, ""),
      filename: stored.filename,
      storagePath: stored.storagePath,
      createdAt: new Date().toISOString(),
    })
    .returning();

  const settings = await getOrCreateSettings(user.id);
  if (!settings.defaultTemplateId) {
    await db
      .update(appSettings)
      .set({ defaultTemplateId: rows[0].id, updatedAt: new Date().toISOString() })
      .where(eq(appSettings.userId, user.id));
  }

  const item: TemplateSummary = {
    id: rows[0].id,
    name: rows[0].name,
    filename: rows[0].filename,
    isDefault: !settings.defaultTemplateId,
    createdAt: rows[0].createdAt, // Ya es un string ISO
  };
  return c.json(item, 201);
});

templateRoutes.patch("/:id/default", async (c) => {
  const user = c.get("user");
  const id = c.req.param("id");
  const found = await db
    .select()
    .from(templates)
    .where(and(eq(templates.id, id), eq(templates.userId, user.id)))
    .limit(1);
  if (!found[0]) return c.json({ error: "Plantilla no encontrada." }, 404);
  await db
    .update(appSettings)
    .set({ defaultTemplateId: id, updatedAt: new Date().toISOString() })
    .where(eq(appSettings.userId, user.id));
  return c.json({ ok: true });
});

templateRoutes.delete("/:id", async (c) => {
  const user = c.get("user");
  const id = c.req.param("id");
  const found = await db
    .select()
    .from(templates)
    .where(and(eq(templates.id, id), eq(templates.userId, user.id)))
    .limit(1);
  if (!found[0]) return c.json({ error: "Plantilla no encontrada." }, 404);
  await db.delete(templates).where(eq(templates.id, id));
  await unlink(found[0].storagePath).catch(() => undefined);

  const settings = await getOrCreateSettings(user.id);
  if (settings.defaultTemplateId === id) {
    await db
      .update(appSettings)
      .set({ defaultTemplateId: null, updatedAt: new Date().toISOString() })
      .where(eq(appSettings.userId, user.id));
  }
  return c.json({ ok: true });
});
