import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { serveStatic } from "@hono/node-server/serve-static";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
import { env, trustedOrigins } from "./env";
import { auth } from "./auth";
import { requireAuth } from "./lib/http";
import { ensureDataDirs } from "./lib/storage";
import { settingsRoutes } from "./routes/settings";
import { templateRoutes } from "./routes/templates";
import { sessionRoutes } from "./routes/sessions";
import { historyRoutes } from "./routes/history";
import { reportRoutes } from "./routes/reports";
import { errorMessage } from "./lib/http";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webDistPath = path.join(__dirname, "../../web/dist");

const app = new Hono();

app.use("*", logger());

// Configuración de CORS simplificada ya que la API y el Frontend están en el mismo origen
app.use(
  "/api/*", // Aplicar CORS solo a las rutas de la API
  cors({
    origin: (origin) => {
      // Permitir el mismo origen (API y Web)
      if (!origin) return env.WEB_ORIGIN;
      if (trustedOrigins.includes(origin)) return origin;
      return null;
    },
    allowHeaders: ["Content-Type", "Authorization"],
    allowMethods: ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
    exposeHeaders: ["Content-Length", "Content-Disposition"],
    credentials: true,
    maxAge: 86400,
  }),
);

// Rutas de la API
const api = new Hono();

api.get("/health", (c) => c.json({ ok: true, name: "parte-diario-api", version: "3.0.0" }));

api.on(["GET", "POST"], "/auth/*", (c) => auth.handler(c.req.raw));

api.use("/settings/*", requireAuth);
api.use("/settings", requireAuth);
api.use("/templates/*", requireAuth);
api.use("/templates", requireAuth);
api.use("/sessions/*", requireAuth);
api.use("/sessions", requireAuth);
api.use("/history/*", requireAuth);
api.use("/history", requireAuth);
api.use("/reports/*", requireAuth);
api.use("/reports", requireAuth);

api.route("/settings", settingsRoutes);
api.route("/templates", templateRoutes);
api.route("/sessions", sessionRoutes);
api.route("/history", historyRoutes);
api.route("/reports", reportRoutes);

app.route("/api", api);

// Servir archivos estáticos del frontend
app.use("/*", serveStatic({ root: webDistPath }));

// Fallback para SPA (sirve index.html para rutas no manejadas por la API o estáticos)
app.get("*", serveStatic({ path: path.join(webDistPath, "index.html") }));

app.notFound((c) => c.json({ error: "Ruta no encontrada." }, 404));

app.onError((error, c) => {
  console.error("[api] error:", error);
  return c.json({ error: errorMessage(error) }, 500);
});

await ensureDataDirs();

serve({ fetch: app.fetch, port: env.PORT }, (info) => {
  console.log(`Parte Diario API escuchando en http://localhost:${info.port} (${env.NODE_ENV})`);
});
