//! Vista previa del documento (WYSIWYG aproximado del .docx generado).
//!
//! Muestra el Parte Diario tal como quedará: fecha del documento, rango del
//! asunto y una tabla por novedad incluida (encabezados + fotos a la
//! izquierda, cuerpo formal —o crudo si aún no se formalizó— a la derecha),
//! en el mismo orden y con el mismo filtro que `build_report` en
//! `parte-diario-docx`. Se alimenta de los datos ya cargados (`novedades`,
//! `date_from`/`date_to`); nunca regenera el .docx. Al editar una novedad,
//! la vista previa se actualiza sola en el siguiente fotograma.

use chrono::{Datelike, NaiveDate};
use eframe::egui;

use super::theme;
use super::App;
use parte_diario_parser::grouping::Novedad;

/// Ancho de la columna izquierda (encabezados + fotos) en la vista previa.
const PREVIEW_LEFT_W: f32 = 180.0;
/// Ancho de la "hoja" del documento.
const PREVIEW_PAGE_W: f32 = 660.0;

// --- Colores "papel": el documento es papel blanco, como el .docx real ---
const PAPER: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
const PAPER_TEXT: egui::Color32 = egui::Color32::from_rgb(0x1C, 0x1C, 0x1E);
const PAPER_MUTED: egui::Color32 = egui::Color32::from_rgb(0x6B, 0x6B, 0x70);
const PAPER_NAVY: egui::Color32 = egui::Color32::from_rgb(0x1F, 0x3A, 0x5F);
const PAPER_RULE: egui::Color32 = egui::Color32::from_rgb(0xD5, 0xDC, 0xE8);
const PAPER_WARN: egui::Color32 = egui::Color32::from_rgb(0x8A, 0x5A, 0x00);

const MESES_ES: &[&str] = &[
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
];

fn fecha_larga(d: NaiveDate) -> String {
    format!(
        "{} de {}, {}.",
        d.day(),
        MESES_ES[d.month0() as usize],
        d.year()
    )
}

fn fecha_corta(d: NaiveDate) -> String {
    format!("{:02}-{:02}-{}", d.day(), d.month(), d.year())
}

/// Quita los `**` de negrita de WhatsApp (como hace el generador .docx).
fn strip_asterisks(s: &str) -> String {
    s.replace('*', "")
}

/// Mismo filtro que `build_report`: solo las incluidas, salvo huérfanas sin
/// texto ni encabezados.
fn incluidas(novedades: &[Novedad]) -> Vec<&Novedad> {
    novedades
        .iter()
        .filter(|n| {
            n.include
                && !(n.is_orphan
                    && n.body_raw.trim().is_empty()
                    && n.headers.is_empty())
        })
        .collect()
}

/// Datos ya resueltos de una novedad para la vista previa (todo propio, para
/// no pelear con el borrow checker al pedir miniaturas con `&mut self`).
struct NovedadPreview {
    id: usize,
    headers: Vec<String>,
    photos: Vec<String>,
    paras: Vec<String>,
    formalizado: bool,
    closing: Option<String>,
}

fn resolver(n: &Novedad) -> NovedadPreview {
    let formal = n.body_formal.trim();
    let raw = n.body_raw.trim();
    let (texto, formalizado) = if formal.is_empty() {
        (raw, false)
    } else {
        (formal, true)
    };
    let mut body = strip_asterisks(texto);
    // Evitar duplicar la oración de cierre (va como párrafo final, igual
    // que en el .docx).
    if let Some(cl) = &n.closing {
        let clt = strip_asterisks(cl.trim());
        if !clt.is_empty() {
            if let Some(pos) = body.find(&clt) {
                body.replace_range(pos..pos + clt.len(), "");
            }
        }
    }
    // Párrafos separados por líneas en blanco; colapsar espacios en blanco
    // dentro de cada párrafo (igual que el generador .docx).
    let mut paras: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !cur.trim().is_empty() {
                paras.push(cur.split_whitespace().collect::<Vec<_>>().join(" "));
                cur.clear();
            }
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(t);
        }
    }
    if !cur.trim().is_empty() {
        paras.push(cur.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    if paras.is_empty() {
        paras.push("(sin texto)".to_string());
    }
    NovedadPreview {
        id: n.id,
        headers: n.headers.clone(),
        photos: n.photos.clone(),
        paras,
        formalizado,
        closing: n.closing.as_ref().map(|c| strip_asterisks(c.trim())),
    }
}

/// Regla horizontal fina de 1px.
fn rule(ui: &mut egui::Ui, color: egui::Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
}

impl App {
    /// Muestra la vista previa del documento en el panel central.
    /// Sin novedades, muestra el esqueleto del documento (membrete, fecha
    /// y rango) con la nota de que aún no hay novedades cargadas: así el
    /// botón "Ver ejemplo de exportación" enseña cómo quedará el archivo.
    pub(crate) fn show_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let desde = super::parse_fecha(&self.date_from);
        let hasta = super::parse_fecha(&self.date_to);
        let fechas_validas =
            matches!((desde, hasta), (Some(d), Some(h)) if h >= d);
        let items: Vec<NovedadPreview> = incluidas(&self.novedades)
            .iter()
            .map(|n| resolver(n))
            .collect();
        let n_fotos: usize = items.iter().map(|n| n.photos.len()).sum();
        let excluidas = self.novedades.len() - items.len();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Vista previa del documento")
                    .strong()
                    .size(15.0)
                    .color(theme::TEXT_PRIMARY),
            );
            ui.label(
                egui::RichText::new(
                    "Se actualiza sola al editar las novedades. El documento final justifica el texto y usa el membrete de su plantilla .docx.",
                )
                .size(12.0)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(12.0);

            // ============ La hoja ============
            ui.vertical_centered(|ui| {
                egui::Frame::none()
                    .fill(PAPER)
                    .stroke(egui::Stroke::new(1.0_f32, theme::BORDER_SUBTLE))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::same(34.0))
                    .show(ui, |ui| {
                        ui.set_width(PREVIEW_PAGE_W);

                        // Encabezado del documento.
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("PARTE DIARIO")
                                    .strong()
                                    .size(20.0)
                                    .color(PAPER_NAVY),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Dirección de Anti-Ruidos y Contaminación Sónica, P.N.",
                                )
                                .size(12.5)
                                .color(PAPER_MUTED),
                            );
                            ui.add_space(6.0);
                            match desde {
                                Some(d) => ui.label(
                                    egui::RichText::new(format!(
                                        "Santo Domingo, D.N., {}",
                                        fecha_larga(d)
                                    ))
                                    .size(13.0)
                                    .color(PAPER_TEXT),
                                ),
                                None => ui.label(
                                    egui::RichText::new(
                                        "Fecha del documento: configure el rango de fechas.",
                                    )
                                    .size(13.0)
                                    .color(PAPER_WARN),
                                ),
                            }
                        });
                        ui.add_space(10.0);
                        rule(ui, PAPER_RULE);
                        ui.add_space(10.0);

                        // Rango del asunto (lo que la app escribe en la plantilla).
                        if fechas_validas {
                            let (d, h) = (desde.unwrap(), hasta.unwrap());
                            ui.label(
                                egui::RichText::new(format!(
                                    "Rango del reporte: desde las 08:00 horas del día {} hasta las 08:00 horas del día {}.",
                                    fecha_corta(d),
                                    fecha_corta(h)
                                ))
                                .size(13.0)
                                .color(PAPER_TEXT),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(
                                    "Rango del reporte: escriba fechas válidas (DD-MM-AAAA) para ver el rango que se escribirá en el documento.",
                                )
                                .size(13.0)
                                .color(PAPER_WARN),
                            );
                        }
                        ui.add_space(14.0);

                        // Sin novedades: nota en lugar de la tabla.
                        if items.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "Aún no hay novedades cargadas. Cargue el .zip exportado del chat de WhatsApp y aparecerán aquí, en el mismo orden del documento final.",
                                )
                                .size(13.0)
                                .color(PAPER_MUTED),
                            );
                            ui.add_space(14.0);
                        }

                        // Novedades, en el mismo orden del documento.
                        for (k, n) in items.iter().enumerate() {
                            if k > 0 {
                                rule(ui, PAPER_RULE);
                                ui.add_space(14.0);
                            }
                            ui.horizontal_top(|ui| {
                                // Columna izquierda: encabezados + fotos.
                                ui.vertical(|ui| {
                                    ui.set_width(PREVIEW_LEFT_W);
                                    ui.vertical_centered(|ui| {
                                        if n.headers.is_empty() {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Novedad #{}",
                                                    n.id
                                                ))
                                                .strong()
                                                .size(13.0)
                                                .color(PAPER_TEXT),
                                            );
                                        } else {
                                            for h in &n.headers {
                                                ui.label(
                                                    egui::RichText::new(h)
                                                        .strong()
                                                        .size(13.0)
                                                        .color(PAPER_TEXT),
                                                );
                                            }
                                        }
                                        ui.add_space(8.0);
                                        for ph in &n.photos {
                                            let key = format!("thumb-{ph}");
                                            let tex = if let Some(t) =
                                                self.textures.get(&key).cloned()
                                            {
                                                Some(t)
                                            } else {
                                                self.find_media(ph)
                                                    .cloned()
                                                    .and_then(|b| self.thumb(ctx, &key, &b))
                                            };
                                            match tex {
                                                Some(tex) => {
                                                    ui.add(
                                                        egui::Image::new(&tex)
                                                            .max_width(160.0)
                                                            .rounding(2.0),
                                                    );
                                                    ui.add_space(6.0);
                                                }
                                                None => {
                                                    ui.label(
                                                        egui::RichText::new(format!("🖼 {ph}"))
                                                            .size(11.0)
                                                            .color(PAPER_MUTED),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                // Columna derecha: cuerpo + cierre.
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width());
                                    if !n.formalizado {
                                        ui.label(
                                            egui::RichText::new("Sin formalizar")
                                                .size(10.5)
                                                .strong()
                                                .color(PAPER_WARN),
                                        );
                                        ui.add_space(2.0);
                                    }
                                    for p in &n.paras {
                                        ui.label(
                                            egui::RichText::new(p)
                                                .size(13.0)
                                                .color(PAPER_TEXT),
                                        );
                                        ui.add_space(6.0);
                                    }
                                    if let Some(cl) = &n.closing {
                                        if !cl.is_empty() {
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(cl)
                                                    .size(13.0)
                                                    .color(PAPER_TEXT),
                                            );
                                        }
                                    }
                                });
                            });
                            ui.add_space(14.0);
                        }

                        // Pie de la hoja.
                        rule(ui, PAPER_RULE);
                        ui.add_space(6.0);
                        ui.vertical_centered(|ui| {
                            let mut pie = format!(
                                "{} novedades incluidas, {} fotos",
                                items.len(),
                                n_fotos
                            );
                            if excluidas > 0 {
                                pie.push_str(&format!(
                                    ", {excluidas} excluidas (no aparecen)"
                                ));
                            }
                            ui.label(
                                egui::RichText::new(pie)
                                    .size(11.5)
                                    .color(PAPER_MUTED),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "Generado con Parte Diario v{}",
                                    parte_diario_core::build_info::VERSION
                                ))
                                .size(11.0)
                                .color(PAPER_MUTED),
                            );
                        });
                    });
            });
            ui.add_space(16.0);
        });
    }
}
