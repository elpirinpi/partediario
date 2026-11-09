import Database from "better-sqlite3";
import { drizzle } from "drizzle-orm/better-sqlite3";
import path from "node:path";
import fs from "node:fs";
import * as schema from "./schema";
import { env } from "../env";

const dbPath = env.DATABASE_URL.replace(/^sqlite:\/\//, "");
const resolvedPath = path.resolve(dbPath);
const dir = path.dirname(resolvedPath);
if (!fs.existsSync(dir)) {
  fs.mkdirSync(dir, { recursive: true });
}

const sqlite = new Database(resolvedPath);
export const db = drizzle(sqlite, { schema });
export { sqlite as sqliteClient, schema };
