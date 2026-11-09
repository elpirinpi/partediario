//! Redacción formal de las narrativas.
//!
//! Proveedores soportados:
//! 1. **OpenRouter** (`formalize_with_openrouter`, recomendado): usa su API
//!    key de OpenRouter con un modelo **gratis** (`:free`, sin tarjeta).
//!    Modelo por defecto: `google/gemma-4-31b-it:free` (sin razonamiento).
//!    Límites del plan gratuito: ~20 req/min, ~50 req/día.
//! 2. **Anthropic** (`formalize_with_claude`): API directa de Claude (de pago).
//! 3. **Sin conexión** (`cleanup_offline`): reglas simples y seguras de
//!    limpieza ortográfica/gramatical, sin IA. No altera datos factuales.
//!
//! En todos los modos el texto generado SIEMPRE se muestra al usuario para
//! revisión/edición manual antes de insertarlo en el .docx.

use std::time::Duration;

use parte_diario_core::privacy::Anonymizer;

/// Contrato para cualquier proveedor de redacción formal intercambiable.
pub trait Formalizer {
    fn formalize(&self, text: &str) -> Result<String, String>;
}

/// Adaptador del proveedor configurado que cumple el contrato [`Formalizer`].
pub struct ProviderFormalizer {
    pub api_key: String,
    pub provider: LlmProvider,
    pub model: String,
}

impl Formalizer for ProviderFormalizer {
    fn formalize(&self, text: &str) -> Result<String, String> {
        formalize(&self.api_key, self.provider, &self.model, text)
    }
}

/// Prompt del sistema exigido para la llamada al LLM.
pub const SYSTEM_PROMPT: &str = "Eres un editor experto en reportes policiales de la República Dominicana. Tu tarea es corregir y formalizar textos para el 'Parte Diario'.
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
10. PROHIBIDO mostrar tu razonamiento, análisis, borradores, pasos o etiquetas como 'Thinking Process', 'Análisis', 'Input', 'Output' o 'Respuesta final'. No empieces con 'Aquí está el texto'. Devuelve solo el párrafo final listo para el informe.";

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const FREELLMAPI_URL: &str = "http://127.0.0.1:31415/v1/chat/completions";

/// Proveedor de IA para la redacción formal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    Anthropic,
    OpenRouter,
    FreeLLMApi,
}

impl LlmProvider {
    pub fn from_config(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "openrouter" => LlmProvider::OpenRouter,
            "freellmapi" => LlmProvider::FreeLLMApi,
            _ => LlmProvider::Anthropic,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "anthropic",
            LlmProvider::OpenRouter => "openrouter",
            LlmProvider::FreeLLMApi => "freellmapi",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "Anthropic (Claude, de pago)",
            LlmProvider::OpenRouter => "OpenRouter (modelos gratis :free)",
            LlmProvider::FreeLLMApi => "FreeLLMApi (Local)",
        }
    }

    /// Modelo recomendado por defecto para cada proveedor.
    pub fn default_model(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "claude-3-5-sonnet-latest",
            LlmProvider::OpenRouter => "google/gemma-4-31b-it:free",
            LlmProvider::FreeLLMApi => "auto",
        }
    }
}

/// Punto de entrada único: formaliza con el proveedor configurado.
pub fn formalize(
    api_key: &str,
    provider: LlmProvider,
    model: &str,
    text: &str,
) -> Result<String, String> {
    let mut last_error = String::new();
    for attempt in 0..3 {
        let res = match provider {
            LlmProvider::Anthropic => formalize_with_claude(api_key, model, text),
            LlmProvider::OpenRouter => formalize_with_openrouter(api_key, model, text),
            LlmProvider::FreeLLMApi => formalize_with_freellmapi(api_key, model, text),
        };
        // Algunos modelos filtran su cadena de pensamiento dentro del contenido.
        // Se descarta ese ruido y se rechaza la respuesta si solo trae razonamiento.
        match res.and_then(|t| sanitize_model_output(&t)) {
            Ok(clean) => return Ok(clean),
            Err(e) if is_retryable(&e) && attempt < 2 => {
                last_error = e;
                std::thread::sleep(Duration::from_secs(2 * (attempt as u64 + 1)));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_error)
}

/// Formaliza con anonimización opcional de datos sensibles. Los datos se
/// restauran en la respuesta antes de devolverla a la interfaz.
pub fn formalize_with_privacy(
    api_key: &str,
    provider: LlmProvider,
    model: &str,
    text: &str,
    anonymize: bool,
) -> Result<String, String> {
    if !anonymize {
        return formalize(api_key, provider, model, text);
    }
    let mut anonymizer = Anonymizer::new();
    let clean_input = anonymizer.anonymize(text);
    let output = formalize(api_key, provider, model, &clean_input)?;
    Ok(anonymizer.restore(&output))
}

fn is_retryable(error: &str) -> bool {
    let e = error.to_lowercase();
    [
        "429",
        "502",
        "503",
        "504",
        "timeout",
        "timed out",
        "error de red",
    ]
    .iter()
    .any(|needle| e.contains(needle))
}

/// Quita los marcadores de negrita de WhatsApp/Markdown ("*texto*",
/// "**texto**") que los agentes escriben en el chat y que la IA a veces
/// conserva. En el texto policial los asteriscos no tienen significado,
/// así que se eliminan por completo y se recolan los espacios sueltos.
/// Conserva los saltos de línea y párrafos.
pub fn strip_asterisks(text: &str) -> String {
    // Se recorre con `split('\n')` (no con `lines()`) para conservar el
    // número EXACTO de líneas, incluida una línea final vacía. Con `lines()`,
    // una salida terminada en '\n' perdía esa línea al volver a aplicar la
    // función, de modo que no era idempotente (p. ej. "\n*" -> "\n" -> "").
    let mut out = String::with_capacity(text.len());
    for (i, raw) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let no_star: String = line.chars().filter(|c| *c != '*').collect();
        out.push_str(&no_star.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    out
}

/// Limpia la salida del modelo: quita bloques de razonamiento, vallas de
/// código y etiquetas de cortesía. Si solo devolvió su cadena de pensamiento,
/// falla en vez de insertar texto sin sentido en el informe.
pub fn sanitize_model_output(raw: &str) -> Result<String, String> {
    let mut text = raw.replace("\r\n", "\n").replace('\r', "\n");
    text = text.replace('*', "");
    for tag in ["thinking", "analysis", "reasoning", "thought"] {
        text = remove_tagged_block(&text, tag);
    }
    text = strip_code_fence(text.trim());

    // Si el modelo etiquetó la respuesta, quedarse con lo que sigue a la
    // última etiqueta (algunos modelos repiten "Final Answer" al final).
    if let Some(pos) = last_answer_marker(&text) {
        text = text[pos..].to_string();
    }
    text = strip_leading_labels(&text);
    let cleaned = text.trim().trim_matches('"').trim().to_string();
    if cleaned.is_empty() {
        return Err(
            "La IA no devolvió texto utilizable. Reintente o use la limpieza sin conexión."
                .to_string(),
        );
    }
    if looks_like_reasoning(&cleaned) {
        return Err("La IA devolvió su razonamiento interno en vez del texto corregido. Cambie a un modelo sin razonamiento (google/gemma-4-31b-it:free) o use la limpieza sin conexión.".to_string());
    }
    Ok(strip_asterisks(&cleaned))
}

fn remove_tagged_block(text: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = text.to_string();
    loop {
        let lower = out.to_lowercase();
        let Some(start) = lower.find(&open) else {
            break;
        };
        if let Some(rel_end) = lower[start..].find(&close) {
            let end = start + rel_end + close.len();
            out.replace_range(start..end, " ");
        } else {
            out.truncate(start);
            break;
        }
    }
    out
}

fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut inner = trimmed.lines().skip(1).collect::<Vec<_>>().join("\n");
    if let Some(pos) = inner.rfind("```") {
        inner.truncate(pos);
    }
    inner.trim().to_string()
}

fn last_answer_marker(text: &str) -> Option<usize> {
    const MARKERS: &[&str] = &[
        "respuesta final:",
        "texto final:",
        "texto corregido:",
        "resultado final:",
        "final answer:",
        "corrected text:",
        "final text:",
    ];
    let lower = text.to_lowercase();
    MARKERS
        .iter()
        .filter_map(|m| lower.rfind(m).map(|p| p + m.len()))
        .max()
}

fn strip_leading_labels(text: &str) -> String {
    const LABELS: &[&str] = &[
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
    let mut out = text.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let Some(label) = LABELS.iter().find(|l| lower.starts_with(*l)) else {
            break;
        };
        out = out[label.len()..].trim_start().to_string();
    }
    out
}

fn looks_like_reasoning(text: &str) -> bool {
    const HEADERS: &[&str] = &[
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
    const ANYWHERE: &[&str] = &[
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
    let head = text.trim_start().to_lowercase();
    HEADERS.iter().any(|h| head.starts_with(h)) || ANYWHERE.iter().any(|h| head.contains(h))
}

/// Reescribe `text` en español formal usando la API de Anthropic.
/// Devuelve el texto generado o un mensaje de error legible.
pub fn formalize_with_claude(api_key: &str, model: &str, text: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err(
            "No hay API key configurada. Usa el modo sin conexión o configura tu key en Ajustes."
                .to_string(),
        );
    }
    if text.trim().is_empty() {
        return Err("Texto vacío, nada que formalizar.".to_string());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("No se pudo crear el cliente HTTP: {e}"))?;

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "system": SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": format!("Texto a reescribir:\n\n{}", text) }
        ]
    });

    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key.trim())
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Error de red al contactar Anthropic: {e}"))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("La API devolvió una respuesta inválida: {e}"))?;

    if !status.is_success() {
        let detail = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("error desconocido");
        return Err(format!("La API devolvió {status}: {detail}"));
    }

    json.get("content")
        .and_then(|c| c.get(0))
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "La API no devolvió texto utilizable.".to_string())
}

/// Reescribe `text` usando un modelo de OpenRouter (idealmente uno `:free`,
/// gratuito). La API es compatible con el formato de OpenAI.
pub fn formalize_with_openrouter(api_key: &str, model: &str, text: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("No hay API key de OpenRouter configurada. Consiga una gratis en https://openrouter.ai/keys y péguela en Ajustes.".to_string());
    }
    if text.trim().is_empty() {
        return Err("Texto vacío, nada que formalizar.".to_string());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("No se pudo crear el cliente HTTP: {e}"))?;

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "temperature": 0.2,
        "reasoning": { "enabled": false, "exclude": true },
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": format!("Texto a reescribir:\n\n{}", text) }
        ]
    });

    let resp = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .header("content-type", "application/json")
        .header("X-Title", "Parte Diario P.N.")
        .json(&body)
        .send()
        .map_err(|e| format!("Error de red al contactar OpenRouter: {e}"))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("OpenRouter devolvió una respuesta inválida: {e}"))?;

    if !status.is_success() {
        let detail = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("error desconocido");
        if status.as_u16() == 429 {
            return Err(format!(
                "Límite del plan gratuito alcanzado (modelos :free: ~20/min, ~50/día). Espere un rato o pruebe otro modelo :free. Detalle: {detail}"
            ));
        }
        return Err(format!("OpenRouter devolvió {status}: {detail}"));
    }

    parse_openrouter_response(&json)
}

/// Reescribe `text` usando la API local de FreeLLMApi.
pub fn formalize_with_freellmapi(api_key: &str, model: &str, text: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("No hay API key configurada para FreeLLMApi. Asegúrate de que el servidor local está corriendo y configura tu key en Ajustes.".to_string());
    }
    if text.trim().is_empty() {
        return Err("Texto vacío, nada que formalizar.".to_string());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("No se pudo crear el cliente HTTP: {e}"))?;

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "temperature": 0.2,
        // FreeLLMApi usa el nombre OpenAI de este parámetro; con "none" evita
        // que el modelo enrute su cadena de pensamiento al contenido.
        "reasoning_effort": "none",
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": format!("Texto a reescribir:\n\n{}", text) }
        ]
    });

    let resp = client
        .post(FREELLMAPI_URL)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Error de red al contactar FreeLLMApi local: {e}"))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("La API local devolvió una respuesta inválida: {e}"))?;

    if !status.is_success() {
        let detail = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("error desconocido");
        return Err(format!("La API local devolvió {status}: {detail}"));
    }

    parse_openrouter_response(&json)
}

/// Extrae el texto de una respuesta estilo OpenAI (testeable sin red).
pub fn parse_openrouter_response(json: &serde_json::Value) -> Result<String, String> {
    let message = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|ch| ch.get("message"))
        .ok_or_else(|| "La IA no devolvió texto utilizable.".to_string())?;
    let content = message
        .get("content")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    if !content.is_empty() {
        return Ok(content.to_string());
    }
    let has_reasoning = ["reasoning", "reasoning_content"].iter().any(|k| {
        message
            .get(*k)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty())
    }) || message
        .get("reasoning_details")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    if has_reasoning {
        return Err("La IA solo devolvió su razonamiento interno y ningún texto final. Reintente, cambie de modelo o use la limpieza sin conexión.".to_string());
    }
    Err("La IA no devolvió texto utilizable.".to_string())
}

/// Limpieza sin conexión: reglas conservadoras que NO tocan datos factuales.
/// - Recorta espacios y líneas vacías sobrantes.
/// - Colapsa espacios múltiples.
/// - Capitaliza la primera letra del texto y después de cada punto.
/// - Garantiza punto final.
pub fn cleanup_offline(text: &str) -> String {
    // 0) Quitar los asteriscos de negrita del chat ("**texto**", "*texto*").
    let text = strip_asterisks(text);
    // 1) Normalizar líneas: recortar y colapsar espacios internos.
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        lines.push(l);
    }
    // 2) Colapsar 3+ saltos de línea en 2 (separador de párrafo).
    let mut collapsed: Vec<String> = Vec::new();
    let mut blanks = 0;
    for l in lines {
        if l.is_empty() {
            blanks += 1;
            if blanks <= 1 {
                collapsed.push(String::new());
            }
        } else {
            blanks = 0;
            collapsed.push(l);
        }
    }
    let mut out = collapsed.join("\n").trim().to_string();
    if out.is_empty() {
        return out;
    }

    // 3) Capitalizar inicio del texto y después de ". ", "! ", "? ".
    let mut chars: Vec<char> = out.chars().collect();
    let mut capitalize_next = true;
    for c in &mut chars {
        if capitalize_next && c.is_alphabetic() {
            *c = c.to_uppercase().next().unwrap_or(*c);
            capitalize_next = false;
        }
        if matches!(*c, '.' | '!' | '?') {
            capitalize_next = true;
        }
    }
    out = chars.into_iter().collect();

    // 4) Punto final si falta.
    let t = out.trim_end();
    if !t.ends_with(['.', '!', '?', ':']) {
        out = format!("{t}.");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_cleanup_is_conservative() {
        let raw = "siendo las  20:00 horas del día 10/10/2026,  se incautó la  bocina marca  Koss";
        let clean = cleanup_offline(raw);
        assert!(clean.starts_with("Siendo las 20:00"));
        assert!(clean.contains("10/10/2026"));
        assert!(clean.contains("Koss"));
        assert!(clean.ends_with('.'));
        assert!(!clean.contains("  "));
    }

    #[test]
    fn strip_asterisks_quita_negrita_de_whatsapp() {
        let raw = "**REPÚBLICA DOMINICANA**\n**Policía Nacional**\n\n**Fecha:** 07 de octubre de 2026.\nLugar:* Comandancia Mirador Norte, P.N.";
        let s = strip_asterisks(raw);
        assert!(s.contains("REPÚBLICA DOMINICANA"));
        assert!(s.contains("Policía Nacional"));
        assert!(s.contains("Fecha: 07 de octubre de 2026."));
        assert!(s.contains("Lugar: Comandancia Mirador Norte, P.N."));
        assert!(!s.contains('*'), "no debe quedar ningún asterisco: {s}");
        assert!(s.contains("\n\n"), "conserva los párrafos");
    }

    #[test]
    fn offline_cleanup_quita_asteriscos() {
        let clean = cleanup_offline(
            "**Siendo las 18:00 horas** del día 07/10/2026, se dio *inicio* al operativo.",
        );
        assert!(!clean.contains('*'), "limpieza sin conexión: {clean}");
        assert!(clean.starts_with("Siendo las 18:00 horas"));
        assert!(clean.contains("inicio al operativo"));
    }

    #[test]
    fn openrouter_response_parsing() {
        let v: serde_json::Value = serde_json::json!({
            "choices": [{"message": {"content": "  Texto formal.  "}}]
        });
        assert_eq!(parse_openrouter_response(&v).unwrap(), "Texto formal.");
        let bad = serde_json::json!({"choices": []});
        assert!(parse_openrouter_response(&bad).is_err());
        let only_reasoning = serde_json::json!({
            "choices": [{"message": {"content": "", "reasoning": "Pensando el texto..."}}]
        });
        let err = parse_openrouter_response(&only_reasoning).unwrap_err();
        assert!(err.contains("razonamiento"), "error inesperado: {err}");
    }

    #[test]
    fn provider_defaults() {
        assert_eq!(
            LlmProvider::from_config("openrouter"),
            LlmProvider::OpenRouter
        );
        assert_eq!(
            LlmProvider::from_config("cualquier-otra-cosa"),
            LlmProvider::Anthropic
        );
        assert_eq!(LlmProvider::OpenRouter.as_str(), "openrouter");
        assert!(LlmProvider::OpenRouter.default_model().ends_with(":free"));
    }

    #[test]
    fn endpoints_are_not_mixed() {
        assert!(OPENROUTER_URL.starts_with("https://openrouter.ai/"));
        assert!(FREELLMAPI_URL.starts_with("http://127.0.0.1:31415/"));
        assert_ne!(OPENROUTER_URL, FREELLMAPI_URL);
    }

    #[test]
    fn openrouter_default_is_not_a_reasoning_model() {
        assert_eq!(
            LlmProvider::OpenRouter.default_model(),
            "google/gemma-4-31b-it:free"
        );
    }

    #[test]
    fn sanitize_rechaza_cadena_de_pensamiento() {
        let raw = "Thinking Process:\n\n1. Analyze the Request:\nRole: Expert editor.\nLet's extract the key facts.\n\nInput:\n> \"Todo por la Patria\"";
        let err = sanitize_model_output(raw).unwrap_err();
        assert!(err.contains("razonamiento"), "error inesperado: {err}");
    }

    #[test]
    fn sanitize_rechaza_el_texto_real_del_modelo_actual() {
        let raw = "First, the user wants me to correct and formalize the given text. I need to act as an expert editor in police reports from the Dominican Republic.\n\nThe original text is:\n\"Todo por la Patria\"\n\nI notice several issues: spelling errors, grammar and structure.";
        let err = sanitize_model_output(raw).unwrap_err();
        assert!(err.contains("razonamiento"), "error inesperado: {err}");
    }

    #[test]
    fn sanitize_extrae_la_respuesta_final() {
        let raw = "Thinking Process:\n1. Analyze.\n\nFinal Answer:\nSiendo las 11:59 p.m. del día 30/10/2026, se incautaron dos bocinas.";
        let out = sanitize_model_output(raw).unwrap();
        assert!(out.starts_with("Siendo las 11:59"));
        assert!(!out.contains("Thinking"));
        assert!(!out.contains("Final Answer"));
    }

    #[test]
    fn sanitize_quita_bloques_y_vallas() {
        let raw = "```text\n<thinking>borrador interno</thinking>\nTexto corregido: Se informa el hecho.\n```";
        let out = sanitize_model_output(raw).unwrap();
        assert_eq!(out, "Se informa el hecho.");
    }

    #[test]
    fn reintenta_solo_errores_transitorios() {
        assert!(is_retryable("La API devolvió 429: rate limit"));
        assert!(is_retryable("Error de red al contactar OpenRouter"));
        assert!(!is_retryable("La IA devolvió su razonamiento interno"));
        assert!(!is_retryable("No hay API key configurada"));
    }
}
