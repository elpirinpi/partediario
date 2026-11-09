//! Property-based tests para el crate de IA:
//! - Limpieza y saneamiento de texto (`strip_asterisks`, `cleanup_offline`,
//!   `sanitize_model_output`, `parse_openrouter_response`).
//! - Anonimización reversible (`Anonymizer`): anonimizar → restaurar debe
//!   devolver exactamente el texto original y nunca perder datos.

use proptest::prelude::*;

use parte_diario_ai::{
    cleanup_offline, parse_openrouter_response, sanitize_model_output, strip_asterisks, LlmProvider,
};
use parte_diario_core::privacy::Anonymizer;

// ---------------------------------------------------------------------------
// Estrategias
// ---------------------------------------------------------------------------

/// Texto arbitrario (incluye saltos de línea y caracteres raros).
fn arb_any_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"(?s).{0,300}").unwrap()
}

/// Texto "normalizado": palabras sin espacios internos, unidas por un espacio,
/// líneas separadas por un salto de línea. `strip_asterisks` no debe cambiarlo.
fn arb_normalized_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::collection::vec(
            prop::string::string_regex(r"[A-Za-z0-9ÁÉÍÓÚÑáéíóúñ.,:;!?]{1,10}").unwrap(),
            1..=8,
        ),
        1..=6,
    )
    .prop_map(|lines| {
        lines
            .iter()
            .map(|words| words.join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Cédula dominicana con el formato que reconoce la regex (`001-0313301-5`).
fn arb_cedula() -> impl Strategy<Value = String> {
    (100u32..=999, 0u32..=9_999_999, 0u32..=9).prop_map(|(a, b, c)| format!("{a:03}-{b:07}-{c}"))
}

/// Teléfono con prefijo 809/829/849.
fn arb_phone() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["809", "829", "849"]),
        100u32..=999,
        0u32..=9999,
    )
        .prop_map(|(area, mid, last)| format!("{area}-{mid:03}-{last:04}"))
}

fn arb_email() -> impl Strategy<Value = String> {
    (
        prop::string::string_regex(r"[a-z0-9]{1,10}").unwrap(),
        prop::string::string_regex(r"[a-z]{2,8}").unwrap(),
        prop::sample::select(vec!["com", "net", "org", "do"]),
    )
        .prop_map(|(user, domain, tld)| format!("{user}@{domain}.{tld}"))
}

/// Frase con la que la regex de nombres asocia el nombre propio.
fn arb_name_phrase() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec![
            "nombrado",
            "nombrada",
            "señor",
            "señora",
            "ciudadano",
            "ciudadana",
            "propietario",
            "propietaria",
        ]),
        prop::string::string_regex(r"[A-Z][a-z]{2,8}").unwrap(),
        prop::string::string_regex(r"[A-Z][a-z]{2,8}").unwrap(),
    )
        .prop_map(|(trigger, n1, n2)| format!("{trigger} {n1} {n2}"))
}

/// Texto realista que contiene datos sensibles de los cuatro tipos, con al
/// menos uno de ellos presente (para que siempre se generen tokens).
fn arb_sensitive_text() -> impl Strategy<Value = String> {
    (
        1u8..=15,
        arb_cedula(),
        arb_phone(),
        arb_email(),
        arb_name_phrase(),
    )
        .prop_map(|(mask, ced, tel, mail, name)| {
            let mut parts = vec![
                "Siendo las 20:00 horas del día 10/10/2026, se realizó operativo.".to_string(),
            ];
            if mask & 1 != 0 {
                parts.push(format!("Cédula: {ced}."));
            }
            if mask & 2 != 0 {
                parts.push(format!("Teléfono: {tel}."));
            }
            if mask & 4 != 0 {
                parts.push(format!("Correo: {mail}."));
            }
            if mask & 8 != 0 {
                parts.push(format!("{name} en la escena."));
            }
            parts.join(" ")
        })
}

// ---------------------------------------------------------------------------
// strip_asterisks
// ---------------------------------------------------------------------------

proptest! {
    /// La negrita de WhatsApp nunca sobrevive a la limpieza.
    #[test]
    fn strip_asterisks_output_has_no_asterisks(input in arb_any_text()) {
        prop_assert!(!strip_asterisks(&input).contains('*'));
    }

    /// Aplicarla dos veces equivale a aplicarla una vez.
    #[test]
    fn strip_asterisks_is_idempotent(input in arb_any_text()) {
        let once = strip_asterisks(&input);
        prop_assert_eq!(strip_asterisks(&once), once);
    }

    /// Nunca aumenta el tamaño del texto (solo quita `*` y colapsa espacios).
    #[test]
    fn strip_asterisks_never_grows(input in arb_any_text()) {
        let out = strip_asterisks(&input);
        prop_assert!(
            out.chars().count() <= input.chars().count(),
            "creció: {} -> {} caracteres",
            input.chars().count(),
            out.chars().count()
        );
    }

    /// Sobre texto ya normalizado es la identidad.
    #[test]
    fn strip_asterisks_is_identity_on_normalized_text(input in arb_normalized_text()) {
        prop_assert_eq!(strip_asterisks(&input), input);
    }
}

// ---------------------------------------------------------------------------
// cleanup_offline
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn cleanup_offline_output_has_no_asterisks(input in arb_any_text()) {
        prop_assert!(!cleanup_offline(&input).contains('*'));
    }

    #[test]
    fn cleanup_offline_is_idempotent(input in arb_any_text()) {
        let once = cleanup_offline(&input);
        prop_assert_eq!(cleanup_offline(&once), once);
    }

    /// Ninguna línea queda con espacios dobles ni espacios en los extremos.
    #[test]
    fn cleanup_offline_normalizes_line_whitespace(input in arb_any_text()) {
        let out = cleanup_offline(&input);
        for line in out.lines() {
            prop_assert!(!line.starts_with(' '), "línea empieza con espacio: {:?}", line);
            prop_assert!(!line.ends_with(' '), "línea termina con espacio: {:?}", line);
            prop_assert!(!line.contains("  "), "espacios dobles en: {:?}", line);
        }
    }

    /// Todo texto no vacío termina en puntuación de cierre.
    #[test]
    fn cleanup_offline_ends_with_terminal_punctuation(input in arb_any_text()) {
        let out = cleanup_offline(&input);
        if let Some(last) = out.trim_end().chars().last() {
            prop_assert!(
                matches!(last, '.' | '!' | '?' | ':'),
                "termina en {:?}: {:?}",
                last,
                out
            );
        }
    }

    /// La primera letra alfabética queda en su forma mayúscula (algunos
    /// caracteres, como ciertos ideogramas, no tienen mayúscula: se comparan
    /// contra su propia conversión a mayúsculas).
    #[test]
    fn cleanup_offline_capitalizes_first_letter(input in arb_any_text()) {
        let out = cleanup_offline(&input);
        if let Some(c) = out.chars().find(|c| c.is_alphabetic()) {
            let upper = c.to_uppercase().next().unwrap_or(c);
            prop_assert_eq!(
                c,
                upper,
                "primera letra {:?} en minúscula: {:?}",
                c,
                out
            );
        }
    }
}

// ---------------------------------------------------------------------------
// sanitize_model_output
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn sanitize_never_panics(input in arb_any_text()) {
        let _ = sanitize_model_output(&input);
    }

    /// Cuando hay respuesta utilizable, no queda ningún asterisco y no está vacía.
    #[test]
    fn sanitize_ok_output_is_clean(input in arb_any_text()) {
        if let Ok(out) = sanitize_model_output(&input) {
            prop_assert!(!out.contains('*'));
            prop_assert!(!out.trim().is_empty());
        }
    }

    /// Sanear una salida ya saneada no la altera.
    #[test]
    fn sanitize_ok_is_idempotent(input in arb_any_text()) {
        if let Ok(out) = sanitize_model_output(&input) {
            prop_assert_eq!(sanitize_model_output(&out), Ok(out));
        }
    }

    /// Los errores explican qué pasó (razonamiento filtrado o texto vacío).
    #[test]
    fn sanitize_errors_are_descriptive(input in arb_any_text()) {
        if let Err(e) = sanitize_model_output(&input) {
            prop_assert!(
                e.contains("razonamiento") || e.contains("utilizable"),
                "error inesperado: {}",
                e
            );
        }
    }
}

// ---------------------------------------------------------------------------
// parse_openrouter_response
// ---------------------------------------------------------------------------

proptest! {
    /// El contenido (recortado) se devuelve tal cual cuando no está vacío.
    #[test]
    fn parse_content_is_returned_trimmed(content in arb_any_text()) {
        let v = serde_json::json!({
            "choices": [{"message": {"content": content}}]
        });
        let res = parse_openrouter_response(&v);
        let trimmed = content.trim();
        if trimmed.is_empty() {
            prop_assert!(res.is_err());
        } else {
            prop_assert_eq!(res.unwrap(), trimmed.to_string());
        }
    }

    /// Si solo hay razonamiento (no vacío), debe ser un error explicativo.
    #[test]
    fn parse_reasoning_only_is_error(reasoning in "[A-Za-z]{1,80}") {
        let v = serde_json::json!({
            "choices": [{"message": {"content": "", "reasoning": reasoning}}]
        });
        let err = parse_openrouter_response(&v).unwrap_err();
        prop_assert!(err.contains("razonamiento"), "error inesperado: {}", err);
    }
}

/// Estructuras inválidas deben devolver error, no panic.
#[test]
fn parse_invalid_shapes_are_errors() {
    assert!(parse_openrouter_response(&serde_json::json!({"foo": "bar"})).is_err());
    assert!(parse_openrouter_response(&serde_json::json!({"choices": []})).is_err());
    assert!(parse_openrouter_response(
        &serde_json::json!({"choices": [{"message": {"content": 42}}]})
    )
    .is_err());
    // Hay contenido y razonamiento: gana el contenido.
    let v = serde_json::json!({
        "choices": [{"message": {"content": "Texto final.", "reasoning": "pensando"}}]
    });
    assert_eq!(parse_openrouter_response(&v).unwrap(), "Texto final.");
}

// ---------------------------------------------------------------------------
// Anonymizer (anonimizar -> restaurar)
// ---------------------------------------------------------------------------

proptest! {
    /// Anonimizar y restaurar devuelve exactamente el texto original.
    #[test]
    fn anonymizer_roundtrip(text in arb_sensitive_text()) {
        let mut a = Anonymizer::new();
        let clean = a.anonymize(&text);
        prop_assert_eq!(a.restore(&clean), text);
    }

    /// Los datos sensibles desaparecen del texto que se envía a la IA.
    #[test]
    fn anonymizer_hides_sensitive_values(text in arb_sensitive_text()) {
        let mut a = Anonymizer::new();
        let clean = a.anonymize(&text);
        // No debe quedar ningún correo, ni cédula ni teléfono con ese formato.
        let re_email = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap();
        let re_cedula = regex::Regex::new(r"\b\d{3}[- ]\d{7}[- ]\d\b").unwrap();
        let re_phone = regex::Regex::new(r"\b(?:809|829|849)[- ]\d{3}[- ]\d{4}\b").unwrap();
        prop_assert!(!re_email.is_match(&clean), "correo sin anonimizar: {:?}", clean);
        prop_assert!(!re_cedula.is_match(&clean), "cédula sin anonimizar: {:?}", clean);
        prop_assert!(!re_phone.is_match(&clean), "teléfono sin anonimizar: {:?}", clean);
        prop_assert!(clean.contains("<<"), "no se generaron tokens: {:?}", clean);
    }

    /// Volver a anonimizar una salida ya anonimizada no introduce cambios.
    #[test]
    fn anonymizer_is_idempotent(text in arb_sensitive_text()) {
        let mut a = Anonymizer::new();
        let once = a.anonymize(&text);
        let twice = a.anonymize(&once);
        prop_assert_eq!(&twice, &once);
        prop_assert_eq!(a.restore(&once), text);
    }

    /// Nunca hace panic con texto arbitrario (incluidos tokens falsos).
    #[test]
    fn anonymizer_never_panics(input in arb_any_text()) {
        let mut a = Anonymizer::new();
        let clean = a.anonymize(&input);
        let _ = a.restore(&clean);
    }
}

// ---------------------------------------------------------------------------
// LlmProvider
// ---------------------------------------------------------------------------

proptest! {
    /// Cada proveedor se serializa/parsea de forma estable y tiene modelo por
    /// defecto no vacío.
    #[test]
    fn provider_roundtrips(idx in 0usize..3) {
        let providers = [
            LlmProvider::Anthropic,
            LlmProvider::OpenRouter,
            LlmProvider::FreeLLMApi,
        ];
        let p = providers[idx];
        prop_assert_eq!(LlmProvider::from_config(p.as_str()), p);
        prop_assert!(!p.default_model().is_empty());
        prop_assert!(!p.label().is_empty());
    }
}
