import { Hono } from "hono";
import { z } from "zod";
import { eq } from "drizzle-orm";
import { db } from "../db/client";
import { appSettings } from "../db/schema";
import { getOrCreateSettings } from "../services/settings";
import { decryptSecret, encryptSecret, previewSecret } from "../lib/secrets";
import { PROVIDER_DEFAULT_MODEL } from "../services/ai";
import type { AppVariables } from "../lib/http";
import type { AppSettings, LlmProvider } from "@parte/shared";

const settingsInput = z.object({
  provider: z.enum(["openrouter", "anthropic", "freellmapi"]).optional(),
  model: z.string().min(1).max(200).optional(),
  apiKey: z.string().max(500).optional(),
  anonymize: z.boolean().optional(),
  defaultTemplateId: z.string().uuid().nullable().optional(),
});

export const settingsRoutes = new Hono<{ Variables: AppVariables }>();

settingsRoutes.get("/", async (c) => {
  const user = c.get("user");
  const row = await getOrCreateSettings(user.id);
  const secret = decryptSecret(row.apiKeyEncrypted);
  const payload: AppSettings = {
    provider: row.provider as LlmProvider,
    model: row.model,
    apiKeySet: secret.length > 0,
    apiKeyPreview: previewSecret(secret),
    anonymize: row.anonymize,
    defaultTemplateId: row.defaultTemplateId,
  };
  return c.json(payload);
});

settingsRoutes.put("/", async (c) => {
  const user = c.get("user");
  const parsed = settingsInput.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) {
    return c.json({ error: "Datos de ajustes inválidos", detail: parsed.error.issues }, 400);
  }
  const input = parsed.data;
  const row = await getOrCreateSettings(user.id);

  const provider = (input.provider ?? row.provider) as LlmProvider;
  let model = input.model ?? row.model;
  if (input.provider && !input.model && provider !== row.provider) {
    model = PROVIDER_DEFAULT_MODEL[provider];
  }

  let apiKeyEncrypted = row.apiKeyEncrypted;
  if (input.apiKey !== undefined) {
    apiKeyEncrypted = input.apiKey.trim() ? encryptSecret(input.apiKey.trim()) : null;
  }

  await db
    .update(appSettings)
    .set({
      provider,
      model,
      apiKeyEncrypted,
      anonymize: input.anonymize ?? row.anonymize,
      defaultTemplateId:
        input.defaultTemplateId === undefined ? row.defaultTemplateId : input.defaultTemplateId,
      updatedAt: new Date().toISOString(),
    })
    .where(eq(appSettings.userId, user.id));

  const secret = decryptSecret(apiKeyEncrypted);
  const payload: AppSettings = {
    provider,
    model,
    apiKeySet: secret.length > 0,
    apiKeyPreview: previewSecret(secret),
    anonymize: input.anonymize ?? row.anonymize,
    defaultTemplateId:
      input.defaultTemplateId === undefined ? row.defaultTemplateId : input.defaultTemplateId,
  };
  return c.json(payload);
});
