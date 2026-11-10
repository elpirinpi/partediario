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

/// Paleta del tema (claro u oscuro). Los colores se leen de la paleta
/// activa guardada en el `Context` de egui (`set_palette` / `pal`), no de
/// constantes: así el modo oscuro puede cambiarlos en caliente sin
/// reescribir cada uso. La variante clara es el diseño institucional
/// aprobado (Concepto A); la oscura mantiene el azul institucional
/// (aclarado a #2E6CB5 para que el texto blanco siga con contraste ≥4.5:1)
/// y el dorado solo para Generar.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    // Superficies.
    pub bg_panel: egui::Color32,
    pub bg_card: egui::Color32,
    pub bg_overlay: egui::Color32,
    pub bg_hover: egui::Color32,
    pub bg_active: egui::Color32,
    pub bg_inset: egui::Color32,
    pub border_subtle: egui::Color32,
    // Azul profundo (tarjeta oscura "Turno de hoy").
    pub deep_bg: egui::Color32,
    pub deep_text: egui::Color32,
    pub deep_sub: egui::Color32,
    // Texto (jerarquía por énfasis).
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub text_muted: egui::Color32,
    // Acento: azul institucional.
    pub accent: egui::Color32,
    pub accent_hover: egui::Color32,
    pub on_accent: egui::Color32,
    // Dorado institucional (SOLO la acción final: Generar Parte Diario).
    pub gold: egui::Color32,
    pub on_gold: egui::Color32,
    // Semánticos (tonos oscuros en claro / claros en oscuro, para buen
    // contraste en ambos temas).
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,
    // Tintes suaves (color + alfa) para fondos de avisos y selección.
    pub accent_tint: egui::Color32,
    pub accent_line: egui::Color32,
    pub warn_tint: egui::Color32,
    pub warn_line: egui::Color32,
    pub info_tint: egui::Color32,
    pub success_tint: egui::Color32,
    pub success_line: egui::Color32,
}

impl Palette {
    /// Tema claro — Concepto A "Esencial" (aprobado).
    pub(crate) fn light() -> Self {
        Self {
            bg_panel: egui::Color32::from_rgb(0xF4, 0xF6, 0xFA),
            bg_card: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            bg_overlay: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            bg_hover: egui::Color32::from_rgb(0xE8, 0xED, 0xF5),
            bg_active: egui::Color32::from_rgb(0xD5, 0xDE, 0xEC),
            bg_inset: egui::Color32::from_rgb(0xE9, 0xED, 0xF3),
            border_subtle: egui::Color32::from_rgb(0xD5, 0xDC, 0xE8),
            deep_bg: egui::Color32::from_rgb(0x1B, 0x3A, 0x6B),
            deep_text: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            deep_sub: egui::Color32::from_rgb(0xC7, 0xD3, 0xE8),
            // Texto tenue #5A6577: 5.89:1 sobre blanco, 5.45:1 sobre el
            // panel (mínimo WCAG AA 4.5:1).
            text_primary: egui::Color32::from_rgb(0x1A, 0x24, 0x33),
            text_secondary: egui::Color32::from_rgb(0x4A, 0x55, 0x68),
            text_muted: egui::Color32::from_rgb(0x5A, 0x65, 0x77),
            accent: egui::Color32::from_rgb(0x0F, 0x4C, 0x9C),
            accent_hover: egui::Color32::from_rgb(0x0C, 0x3F, 0x82),
            on_accent: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            gold: egui::Color32::from_rgb(0xC9, 0xA8, 0x6A),
            on_gold: egui::Color32::from_rgb(0x1A, 0x24, 0x33),
            success: egui::Color32::from_rgb(0x1E, 0x7A, 0x4C),
            warning: egui::Color32::from_rgb(0x9A, 0x6B, 0x0F),
            error: egui::Color32::from_rgb(0xB3, 0x26, 0x1E),
            accent_tint: egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 30),
            accent_line: egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 140),
            warn_tint: egui::Color32::from_rgba_unmultiplied(0xB7, 0x79, 0x1F, 24),
            warn_line: egui::Color32::from_rgba_unmultiplied(0xB7, 0x79, 0x1F, 110),
            info_tint: egui::Color32::from_rgba_unmultiplied(0x0F, 0x4C, 0x9C, 24),
            success_tint: egui::Color32::from_rgba_unmultiplied(0x1E, 0x7A, 0x4C, 28),
            success_line: egui::Color32::from_rgba_unmultiplied(0x1E, 0x7A, 0x4C, 120),
        }
    }

    /// Tema oscuro. Contrastes verificados (texto sobre tarjeta #1D232E):
    /// primario 13.29:1, secundario 7.86:1, tenue 5.26:1; blanco sobre
    /// acento #2E6CB5 5.35:1; éxito 5.81:1, aviso 7.38:1, error 5.28:1.
    pub(crate) fn dark() -> Self {
        Self {
            bg_panel: egui::Color32::from_rgb(0x14, 0x18, 0x1F),
            bg_card: egui::Color32::from_rgb(0x1D, 0x23, 0x2E),
            bg_overlay: egui::Color32::from_rgb(0x1D, 0x23, 0x2E),
            bg_hover: egui::Color32::from_rgb(0x2A, 0x32, 0x42),
            bg_active: egui::Color32::from_rgb(0x36, 0x41, 0x5A),
            bg_inset: egui::Color32::from_rgb(0x10, 0x14, 0x1A),
            border_subtle: egui::Color32::from_rgb(0x32, 0x3E, 0x52),
            deep_bg: egui::Color32::from_rgb(0x1B, 0x3A, 0x6B),
            deep_text: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            deep_sub: egui::Color32::from_rgb(0xC7, 0xD3, 0xE8),
            text_primary: egui::Color32::from_rgb(0xE8, 0xEC, 0xF1),
            text_secondary: egui::Color32::from_rgb(0xAE, 0xB8, 0xC6),
            text_muted: egui::Color32::from_rgb(0x8B, 0x96, 0xA5),
            accent: egui::Color32::from_rgb(0x2E, 0x6C, 0xB5),
            accent_hover: egui::Color32::from_rgb(0x26, 0x59, 0x8F),
            on_accent: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            gold: egui::Color32::from_rgb(0xC9, 0xA8, 0x6A),
            on_gold: egui::Color32::from_rgb(0x1A, 0x24, 0x33),
            success: egui::Color32::from_rgb(0x4C, 0xAF, 0x7D),
            warning: egui::Color32::from_rgb(0xE0, 0xA8, 0x3C),
            error: egui::Color32::from_rgb(0xE5, 0x73, 0x73),
            accent_tint: egui::Color32::from_rgba_unmultiplied(0x2E, 0x6C, 0xB5, 48),
            accent_line: egui::Color32::from_rgba_unmultiplied(0x2E, 0x6C, 0xB5, 160),
            warn_tint: egui::Color32::from_rgba_unmultiplied(0xE0, 0xA8, 0x3C, 36),
            warn_line: egui::Color32::from_rgba_unmultiplied(0xE0, 0xA8, 0x3C, 130),
            info_tint: egui::Color32::from_rgba_unmultiplied(0x2E, 0x6C, 0xB5, 40),
            success_tint: egui::Color32::from_rgba_unmultiplied(0x4C, 0xAF, 0x7D, 40),
            success_line: egui::Color32::from_rgba_unmultiplied(0x4C, 0xAF, 0x7D, 140),
        }
    }
}

/// Guarda la paleta activa en el `Context` (se repone en cada `update`,
/// por si el dato temporal se limpia entre fotogramas).
pub(crate) fn set_palette(ctx: &egui::Context, dark: bool) {
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("parte_diario_palette"),
            if dark {
                Palette::dark()
            } else {
                Palette::light()
            },
        );
    });
}

/// Paleta activa para este `Context` (clara si aún no se fijó ninguna).
pub(crate) fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|d| {
        d.get_temp::<Palette>(egui::Id::new("parte_diario_palette"))
            .unwrap_or_else(Palette::light)
    })
}

/// Paleta activa para el `Ui` actual.
pub(crate) fn pal(ui: &egui::Ui) -> Palette {
    palette(ui.ctx())
}

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
/// símbolos que Inter (subconjunto latino) no trae (✓, →, ⚠, ℹ, ◀, ▶, ⬆).
/// La interfaz NO usa emoji como iconos (UI/UX Pro Max: `no-emoji-icons`):
/// botones y navegación llevan solo texto, y los avisos usan ⚠ / ℹ.
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

/// Visuales de egui para la paleta dada (clara u oscura). Se aplican a
/// AMBOS temas del sistema para que el tema del OS no pueda romper la
/// interfaz (egui 0.29 sigue el tema del sistema por defecto).
pub(crate) fn app_visuals(dark: bool) -> egui::Visuals {
    let p = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.bg_panel;
    v.extreme_bg_color = p.bg_inset;
    v.window_fill = p.bg_overlay;
    v.faint_bg_color = p.bg_hover;
    v.code_bg_color = p.bg_inset;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.error;
    v.hyperlink_color = p.accent_hover;

    let r = egui::Rounding::same(10.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, p.text_primary);
    v.widgets.inactive.rounding = r;
    v.widgets.inactive.bg_fill = p.bg_hover;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, p.text_primary);
    v.widgets.hovered.rounding = r;
    v.widgets.hovered.bg_fill = p.bg_active;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, p.text_primary);
    v.widgets.active.rounding = r;
    v.widgets.active.bg_fill = p.accent_tint;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, p.accent);
    v.widgets.open.rounding = r;
    v.selection.bg_fill = p.accent_tint;
    v.selection.stroke = egui::Stroke::new(1.0_f32, p.accent_line);
    v.window_rounding = egui::Rounding::same(14.0);
    v.menu_rounding = egui::Rounding::same(10.0);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.border_subtle);
    v
}

/// Aplica el tema completo a un `Context`: paleta + visuales + estilo.
/// Llamar al arrancar y cada vez que cambie el tema o la densidad.
pub(crate) fn apply_theme(ctx: &egui::Context, dark: bool, compact: bool) {
    set_palette(ctx, dark);
    let visuals = app_visuals(dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);
    ctx.all_styles_mut(|style| apply_style(style, compact));
}

/// Espaciado y tipografía globales: espaciado en múltiplos de 8 (4 en
/// vista compacta); títulos en Inter SemiBold, cuerpo en Inter Regular.
pub(crate) fn apply_style(style: &mut egui::Style, compact: bool) {
    let s = if compact { 4.0 } else { 8.0 };
    style.spacing.item_spacing = egui::vec2(s, s);
    style.spacing.button_padding = if compact {
        egui::vec2(12.0, 6.0)
    } else {
        egui::vec2(16.0, 8.0)
    };
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
    btn_primary_enabled(ui, text, true)
}

/// Botón primario con estado habilitado/deshabilitado. Mientras una
/// operación asíncrona corre (p. ej. formalización con IA), el botón que
/// la dispara se muestra deshabilitado en vez de parecer activo sin hacer
/// nada al pulsarlo.
pub(crate) fn btn_primary_enabled(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).strong().color(pal(ui).on_accent))
            .fill(pal(ui).accent)
            .rounding(10.0),
    )
}

/// Botón de acción callado: relleno claro, borde azul y texto azul. Para
/// la acción principal del estado vacío ("Cargar .zip de WhatsApp"); el
/// único botón con color fuerte de la app es el dorado de Generar.
pub(crate) fn btn_action(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .strong()
                .color(pal(ui).accent_hover),
        )
        .fill(pal(ui).bg_hover)
        .stroke(egui::Stroke::new(1.0_f32, pal(ui).accent_line))
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
        egui::Button::new(egui::RichText::new(text).strong().color(pal(ui).on_gold))
            .fill(pal(ui).gold)
            .rounding(10.0),
    )
}

/// Botón secundario: superficie sutil + borde fino, para acciones de apoyo.
pub(crate) fn btn_secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    btn_secondary_enabled(ui, text, true)
}

/// Botón secundario con estado habilitado/deshabilitado (ver
/// `btn_primary_enabled`).
pub(crate) fn btn_secondary_enabled(
    ui: &mut egui::Ui,
    text: &str,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).color(pal(ui).text_primary))
            .fill(pal(ui).bg_hover)
            .stroke(egui::Stroke::new(1.0_f32, pal(ui).border_subtle))
            .rounding(10.0),
    )
}

/// Botón terciario: solo texto, para acciones de baja prioridad.
pub(crate) fn btn_tertiary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(pal(ui).text_secondary))
            .fill(egui::Color32::TRANSPARENT)
            .rounding(10.0),
    )
}

/// Insignia neutra pequeña (texto secundario, nada de blanco sobre neón).
pub(crate) fn pill(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(pal(ui).bg_hover)
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(11.5)
                    .color(pal(ui).text_secondary),
            );
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

/// Insignia de verificación dibujada (círculo + ✓ con líneas): ninguna
/// fuente del stack trae el glifo "✓", por eso se dibuja a mano, igual que
/// en el stepper. Para confirmar acciones completadas (éxito al generar).
pub(crate) fn check_badge(ui: &mut egui::Ui, color: egui::Color32) {
    let size = 20.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let c = rect.center();
    let painter = ui.painter_at(rect);
    painter.circle_filled(c, size / 2.0, color);
    let s = 4.5;
    painter.line_segment(
        [c + egui::vec2(-s, 0.0), c + egui::vec2(-s + 3.0, s - 1.0)],
        egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
    );
    painter.line_segment(
        [
            c + egui::vec2(-s + 3.0, s - 1.0),
            c + egui::vec2(s, -s + 1.0),
        ],
        egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
    );
}

/// Banda de aviso: tinte suave + borde tenue del color semántico.
pub(crate) fn banner(ui: &mut egui::Ui, text: &str, tint: egui::Color32, line: egui::Color32) {
    egui::Frame::none()
        .fill(tint)
        .stroke(egui::Stroke::new(1.0_f32, line))
        .rounding(10.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(13.0)
                    .color(pal(ui).text_primary),
            );
        });
    ui.add_space(8.0);
}

/// Tarjeta de nivel 1: superficie elevada + borde sutil.
pub(crate) fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(pal(ui).bg_card)
        .stroke(egui::Stroke::new(1.0_f32, pal(ui).border_subtle))
        .rounding(14.0)
        .inner_margin(16.0)
        .show(ui, |ui| add(ui))
        .inner
}

/// Etiqueta superior de sección (overline): 11px, secundaria, en formato
/// oración (nunca en mayúsculas: el rótulo en mayúsculas es cromo de
/// plantilla).
pub(crate) fn overline(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(pal(ui).text_muted),
    );
    ui.add_space(2.0);
}

/// Título de sección dentro de una tarjeta, con regla fina debajo.
pub(crate) fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .strong()
            .size(14.5)
            .color(pal(ui).text_primary),
    );
    ui.add_space(2.0);
    // Regla fina azul-gris bajo el título.
    let rect = ui.available_rect_before_wrap();
    let y = rect.min.y + 1.0;
    ui.painter().hline(
        rect.x_range(),
        y,
        egui::Stroke::new(1.0_f32, pal(ui).border_subtle),
    );
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
///
/// Los pasos HECHOS y el ACTUAL son clicables (navegación atrás predecible,
/// UI/UX Pro Max: `back-behavior`): devuelven su índice al pulsarlos; los
/// PENDIENTES no se pueden saltar. El cursor cambia a mano sobre un paso
/// clicable y se dibuja un halo suave.
pub(crate) fn stepper(ui: &mut egui::Ui, steps: &[(String, StepState)]) -> Option<usize> {
    let width = ui.available_width();
    let height = 54.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    if steps.is_empty() {
        return None;
    }
    let painter = ui.painter_at(rect);
    let n = steps.len() as f32;
    let y_circle = rect.top() + 15.0;
    let r = 14.0;
    let xs: Vec<f32> = (0..steps.len())
        .map(|i| rect.left() + (i as f32 + 0.5) * width / n)
        .collect();
    // Paso bajo el cursor (solo hechos y actual son clicables).
    let hovered = response.hover_pos().and_then(|pos| {
        xs.iter()
            .position(|&x| {
                (pos.x - x).abs() <= 34.0 && pos.y >= rect.top() && pos.y <= rect.bottom()
            })
            .filter(|&i| !matches!(steps[i].1, StepState::Pending))
    });
    if hovered.is_some() {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
    }
    // Línea que une los círculos (detrás de ellos).
    if steps.len() > 1 {
        painter.line_segment(
            [
                egui::pos2(xs[0], y_circle),
                egui::pos2(*xs.last().unwrap(), y_circle),
            ],
            egui::Stroke::new(1.5_f32, pal(ui).border_subtle),
        );
    }
    for (i, (label, state)) in steps.iter().enumerate() {
        let c = egui::pos2(xs[i], y_circle);
        // Halo suave al pasar el cursor sobre un paso clicable.
        if hovered == Some(i) {
            painter.circle_filled(c, r + 5.0, pal(ui).bg_hover);
        }
        match state {
            StepState::Done => {
                painter.circle_filled(c, r, pal(ui).accent);
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
                painter.circle_filled(c, r, pal(ui).accent);
                painter.circle_stroke(c, r + 3.0, egui::Stroke::new(1.5_f32, pal(ui).accent_line));
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    egui::Color32::WHITE,
                );
            }
            StepState::Pending => {
                painter.circle_filled(c, r, pal(ui).bg_hover);
                painter.circle_stroke(c, r, egui::Stroke::new(1.0_f32, pal(ui).border_subtle));
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    pal(ui).text_muted,
                );
            }
        }
        let (label_color, label_font) = match state {
            StepState::Current => (pal(ui).text_primary, font_semibold(11.5)),
            StepState::Done => (pal(ui).text_primary, egui::FontId::proportional(11.5)),
            StepState::Pending => (pal(ui).text_muted, egui::FontId::proportional(11.5)),
        };
        painter.text(
            egui::pos2(xs[i], rect.top() + 40.0),
            egui::Align2::CENTER_TOP,
            label,
            label_font,
            label_color,
        );
    }
    // Clic sobre un paso clicable (hecho o actual); los pendientes y los
    // huecos entre pasos no devuelven nada.
    if response.clicked() {
        hovered
    } else {
        None
    }
}

/// Botón de un control segmentado (p. ej. Editar | Vista previa). El activo
/// lleva tinte azul + texto azul; el inactivo, solo texto atenuado.
pub(crate) fn seg_button(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let (fill, fg) = if active {
        (pal(ui).accent_tint, pal(ui).accent_hover)
    } else {
        (egui::Color32::TRANSPARENT, pal(ui).text_muted)
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
    /// Los símbolos que la interfaz usa deben dar `true`. "→", "✓", "✕" y
    /// "✗" no los cubre ninguna fuente del stack, por eso la interfaz no
    /// los usa (se verían como □); los emoji (📂, ✨, ⚡, 🧹, 👁, ✏️, 🕘, ⚙,
    /// 📝) tampoco se usan: la interfaz lleva solo texto y los símbolos
    /// tipográficos ⚠ / ℹ / ◀ / ▶ / ⬆.
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
                '×' | '·' | '–' | '—' | '…' | '•' | '⚠' | 'ℹ' | '◀' | '▶' | '⬆'
            ) {
                assert!(has, "falta glifo para {c:?} (U+{:04X})", c as u32);
            }
        }
    }
}
