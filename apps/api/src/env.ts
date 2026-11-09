import "dotenv/config";
import { z } from "zod";

const envSchema = z.object({
  NODE_ENV: z.enum(["development", "production", "test"]).default("development"),
  PORT: z.coerce.number().default(3001),
  DATABASE_URL: z.string().default("sqlite://./data/parte.db"),
  BETTER_AUTH_SECRET: z.string().min(16).default("cambia-este-secreto-en-produccion-32chars"),
  BETTER_AUTH_URL: z.string().default("http://localhost:3001"),
  WEB_ORIGIN: z.string().default("http://localhost:5173"),
  DATA_DIR: z.string().default("./data"),
  /** Permite registrar usuarios nuevos. Para uso interno puede desactivarse. */
  ALLOW_SIGNUP: z
    .enum(["true", "false"])
    .default("true")
    .transform((v) => v === "true"),
  /** Clave usada para cifrar la API key del LLM en la base de datos. */
  SECRETS_KEY: z.string().min(16).default("clave-local-de-cifrado-32-chars!!"),
});

export const env = envSchema.parse(process.env);

export const trustedOrigins = [env.WEB_ORIGIN, "http://localhost:5173", "http://localhost:8080"];
