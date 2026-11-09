export function ymd(date: Date): string {
  const y = date.getUTCFullYear();
  const m = String(date.getUTCMonth() + 1).padStart(2, "0");
  const d = String(date.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function parseYmd(value: string | null | undefined): Date | null {
  if (!value) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
  if (!m) return null;
  const date = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return Number.isNaN(date.getTime()) ? null : date;
}

export function makeUtcDate(year: number, month: number, day: number): Date | null {
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }
  return date;
}

export function addDays(date: Date, days: number): Date {
  const next = new Date(date.getTime());
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

export function addMonths(date: Date, months: number): Date {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + months, date.getUTCDate()));
}

export function compareYmd(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

export function formatDdMmYyyy(value: string | Date): string {
  const date = typeof value === "string" ? parseYmd(value) : value;
  if (!date) return "";
  return `${String(date.getUTCDate()).padStart(2, "0")}-${String(date.getUTCMonth() + 1).padStart(
    2,
    "0",
  )}-${date.getUTCFullYear()}`;
}

export const MESES_ES = [
  "enero",
  "febrero",
  "marzo",
  "abril",
  "mayo",
  "junio",
  "julio",
  "agosto",
  "septiembre",
  "octubre",
  "noviembre",
  "diciembre",
] as const;

export function formatLongEs(value: string | Date): string {
  const date = typeof value === "string" ? parseYmd(value) : value;
  if (!date) return "";
  return `${date.getUTCDate()} de ${MESES_ES[date.getUTCMonth()]}, ${date.getUTCFullYear()}.`;
}

export function nowIso(): string {
  return new Date().toISOString();
}

export function monthNumberFromSpanish(name: string): number | null {
  const n = name.trim().toLowerCase();
  const idx = MESES_ES.indexOf(n as (typeof MESES_ES)[number]);
  if (idx >= 0) return idx + 1;
  if (n === "setiembre") return 9;
  return null;
}
