//! Anonimización opcional antes de enviar texto a la IA.
//!
//! Sustituye cédulas, teléfonos, correos y nombres introducidos por fórmulas
//! como "nombrado/a" o "señor/a". Los valores originales se guardan solo en
//! memoria y se restauran en la respuesta del modelo.

use std::sync::OnceLock;

use regex::Regex;

#[derive(Debug, Clone, Default)]
pub struct Anonymizer {
    replacements: Vec<(String, String)>,
}

impl Anonymizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devuelve el texto anonimizado y registra la correspondencia inversa.
    pub fn anonymize(&mut self, text: &str) -> String {
        let mut out = text.to_string();
        out = self.replace_named(&out);
        out = self.replace_regex(&out, re_cedula(), "CEDULA");
        out = self.replace_regex(&out, re_telefono(), "TELEFONO");
        out = self.replace_regex(&out, re_email(), "CORREO");
        out
    }

    /// Restaura los valores originales en el texto devuelto por la IA.
    pub fn restore(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (token, original) in &self.replacements {
            out = out.replace(token, original);
        }
        out
    }

    fn token(&mut self, kind: &str, original: String) -> String {
        if let Some((token, _)) = self
            .replacements
            .iter()
            .find(|(_, value)| value == &original)
        {
            return token.clone();
        }
        let token = format!("<<{}_{}>>", kind, self.replacements.len() + 1);
        self.replacements.push((token.clone(), original));
        token
    }

    fn replace_regex(&mut self, text: &str, re: &Regex, kind: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut last = 0;
        for caps in re.captures_iter(text) {
            let Some(m) = caps.get(0) else { continue };
            result.push_str(&text[last..m.start()]);
            let token = self.token(kind, m.as_str().to_string());
            result.push_str(&token);
            last = m.end();
        }
        result.push_str(&text[last..]);
        result
    }

    fn replace_named(&mut self, text: &str) -> String {
        let re = re_nombre();
        let mut result = String::with_capacity(text.len());
        let mut last = 0;
        for caps in re.captures_iter(text) {
            let Some(full) = caps.get(0) else { continue };
            let Some(name) = caps.get(2) else { continue };
            result.push_str(&text[last..full.start()]);
            result.push_str(&caps[1]);
            result.push(' ');
            let token = self.token("NOMBRE", name.as_str().to_string());
            result.push_str(&token);
            last = full.end();
        }
        result.push_str(&text[last..]);
        result
    }
}

fn re_cedula() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b\d{3}[- ]?\d{7}[- ]?\d\b").expect("regex cédula"))
}

fn re_telefono() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\b(?:\+?1[\s.-]?)?(?:809|829|849)[\s.-]?\d{3}[\s.-]?\d{4}\b")
            .expect("regex teléfono")
    })
}

fn re_email() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").expect("regex correo")
    })
}

fn re_nombre() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)\b(nombrado|nombrada|señor|señora|ciudadano|ciudadana|propietario|propietaria)\s+([A-ZÁÉÍÓÚÑ][a-záéíóúñ]+(?:\s+(?:de\s+|del\s+)?[A-ZÁÉÍÓÚÑ][a-záéíóúñ]+){0,3})",
        )
        .expect("regex nombre")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonimiza_y_restaura_datos() {
        let original = "El nombrado Rudy de los Santos, cédula 001-0313301-5, teléfono 809-555-1234, correo rudy@example.com.";
        let mut a = Anonymizer::new();
        let clean = a.anonymize(original);
        assert!(!clean.contains("Rudy"));
        assert!(!clean.contains("001-0313301-5"));
        assert!(!clean.contains("809-555-1234"));
        assert!(!clean.contains("rudy@example.com"));
        assert_eq!(a.restore(&clean), original);
    }

    #[test]
    fn conserva_datos_no_sensibles() {
        let mut a = Anonymizer::new();
        let clean = a.anonymize("Siendo las 11:59 p.m. del día 30/10/2026 en Sabana Perdida.");
        assert!(clean.contains("11:59"));
        assert!(clean.contains("30/10/2026"));
        assert!(clean.contains("Sabana Perdida"));
    }
}
