//! Sistema de diseño v3 — Concepto A "Esencial" (claro e institucional).
//!
//! Estructura de 3 columnas, SIN banda superior:
//! - Barra lateral (200 px): marca "PD", navegación y tarjeta "Turno de hoy".
//! - Centro: stepper de 6 pasos (Cargar → Analizar → Extraer → Generar →
//!   Editar → Exportar) y contenido del flujo.
//! - Inspector derecho "Resumen" (252 px): estado, fechas, vista previa
//!   y la acción final.
//!
//! Paleta clara: fondo #F4F6FA, tarjetas blancas, texto oscuro. Azul
//! institucional #0F4C9C (acento) / #1B3A6B (azul profundo, tarjeta oscura);
//! el dorado #C9A86A marca ÚNICAMENTE la acción final (botón "Generar Parte
//! Diario") y NUNCA un paso del stepper: el paso actual va en azul.
//! Tipografía Inter (embebida en `assets/fonts/`). Espaciado en múltiplos
//! de 8.

use eframe::egui;

// --- Superficies (nivel de elevación) ---
pub(crate) const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(0xF4, 0xF6, 0xFA);
pub(crate) const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub(crate) const BG_OVERLAY: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub(crate) const BG_HOVER: egui::Color32 = egui::Color32::from_rgb(0xE8, 0xED, 0xF5);
pub(crate) const BG_ACTIVE: egui::Color32 = egui::Color32::from_rgb(0xD5, 0xDE, 0xEC);
pub(crate) const BG_INSET: egui::Color32 = egui::Color32::from_rgb(0xE9, 0xED, 0xF3);
pub(crate) const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(0xD5, 0xDC, 0xE8);

// --- Azul profundo (tarjeta oscura "Turno de hoy") ---
pub(crate) const DEEP_BG: egui::Color32 = egui::Color32::from_rgb(0x1B, 0x3A, 0x6B);
pub(crate) const DEEP_TEXT: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub(crate) const DEEP_SUB: egui::Color32 = egui::Color32::from_rgb(0xC7, 0xD3, 0xE8);

// --- Texto (jerarquía por énfasis) ---
pub(crate) const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0x1A, 0x24, 0x33);
pub(crate) const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x4A, 0x55, 0x68);
pub(crate) const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(0x7A, 0x86, 0x99);

// --- Acento: azul institucional #0F4C9C ---
pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x0F, 0x4C, 0x9C);
pub(crate) const ACCENT_HOVER: egui::Color32 = egui::Color32::from_rgb(0x0C, 0x3F, 0x82);
pub(crate) const ON_ACCENT: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);

// --- Dorado institucional (SOLO la acción final: Generar Parte Diario) ---
pub(crate) const GOLD: egui::Color32 = egui::Color32::from_rgb(0xC9, 0xA8, 0x6A);
pub(crate) const ON_GOLD: egui::Color32 = egui::Color32::from_rgb(0x1A, 0x24, 0x33);

// --- Semánticos para tema claro (tonos oscuros para buen contraste) ---
pub(crate) const SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x7A, 0x4C);
pub(crate) const WARNING: egui::Color32 = egui::Color32::from_rgb(0x9A, 0x6B, 0x0F);
pub(crate) const ERROR: egui::Color32 = egui::Color32::from_rgb(0xB3, 0x26, 0x1E);

pub(crate) static ACCENT_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 30));
pub(crate) static ACCENT_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 140));
pub(crate) static WARN_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0xB7, 0x79, 0x1F, 24));
pub(crate) static WARN_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0xB7, 0x79, 0x1F, 110));
pub(crate) static INFO_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 24));
pub(crate) static SUCCESS_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x1E, 0x7A, 0x4C, 28));
pub(crate) static SUCCESS_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x1E, 0x7A, 0x4C, 120));

/// Familia Inter SemiBold (títulos) como `FontFamily` nombrada.
pub(crate) fn font_semibold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("Inter-SemiBold".into()))
}

/// Familia Inter Bold (marca "PD", énfasis fuerte).
pub(crate) fn font_bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("Inter-Bold".into()))
}

/// Instala Inter (Regular / SemiBold / Bold) desde `assets/fonts/`.
///
/// Las fuentes se guardan como base64 (texto) porque el flujo de subida a
/// GitHub solo admite archivos UTF-8 y un .ttf es binario; los bytes
/// decodificados son idénticos a los del .ttf original.
///
/// Inter queda primera en la familia proporcional; las fuentes por defecto
/// de egui (Ubuntu-Light y NotoEmoji) se conservan como respaldo para los
/// símbolos y emojis que Inter (subconjunto latino) no trae (✓, →, ⚠, 📂…).
fn font_bytes(b64: &'static str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("fuente Inter en base64 corrupta")
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".to_owned(),
        egui::FontData::from_owned(font_bytes(include_str!(
            "../../../../assets/fonts/Inter-Regular.ttf.b64"
        ))),
    );
    fonts.font_data.insert(
        "Inter-SemiBold".to_owned(),
        egui::FontData::from_owned(font_bytes(include_str!(
            "../../../../assets/fonts/Inter-SemiBold.ttf.b64"
        ))),
    );
    fonts.font_data.insert(
        "Inter-Bold".to_owned(),
        egui::FontData::from_owned(font_bytes(include_str!(
            "../../../../assets/fonts/Inter-Bold.ttf.b64"
        ))),
    );
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        fam.insert(0, "Inter".to_owned());
    }
    fonts.families.insert(
        egui::FontFamily::Name("Inter-SemiBold".into()),
        vec![
            "Inter-SemiBold".to_owned(),
            "Ubuntu-Light".to_owned(),
            "NotoEmoji-Regular".to_owned(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Name("Inter-Bold".into()),
        vec![
            "Inter-Bold".to_owned(),
            "Ubuntu-Light".to_owned(),
            "NotoEmoji-Regular".to_owned(),
        ],
    );
    ctx.set_fonts(fonts);
}

pub(crate) fn app_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = BG_PANEL;
    v.extreme_bg_color = BG_INSET;
    v.window_fill = BG_OVERLAY;
    v.faint_bg_color = BG_HOVER;
    v.code_bg_color = BG_INSET;
    v.warn_fg_color = WARNING;
    v.error_fg_color = ERROR;
    v.hyperlink_color = ACCENT_HOVER;

    let r = egui::Rounding::same(10.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.inactive.rounding = r;
    v.widgets.inactive.bg_fill = BG_HOVER;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.hovered.rounding = r;
    v.widgets.hovered.bg_fill = BG_ACTIVE;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.active.rounding = r;
    v.widgets.active.bg_fill = *ACCENT_TINT;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    v.widgets.open.rounding = r;
    v.selection.bg_fill = *ACCENT_TINT;
    v.selection.stroke = egui::Stroke::new(1.0_f32, *ACCENT_LINE);
    v.window_rounding = egui::Rounding::same(14.0);
    v.menu_rounding = egui::Rounding::same(10.0);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_SUBTLE);
    v
}

/// Espaciado y tipografía globales: espaciado en múltiplos de 8; títulos en
/// Inter SemiBold, cuerpo en Inter Regular.
pub(crate) fn apply_style(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    style.spacing.indent = 16.0;
    style.spacing.interact_size = egui::vec2(32.0, 32.0);
    style.spacing.window_margin = egui::Margin::same(16.0);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, font_semibold(20.0));
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(13.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(11.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
}

/// Botón primario: UNA sola acción principal por vista. Azul institucional
/// sólido con texto blanco.
pub(crate) fn btn_primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).strong().color(ON_ACCENT))
            .fill(ACCENT)
            .rounding(10.0),
    )
}

/// Botón de acción callado: relleno claro, borde azul y texto azul. Para
/// la acción principal del estado vacío ("Cargar .zip de WhatsApp"); el
/// único botón con color fuerte de la app es el dorado de Generar.
pub(crate) fn btn_action(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).strong().color(ACCENT_HOVER))
            .fill(BG_HOVER)
            .stroke(egui::Stroke::new(1.0_f32, *ACCENT_LINE))
            .rounding(10.0),
    )
}

/// Botón de la acción FINAL (Generar Parte Diario): dorado sólido con
/// texto oscuro, a ancho completo y 40 px de alto. Es el único botón
/// dorado de la app.
pub(crate) fn btn_gold(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let w = ui.available_width();
    ui.add_sized(
        [w, 40.0],
        egui::Button::new(egui::RichText::new(text).strong().color(ON_GOLD))
            .fill(GOLD)
            .rounding(10.0),
    )
}

/// Botón secundario: superficie sutil + borde fino, para acciones de apoyo.
pub(crate) fn btn_secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(TEXT_PRIMARY))
            .fill(BG_HOVER)
            .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
            .rounding(10.0),
    )
}

/// Botón terciario: solo texto, para acciones de baja prioridad.
pub(crate) fn btn_tertiary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(TEXT_SECONDARY))
            .fill(egui::Color32::TRANSPARENT)
            .rounding(10.0),
    )
}

/// Insignia neutra pequeña (texto secundario, nada de blanco sobre neón).
pub(crate) fn pill(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(BG_HOVER)
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.5).color(TEXT_SECONDARY));
        });
}

/// Insignia semántica con tinte suave (ej. "huérfana", "formalizado").
pub(crate) fn pill_tint(ui: &mut egui::Ui, text: &str, tint: egui::Color32, fg: egui::Color32) {
    egui::Frame::none()
        .fill(tint)
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.5).color(fg));
        });
}

/// Banda de aviso: tinte suave + borde tenue del color semántico.
pub(crate) fn banner(ui: &mut egui::Ui, text: &str, tint: egui::Color32, line: egui::Color32) {
    egui::Frame::none()
        .fill(tint)
        .stroke(egui::Stroke::new(1.0_f32, line))
        .rounding(10.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(13.0).color(TEXT_PRIMARY));
        });
    ui.add_space(8.0);
}

/// Tarjeta de nivel 1: superficie elevada + borde sutil.
pub(crate) fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(BG_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(14.0)
        .inner_margin(16.0)
        .show(ui, |ui| add(ui))
        .inner
}

/// Etiqueta superior de sección (overline): 11px, secundaria, en formato
/// oración (nunca en mayúsculas: el rótulo en mayúsculas es cromo de
/// plantilla).
pub(crate) fn overline(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.0).color(TEXT_MUTED));
    ui.add_space(2.0);
}

/// Título de sección dentro de una tarjeta, con regla fina debajo.
pub(crate) fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .strong()
            .size(14.5)
            .color(TEXT_PRIMARY),
    );
    ui.add_space(2.0);
    // Regla fina azul-gris bajo el título.
    let rect = ui.available_rect_before_wrap();
    let y = rect.min.y + 1.0;
    ui.painter()
        .hline(rect.x_range(), y, egui::Stroke::new(1.0_f32, BORDER_SUBTLE));
    ui.add_space(8.0);
}

/// Estado de un paso del flujo guiado.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepState {
    /// Ya completado.
    Done,
    /// Paso en el que está el usuario ahora.
    Current,
    /// Aún no llega.
    Pending,
}

/// Indicador de pasos del flujo (Cargar → Analizar → Extraer → Generar →
/// Editar → Exportar). Cada paso es un círculo con su número (o ✓ si está
/// hecho) y su etiqueta debajo, unidos por una línea. El paso actual y los
/// hechos van en AZUL institucional con número/blanco; los pendientes en
/// gris. El dorado NO se usa aquí: es solo para el botón Generar.
pub(crate) fn stepper(ui: &mut egui::Ui, steps: &[(String, StepState)]) {
    let width = ui.available_width();
    let height = 54.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if steps.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let n = steps.len() as f32;
    let y_circle = rect.top() + 15.0;
    let r = 14.0;
    let xs: Vec<f32> = (0..steps.len())
        .map(|i| rect.left() + (i as f32 + 0.5) * width / n)
        .collect();
    // Línea que une los círculos (detrás de ellos).
    if steps.len() > 1 {
        painter.line_segment(
            [
                egui::pos2(xs[0], y_circle),
                egui::pos2(*xs.last().unwrap(), y_circle),
            ],
            egui::Stroke::new(1.5_f32, BORDER_SUBTLE),
        );
    }
    for (i, (label, state)) in steps.iter().enumerate() {
        let c = egui::pos2(xs[i], y_circle);
        match state {
            StepState::Done => {
                painter.circle_filled(c, r, ACCENT);
                // Marca de verificación dibujada con líneas: ninguna fuente
                // del stack trae el glifo "✓" (se vería como □).
                let s = 5.0;
                painter.line_segment(
                    [c + egui::vec2(-s, 0.0), c + egui::vec2(-s + 3.5, s - 1.0)],
                    egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
                );
                painter.line_segment(
                    [
                        c + egui::vec2(-s + 3.5, s - 1.0),
                        c + egui::vec2(s, -s + 1.0),
                    ],
                    egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
                );
            }
            StepState::Current => {
                painter.circle_filled(c, r, ACCENT);
                painter.circle_stroke(c, r + 3.0, egui::Stroke::new(1.5_f32, *ACCENT_LINE));
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    egui::Color32::WHITE,
                );
            }
            StepState::Pending => {
                painter.circle_filled(c, r, BG_HOVER);
                painter.circle_stroke(c, r, egui::Stroke::new(1.0_f32, BORDER_SUBTLE));
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    TEXT_MUTED,
                );
            }
        }
        let (label_color, label_font) = match state {
            StepState::Current => (TEXT_PRIMARY, font_semibold(11.5)),
            StepState::Done => (TEXT_PRIMARY, egui::FontId::proportional(11.5)),
            StepState::Pending => (TEXT_MUTED, egui::FontId::proportional(11.5)),
        };
        painter.text(
            egui::pos2(xs[i], rect.top() + 40.0),
            egui::Align2::CENTER_TOP,
            label,
            label_font,
            label_color,
        );
    }
}

/// Botón de un control segmentado (p. ej. Editar | Vista previa). El activo
/// lleva tinte azul + texto azul; el inactivo, solo texto atenuado.
pub(crate) fn seg_button(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let (fill, fg) = if active {
        (*ACCENT_TINT, ACCENT_HOVER)
    } else {
        (egui::Color32::TRANSPARENT, TEXT_MUTED)
    };
    ui.add(
        egui::Button::new(egui::RichText::new(text).strong().color(fg))
            .fill(fill)
            .rounding(8.0),
    )
}

/// Dibuja un rectángulo de borde punteado (zona de arrastre de archivos).
pub(crate) fn dashed_rect(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let dash = 8.0;
    let gap = 6.0;
    let stroke = egui::Stroke::new(1.5_f32, color);
    let mut x = rect.left();
    while x < rect.right() {
        let x2 = (x + dash).min(rect.right());
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x2, rect.top())],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(x, rect.bottom()), egui::pos2(x2, rect.bottom())],
            stroke,
        );
        x += dash + gap;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        let y2 = (y + dash).min(rect.bottom());
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.left(), y2)],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(rect.right(), y), egui::pos2(rect.right(), y2)],
            stroke,
        );
        y += dash + gap;
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    /// Diagnóstico de cobertura de glifos (no es una prueba de
    /// funcionalidad): imprime qué símbolos cubre la familia proporcional.
    /// Los símbolos que la interfaz usa deben dar `true`. "→", "✓", "✕",
    /// "✗" y "🧹" no los cubre ninguna fuente del stack, por eso la
    /// interfaz no los usa (se veían como □).
    #[test]
    fn diagnostico_cobertura_glifos() {
        let ctx = egui::Context::default();
        super::install_fonts(&ctx);
        let fid = egui::FontId::proportional(13.0);
        let mut results = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                for c in [
                    '→', '✓', '✕', '✗', '🧹', '×', '›', '·', '–', '—', '…', '•', '⚠', 'ℹ', '◀',
                    '▶', '📂', '✨', '⚡', '⬆', '⇒',
                ] {
                    results.push((c, fonts.has_glyph(&fid, c)));
                }
            });
        });
        for (c, has) in &results {
            eprintln!("U+{:04X} {:?} -> {}", *c as u32, c, has);
        }
        // Símbolos que la interfaz sí usa y deben estar cubiertos.
        for (c, has) in results {
            if matches!(
                c,
                '×' | '·'
                    | '–'
                    | '—'
                    | '…'
                    | '•'
                    | '⚠'
                    | 'ℹ'
                    | '◀'
                    | '▶'
                    | '📂'
                    | '✨'
                    | '⚡'
                    | '⬆'
            ) {
                assert!(has, "falta glifo para {c:?} (U+{:04X})", c as u32);
            }
        }
    }
}
