import { eq } from "drizzle-orm";
import { db } from "../db/client";
import { appSettings } from "../db/schema";

export type SettingsRow = typeof appSettings.$inferSelect;

export async function getOrCreateSettings(userId: string): Promise<SettingsRow> {
  const existing = await db
    .select()
    .from(appSettings)
    .where(eq(appSettings.userId, userId))
    .limit(1);
  if (existing[0]) return existing[0];

  const inserted = await db
    .insert(appSettings)
    .values({
      id: crypto.randomUUID(),
      userId,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    })
    .onConflictDoNothing()
    .returning();
  if (inserted[0]) return inserted[0];

  const retry = await db.select().from(appSettings).where(eq(appSettings.userId, userId)).limit(1);
  return retry[0];
}
