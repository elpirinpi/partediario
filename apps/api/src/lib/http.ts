import type { Context, MiddlewareHandler } from "hono";
import { auth } from "../auth";
import type { AuthUser } from "@parte/shared";

export interface AppVariables {
  user: AuthUser;
  session: { id: string };
}

export type AppContext = Context<{ Variables: AppVariables }>;

export const requireAuth: MiddlewareHandler<{ Variables: AppVariables }> = async (c, next) => {
  const session = await auth.api.getSession({ headers: c.req.raw.headers });
  if (!session?.user) {
    return c.json({ error: "No autenticado" }, 401);
  }
  c.set("user", session.user as unknown as AuthUser);
  c.set("session", { id: session.session.id });
  await next();
};

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
