import type { LlmProvider } from "@parte/shared";

export const PROVIDER_OPTIONS: Array<{
  value: LlmProvider;
  label: string;
  defaultModel: string;
  hint: string;
}> = [
  {
    value: "openrouter",
    label: "OpenRouter (modelos gratis :free)",
    defaultModel: "google/gemma-4-31b-it:free",
    hint: "Gratis con límites (~20/min, ~50/día). Cree su key en openrouter.ai/keys.",
  },
  {
    value: "anthropic",
    label: "Anthropic (Claude, de pago)",
    defaultModel: "claude-3-5-sonnet-latest",
    hint: "API directa de Claude. Requiere crédito en la cuenta.",
  },
  {
    value: "freellmapi",
    label: "FreeLLMApi (Local)",
    defaultModel: "auto",
    hint: "Servidor local en http://127.0.0.1:31415. Use el modelo 'auto'.",
  },
];

export function providerLabel(value: string): string {
  return PROVIDER_OPTIONS.find((p) => p.value === value)?.label ?? value;
}
