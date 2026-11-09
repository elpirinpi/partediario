import type { LlmProvider } from "@parte/shared";
import { Anonymizer } from "./privacy";

export const SYSTEM_PROMPT = `Eres un editor experto en reportes policiales de la República Dominicana. Tu tarea es corregir y formalizar textos para el 'Parte Diario'.
IDIOMA: Español (Castellano).

REGLAS CRÍTICAS DE ORTOGRAFÍA Y GRAMÁTICA:
1. Aplica estrictamente las reglas de ortografía de la RAE.
2. Presta especial atención a las tildes (acentuación gráfica), incluyendo palabras agudas, llanas, esdrújulas y tildes diacríticas.
3. Corrige la puntuación para asegurar la claridad y fluidez del reporte.
4. Corrige concordancias de género y número, y tiempos verbales.

REGLAS DE ESTILO Y FORMATO:
5. Reescribe el texto en un tono técnico-formal, profesional y en tercera persona.
6. Preserva EXACTAMENTE (no los toques) los datos factuales: nombres propios, números de cédula, placas, teléfonos, fechas, horas, montos y direcciones.
7. NO añadas información, suposiciones o adjetivos innecesarios.
8. NO elimines información relevante del suceso.
9. RESPUESTA: Entrega ÚNICAMENTE el texto corregido. No incluyas introducciones, explicaciones, ni formatos (negritas, asteriscos, etc.). Solo texto limpio.
10. PROHIBIDO mostrar tu razonamiento, análisis, borradores, pasos o etiquetas como 'Thinking Process', 'Análisis', 'Input', 'Output' o 'Respuesta final'. No empieces con 'Aquí está el texto'. Devuelve solo el párrafo final listo para el informe.`;

const ANTHROPIC_URL = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION = "2023-06-01";
const OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions";
const FREELLMAPI_URL = "http://127.0.0.1:31415/v1/chat/completions";

export const PROVIDER_DEFAULT_MODEL: Record<LlmProvider, string> = {
  openrouter: "google/gemma-4-31b-it:free",
  anthropic: "claude-3-5-sonnet-latest",
  freellmapi: "auto",
};

export const PROVIDER_LABEL: Record<LlmProvider, string> = {
  openrouter: "OpenRouter (modelos gratis :free)",
  anthropic: "Anthropic (Claude, de pago)",
  freellmapi: "FreeLLMApi (Local)",
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export async function formalize(
  apiKey: string,
  provider: LlmProvider,
  model: string,
  text: string,
): Promise<string> {
  let lastError = "";
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const raw =
        provider === "anthropic"
          ? await formalizeWithClaude(apiKey, model, text)
          : provider === "openrouter"
            ? await formalizeWithOpenRouter(apiKey, model, text)
            : await formalizeWithFreeLlmApi(apiKey, model, text);
      return sanitizeModelOutput(raw);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (isRetryable(message) && attempt < 2) {
        lastError = message;
        await sleep(2000 * (attempt + 1));
      } else {
        throw error instanceof Error ? error : new Error(message);
      }
    }
  }
  throw new Error(lastError || "No se pudo formalizar el texto.");
}

export async function formalizeWithPrivacy(
  apiKey: string,
  provider: LlmProvider,
  model: string,
  text: string,
  anonymize: boolean,
): Promise<string> {
  if (!anonymize) return formalize(apiKey, provider, model, text);
  const anonymizer = new Anonymizer();
  const cleanInput = anonymizer.anonymize(text);
  const output = await formalize(apiKey, provider, model, cleanInput);
  return anonymizer.restore(output);
}

function isRetryable(error: string): boolean {
  const e = error.toLowerCase();
  return ["429", "502", "503", "504", "timeout", "timed out", "error de red"].some((needle) =>
    e.includes(needle),
  );
}

export function stripAsterisks(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) =>
      line
        .replace(/\*/g, "")
        .split(/\s+/)
        .filter((w) => w.length > 0)
        .join(" "),
    )
    .join("\n");
}

function removeTaggedBlock(text: string, tag: string): string {
  const open = `<${tag}>`;
  const close = `</${tag}>`;
  let out = text;
  for (;;) {
    const lower = out.toLowerCase();
    const start = lower.indexOf(open);
    if (start < 0) break;
    const relEnd = lower.indexOf(close, start);
    if (relEnd >= 0) {
      out = out.slice(0, start) + " " + out.slice(relEnd + close.length);
    } else {
      out = out.slice(0, start);
      break;
    }
  }
  return out;
}

function stripCodeFence(text: string): string {
  const trimmed = text.trim();
  if (!trimmed.startsWith("```")) return trimmed;
  const inner = trimmed.split("\n").slice(1).join("\n");
  const pos = inner.lastIndexOf("```");
  return (pos >= 0 ? inner.slice(0, pos) : inner).trim();
}

const ANSWER_MARKERS = [
  "respuesta final:",
  "texto final:",
  "texto corregido:",
  "resultado final:",
  "final answer:",
  "corrected text:",
  "final text:",
];

function lastAnswerMarker(text: string): number | null {
  const lower = text.toLowerCase();
  let best: number | null = null;
  for (const marker of ANSWER_MARKERS) {
    const pos = lower.lastIndexOf(marker);
    if (pos >= 0) {
      const end = pos + marker.length;
      if (best === null || end > best) best = end;
    }
  }
  return best;
}

const LEADING_LABELS = [
  "aquí está el texto corregido:",
  "aquí tienes el texto corregido:",
  "texto corregido:",
  "respuesta:",
  "resultado:",
  "output:",
  "texto final:",
  "corrected text:",
  "final answer:",
];

function stripLeadingLabels(text: string): string {
  let out = text.trim();
  for (;;) {
    const lower = out.toLowerCase();
    const label = LEADING_LABELS.find((l) => lower.startsWith(l));
    if (!label) break;
    out = out.slice(label.length).trimStart();
  }
  return out;
}

const REASONING_HEADERS = [
  "thinking process",
  "proceso de pensamiento",
  "cadena de pensamiento",
  "razonamiento:",
  "analysis:",
  "análisis:",
  "let's ",
  "let me ",
  "vamos a analizar",
  "vamos a extraer",
  "the user wants",
  "the input text",
  "we need to",
  "i need to",
  "role:",
  "task:",
  "step 1",
  "paso 1",
  "input:",
  "output:",
  "draft:",
  "borrador:",
  "1. analyze",
  "1. analizar",
  "first, the user",
  "first, i",
  "first, we",
  "primero, el usuario",
  "primero, necesito",
  "el usuario quiere",
];

const REASONING_ANYWHERE = [
  "thinking process:",
  "proceso de pensamiento:",
  "the user wants me to",
  "analyze the request",
  "analyze the input",
  "the input text is",
  "the original text is:",
  "let me draft",
  "i notice several issues",
  "final output should be only",
  "corrected version should",
  "key elements to preserve",
  "vamos a analizar",
  "vamos a extraer",
  "el texto original es:",
  "voy a redactar",
  "necesito corregir",
];

function looksLikeReasoning(text: string): boolean {
  const head = text.trimStart().toLowerCase();
  return (
    REASONING_HEADERS.some((h) => head.startsWith(h)) ||
    REASONING_ANYWHERE.some((h) => head.includes(h))
  );
}

export function sanitizeModelOutput(raw: string): string {
  let text = raw.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  text = text.replace(/\*/g, "");
  for (const tag of ["thinking", "analysis", "reasoning", "thought"]) {
    text = removeTaggedBlock(text, tag);
  }
  text = stripCodeFence(text.trim());

  const marker = lastAnswerMarker(text);
  if (marker !== null) text = text.slice(marker);
  text = stripLeadingLabels(text);
  const cleaned = text
    .trim()
    .replace(/^"+|"+$/g, "")
    .trim();
  if (!cleaned) {
    throw new Error(
      "La IA no devolvió texto utilizable. Reintente o use la limpieza sin conexión.",
    );
  }
  if (looksLikeReasoning(cleaned)) {
    throw new Error(
      "La IA devolvió su razonamiento interno en vez del texto corregido. Cambie a un modelo sin razonamiento (google/gemma-4-31b-it:free) o use la limpieza sin conexión.",
    );
  }
  return stripAsterisks(cleaned);
}

async function postJson(
  url: string,
  headers: Record<string, string>,
  body: unknown,
  providerName: string,
): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 120_000);
  try {
    const resp = await fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json", ...headers },
      body: JSON.stringify(body),
      signal: controller.signal,
    });
    const json = (await resp.json().catch(() => ({}))) as Record<string, unknown>;
    if (!resp.ok) {
      const error = json.error as { message?: string } | string | undefined;
      const detail = typeof error === "string" ? error : (error?.message ?? "error desconocido");
      if (resp.status === 429 && providerName === "OpenRouter") {
        throw new Error(
          `Límite del plan gratuito alcanzado (modelos :free: ~20/min, ~50/día). Espere un rato o pruebe otro modelo :free. Detalle: ${detail}`,
        );
      }
      throw new Error(`${providerName} devolvió ${resp.status}: ${detail}`);
    }
    return json;
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") {
      throw new Error(`Error de red al contactar ${providerName}: timeout`, { cause: error });
    }
    if (error instanceof Error && error.message.includes(providerName)) throw error;
    throw new Error(
      `Error de red al contactar ${providerName}: ${
        error instanceof Error ? error.message : String(error)
      }`,
      { cause: error },
    );
  } finally {
    clearTimeout(timeout);
  }
}

export async function formalizeWithClaude(
  apiKey: string,
  model: string,
  text: string,
): Promise<string> {
  if (!apiKey.trim()) {
    throw new Error(
      "No hay API key configurada. Usa el modo sin conexión o configura tu key en Ajustes.",
    );
  }
  if (!text.trim()) throw new Error("Texto vacío, nada que formalizar.");

  const json = (await postJson(
    ANTHROPIC_URL,
    {
      "x-api-key": apiKey.trim(),
      "anthropic-version": ANTHROPIC_VERSION,
    },
    {
      model,
      max_tokens: 2048,
      system: SYSTEM_PROMPT,
      messages: [{ role: "user", content: `Texto a reescribir:\n\n${text}` }],
    },
    "Anthropic",
  )) as { content?: Array<{ text?: string }> };

  const out = json.content?.[0]?.text?.trim();
  if (!out) throw new Error("La API no devolvió texto utilizable.");
  return out;
}

export async function formalizeWithOpenRouter(
  apiKey: string,
  model: string,
  text: string,
): Promise<string> {
  if (!apiKey.trim()) {
    throw new Error(
      "No hay API key de OpenRouter configurada. Consiga una gratis en https://openrouter.ai/keys y péguela en Ajustes.",
    );
  }
  if (!text.trim()) throw new Error("Texto vacío, nada que formalizar.");

  const json = await postJson(
    OPENROUTER_URL,
    { Authorization: `Bearer ${apiKey.trim()}`, "X-Title": "Parte Diario P.N." },
    {
      model,
      max_tokens: 2048,
      temperature: 0.2,
      reasoning: { enabled: false, exclude: true },
      messages: [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content: `Texto a reescribir:\n\n${text}` },
      ],
    },
    "OpenRouter",
  );
  return parseOpenAiResponse(json);
}

export async function formalizeWithFreeLlmApi(
  apiKey: string,
  model: string,
  text: string,
): Promise<string> {
  if (!apiKey.trim()) {
    throw new Error(
      "No hay API key configurada para FreeLLMApi. Asegúrate de que el servidor local está corriendo y configura tu key en Ajustes.",
    );
  }
  if (!text.trim()) throw new Error("Texto vacío, nada que formalizar.");

  const json = await postJson(
    FREELLMAPI_URL,
    { Authorization: `Bearer ${apiKey.trim()}` },
    {
      model,
      max_tokens: 2048,
      temperature: 0.2,
      reasoning_effort: "none",
      messages: [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content: `Texto a reescribir:\n\n${text}` },
      ],
    },
    "La API local",
  );
  return parseOpenAiResponse(json);
}

export function parseOpenAiResponse(json: unknown): string {
  const root = json as {
    choices?: Array<{
      message?: {
        content?: string;
        reasoning?: string;
        reasoning_content?: string;
        reasoning_details?: unknown[];
      };
    }>;
  };
  const message = root.choices?.[0]?.message;
  if (!message) throw new Error("La IA no devolvió texto utilizable.");
  const content = (message.content ?? "").trim();
  if (content) return content;

  const hasReasoning =
    Boolean(message.reasoning?.trim()) ||
    Boolean(message.reasoning_content?.trim()) ||
    (Array.isArray(message.reasoning_details) && message.reasoning_details.length > 0);
  if (hasReasoning) {
    throw new Error(
      "La IA solo devolvió su razonamiento interno y ningún texto final. Reintente, cambie de modelo o use la limpieza sin conexión.",
    );
  }
  throw new Error("La IA no devolvió texto utilizable.");
}

export function cleanupOffline(text: string): string {
  const stripped = stripAsterisks(text);
  const lines = stripped.split(/\r?\n/).map((line) =>
    line
      .split(/\s+/)
      .filter((w) => w.length > 0)
      .join(" "),
  );

  const collapsed: string[] = [];
  let blanks = 0;
  for (const line of lines) {
    if (!line) {
      blanks += 1;
      if (blanks <= 1) collapsed.push("");
    } else {
      blanks = 0;
      collapsed.push(line);
    }
  }

  let out = collapsed.join("\n").trim();
  if (!out) return out;

  const chars = [...out];
  let capitalizeNext = true;
  for (let i = 0; i < chars.length; i += 1) {
    const c = chars[i];
    if (capitalizeNext && /\p{L}/u.test(c)) {
      chars[i] = c.toUpperCase();
      capitalizeNext = false;
    }
    if (c === "." || c === "!" || c === "?") capitalizeNext = true;
  }
  out = chars.join("");

  const trimmed = out.trimEnd();
  if (!/[.!?:]$/.test(trimmed)) out = `${trimmed}.`;
  return out;
}
