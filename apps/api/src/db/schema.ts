import { index, int, sqliteTable, text } from "drizzle-orm/sqlite-core";

// En SQLite usamos int para booleanos (0/1)
const boolean = (name: string) => int(name, { mode: "boolean" });

/* ------------------------------------------------------------------ */
/* Better Auth                                                         */
/* ------------------------------------------------------------------ */

export const user = sqliteTable("user", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  email: text("email").notNull().unique(),
  emailVerified: boolean("email_verified").notNull().default(false),
  image: text("image"),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

export const session = sqliteTable(
  "session",
  {
    id: text("id").primaryKey(),
    expiresAt: text("expires_at").notNull(),
    token: text("token").notNull().unique(),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
    ipAddress: text("ip_address"),
    userAgent: text("user_agent"),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
  },
  (t) => [index("session_user_id_idx").on(t.userId)],
);

export const account = sqliteTable(
  "account",
  {
    id: text("id").primaryKey(),
    accountId: text("account_id").notNull(),
    providerId: text("provider_id").notNull(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    accessToken: text("access_token"),
    refreshToken: text("refresh_token"),
    idToken: text("id_token"),
    accessTokenExpiresAt: text("access_token_expires_at"),
    refreshTokenExpiresAt: text("refresh_token_expires_at"),
    scope: text("scope"),
    password: text("password"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (t) => [index("account_user_id_idx").on(t.userId)],
);

export const verification = sqliteTable("verification", {
  id: text("id").primaryKey(),
  identifier: text("identifier").notNull(),
  value: text("value").notNull(),
  expiresAt: text("expires_at").notNull(),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

export const appSettings = sqliteTable("app_settings", {
  id: text("id").primaryKey(), // Usaremos randomUUID() al insertar
  userId: text("user_id")
    .notNull()
    .unique()
    .references(() => user.id, { onDelete: "cascade" }),
  provider: text("provider").notNull().default("openrouter"),
  model: text("model").notNull().default("google/gemma-4-31b-it:free"),
  apiKeyEncrypted: text("api_key_encrypted"),
  anonymize: boolean("anonymize").notNull().default(true),
  defaultTemplateId: text("default_template_id"),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

export const templates = sqliteTable(
  "templates",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    filename: text("filename").notNull(),
    storagePath: text("storage_path").notNull(),
    isDefault: boolean("is_default").notNull().default(false),
    createdAt: text("created_at").notNull(),
  },
  (t) => [index("templates_user_id_idx").on(t.userId)],
);

export const chatSessions = sqliteTable(
  "chat_sessions",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    zipName: text("zip_name").notNull(),
    zipSha256: text("zip_sha256").notNull(),
    txtSha256: text("txt_sha256").notNull(),
    txtName: text("txt_name").notNull(),
    format: text("format").notNull(),
    messageCount: int("message_count").notNull().default(0),
    payload: text("payload").notNull(), // JSON
    mediaFiles: text("media_files").notNull(), // JSON
    warnings: text("warnings").notNull(), // JSON
    dateFrom: text("date_from"),
    dateTo: text("date_to"),
    createdAt: text("created_at").notNull(),
  },
  (t) => [index("chat_sessions_user_id_idx").on(t.userId)],
);

export const historyEntries = sqliteTable(
  "history_entries",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    sessionId: text("session_id").references(() => chatSessions.id, { onDelete: "cascade" }),
    zipName: text("zip_name").notNull(),
    zipSha256: text("zip_sha256").notNull(),
    txtSha256: text("txt_sha256").notNull(),
    reportStart: text("report_start").notNull(),
    reportEnd: text("report_end").notNull(),
    generatedAt: text("generated_at").notNull(),
    outputPath: text("output_path").notNull(),
    novedades: int("novedades").notNull().default(0),
  },
  (t) => [
    index("history_user_id_idx").on(t.userId),
    index("history_zip_hash_idx").on(t.zipSha256),
    index("history_range_idx").on(t.reportStart, t.reportEnd),
  ],
);

export const reports = sqliteTable(
  "reports",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    sessionId: text("session_id").references(() => chatSessions.id, { onDelete: "cascade" }),
    filename: text("filename").notNull(),
    storagePath: text("storage_path").notNull(),
    reportStart: text("report_start").notNull(),
    reportEnd: text("report_end").notNull(),
    novedades: int("novedades").notNull().default(0),
    images: int("images").notNull().default(0),
    warnings: text("warnings").notNull().default("[]"), // JSON
    createdAt: text("created_at").notNull(),
  },
  (t) => [index("reports_user_id_idx").on(t.userId)],
);
