//! Interfaz de escritorio (egui/eframe) — rediseño v3, Concepto A "Esencial".
//!
//! Tres columnas, sin banda superior:
//! - Barra lateral (200 px): marca "PD", navegación y "Turno de hoy".
//! - Centro: stepper Cargar → Analizar → Extraer → Generar → Editar →
//!   Exportar; estado vacío con zona de arrastre, o novedades + editor, o
//!   vista previa del documento.
//! - Inspector derecho "Resumen" (252 px): estado, fechas, mini vista
//!   previa y el botón dorado "Generar Parte Diario".
//!
//! El sistema de diseño vive en `gui/theme.rs` (tokens y criterio en
//! `docs/diseno-concepto-a.md`) y la vista previa en `gui/preview.rs`;
//! este archivo guarda la lógica de la app.

mod preview;
mod theme;

use self::theme::{
    apply_theme, banner, btn_action, btn_gold, btn_primary, btn_primary_enabled, btn_secondary,
    btn_secondary_enabled, btn_tertiary, card, check_badge, dashed_rect, font_bold, font_semibold,
    overline, pal, pill, pill_tint, section_title, seg_button, set_palette, stepper, StepState,
};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use chrono::NaiveDate;
use eframe::egui;

use parte_diario_ai as llm;
use parte_diario_config as history;
use parte_diario_docx as docx_editor;
use parte_diario_parser::grouping;
use parte_diario_parser::parser;

/// Vista del panel central.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    /// Editor de la novedad seleccionada.
    Edit,
    /// Vista previa del documento completo, antes de generar.
    Preview,
}

pub struct App {
    cfg: history::Config,
    hist: history::History,
    zip_path: Option<PathBuf>,
    /// SHA-256 del .zip, calculado una sola vez al cargar y reutilizado al
    /// generar el reporte (evita una segunda lectura completa del archivo).
    zip_hash: String,
    /// SHA-256 del texto del chat normalizado, calculado al cargar. Sirve para
    /// detección de duplicados por contenido (aunque el .zip tenga otro nombre)
    /// y para registrarlo en el historial.
    txt_hash: String,
    /// Bytes de las FOTOS en memoria (nombre → bytes). Los videos no se
    /// almacenan: solo se registra su nombre para marcarlos como omitidos.
    media_files: HashMap<String, Vec<u8>>,
    chat: Option<parser::ParsedChat>,
    novedades: Vec<grouping::Novedad>,
    selected: usize,
    template_path: Option<PathBuf>,
    date_from: String,
    date_to: String,
    log: Vec<String>,
    warnings: Vec<String>,
    info: Vec<String>,
    pending_llm: Vec<(usize, Receiver<Result<String, String>>)>,
    llm_busy: HashSet<usize>,
    llm_batch_total: usize,
    /// Deshacer de la última formalización masiva ("Formalizar todas" o
    /// "Limpieza a todas"): (etiqueta de la acción, textos formales previos
    /// por id de novedad). Un solo nivel de deshacer.
    undo_formal: Option<(String, Vec<(usize, String)>)>,
    textures: HashMap<String, egui::TextureHandle>,
    show_settings: bool,
    api_key_edit: String,
    provider_edit: String,
    model_edit: String,
    output_dir_edit: String,
    anonymize_edit: bool,
    update_repository_edit: String,
    check_updates_on_startup_edit: bool,
    /// Borradores de los ajustes de apariencia (se aplican al Guardar).
    dark_mode_edit: bool,
    compact_view_edit: bool,
    /// Id del campo de búsqueda, para enfocarlo con Ctrl+F.
    search_field_id: Option<egui::Id>,
    available_update: Option<parte_diario_updater::UpdateInfo>,
    update_rx: Option<Receiver<Result<Option<parte_diario_updater::UpdateInfo>, String>>>,
    update_action_rx: Option<Receiver<Result<String, String>>>,
    update_busy: bool,
    update_message: String,
    generated_path: Option<PathBuf>,
    success_msg: Option<String>,
    orphan_target: HashMap<usize, usize>,
    replace_armed: bool,
    replace_range: (String, String),
    // UI
    filter: String,
    show_log: bool,
    alerts_expanded: bool,
    view: ViewMode,
    /// El usuario ya abrió la vista previa (paso "Editar" del flujo).
    preview_seen: bool,
    /// El .docx generado ya se abrió (paso "Exportar" del flujo).
    exported: bool,
    /// Ventana del historial de partes generados.
    show_history: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // La configuración se carga ANTES de aplicar el tema, porque el
        // tema (claro/oscuro) y la densidad (compacta) viven en ella.
        let cfg = history::load_config();
        // egui 0.29 sigue el tema del sistema por defecto (Windows en modo
        // oscuro ⇒ Visuals::dark ⇒ paneles oscuros mezclados con los
        // widgets claros). Se fijan AMBOS estilos al tema elegido para que
        // ningún cambio de tema del OS pueda romper la interfaz.
        cc.egui_ctx.set_theme(egui::ThemePreference::Light);
        // Inter (Regular / SemiBold / Bold) desde assets/fonts/.
        theme::install_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, cfg.dark_mode, cfg.compact_view);
        let hist = history::load_history();
        let template_path = if cfg.last_template.is_empty() {
            None
        } else {
            let p = PathBuf::from(&cfg.last_template);
            if p.exists() {
                Some(p)
            } else {
                None
            }
        };
        let auto_check = cfg.check_updates_on_startup && !cfg.update_repository.trim().is_empty();
        let mut app = Self {
            api_key_edit: cfg.api_key.clone(),
            provider_edit: cfg.provider.clone(),
            model_edit: cfg.model.clone(),
            output_dir_edit: cfg.output_dir.clone(),
            anonymize_edit: cfg.anonymize,
            update_repository_edit: cfg.update_repository.clone(),
            check_updates_on_startup_edit: cfg.check_updates_on_startup,
            dark_mode_edit: cfg.dark_mode,
            compact_view_edit: cfg.compact_view,
            search_field_id: None,
            available_update: None,
            update_rx: None,
            update_action_rx: None,
            update_busy: false,
            update_message: String::new(),
            cfg,
            hist,
            template_path,
            zip_path: None,
            zip_hash: String::new(),
            txt_hash: String::new(),
            media_files: HashMap::new(),
            chat: None,
            novedades: Vec::new(),
            selected: 0,
            date_from: String::new(),
            date_to: String::new(),
            log: vec!["Listo. Cargue el .zip exportado de WhatsApp para comenzar.".to_string()],
            warnings: Vec::new(),
            info: Vec::new(),
            pending_llm: Vec::new(),
            llm_busy: HashSet::new(),
            llm_batch_total: 0,
            undo_formal: None,
            textures: HashMap::new(),
            show_settings: false,
            generated_path: None,
            success_msg: None,
            orphan_target: HashMap::new(),
            replace_armed: false,
            replace_range: (String::new(), String::new()),
            filter: String::new(),
            show_log: false,
            alerts_expanded: false,
            view: ViewMode::Edit,
            preview_seen: false,
            exported: false,
            show_history: false,
        };
        if auto_check {
            app.start_update_check(&cc.egui_ctx);
        }
        app
    }

    fn poll_updates(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.update_rx.take() {
            match rx.try_recv() {
                Ok(Ok(info)) => {
                    self.update_busy = false;
                    if let Some(info) = info {
                        self.update_message =
                            format!("Nueva versión disponible: v{}", info.latest_version);
                        self.log(self.update_message.clone());
                        self.available_update = Some(info);
                    } else {
                        self.update_message = "Ya tiene la última versión disponible.".to_string();
                        self.log(self.update_message.clone());
                        self.available_update = None;
                    }
                }
                Ok(Err(e)) => {
                    self.update_busy = false;
                    self.update_message = format!("No se pudo buscar la actualización: {e}");
                    self.log(self.update_message.clone());
                }
                Err(TryRecvError::Empty) => self.update_rx = Some(rx),
                Err(TryRecvError::Disconnected) => {
                    self.update_busy = false;
                    self.update_message =
                        "La búsqueda de actualización se interrumpió.".to_string();
                }
            }
        }
        if let Some(rx) = self.update_action_rx.take() {
            match rx.try_recv() {
                Ok(Ok(msg)) => {
                    self.update_busy = false;
                    self.update_message = msg.clone();
                    self.log(msg);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Ok(Err(e)) => {
                    self.update_busy = false;
                    self.update_message = format!("Actualización cancelada: {e}");
                    self.log(self.update_message.clone());
                }
                Err(TryRecvError::Empty) => self.update_action_rx = Some(rx),
                Err(TryRecvError::Disconnected) => {
                    self.update_busy = false;
                    self.update_message = "La actualización se interrumpió.".to_string();
                }
            }
        }
    }

    fn start_update_check(&mut self, ctx: &egui::Context) {
        if self.update_busy {
            return;
        }
        let repository = self.update_repository_edit.trim().to_string();
        if repository.is_empty() {
            self.update_message =
                "Configure el repositorio de GitHub (usuario/repositorio) en Ajustes.".to_string();
            self.log(self.update_message.clone());
            return;
        }
        let current = parte_diario_core::build_info::VERSION.to_string();
        let (tx, rx) = channel();
        self.update_rx = Some(rx);
        self.update_busy = true;
        self.update_message = "Buscando actualizaciones…".to_string();
        self.log(self.update_message.clone());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = parte_diario_updater::check_for_update(&repository, &current)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn start_install_update(&mut self, ctx: &egui::Context) {
        if self.update_busy {
            return;
        }
        let Some(info) = self.available_update.clone() else {
            return;
        };
        let (tx, rx) = channel();
        self.update_action_rx = Some(rx);
        self.update_busy = true;
        self.update_message = "Descargando y verificando la actualización…".to_string();
        self.log(self.update_message.clone());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = parte_diario_updater::download_and_prepare(&info)
                .and_then(|exe| parte_diario_updater::apply_on_exit(&exe))
                .map(|()| "Actualización lista. La aplicación se reiniciará.".to_string())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn start_rollback(&mut self, ctx: &egui::Context) {
        if self.update_busy {
            return;
        }
        let (tx, rx) = channel();
        self.update_action_rx = Some(rx);
        self.update_busy = true;
        self.update_message = "Restaurando el respaldo anterior…".to_string();
        self.log(self.update_message.clone());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = parte_diario_updater::rollback()
                .map(|p| {
                    format!(
                        "Respaldo restaurado: {}. La aplicación se reiniciará.",
                        p.display()
                    )
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn log(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
        if self.log.len() > 300 {
            let n = self.log.len() - 300;
            self.log.drain(..n);
        }
    }

    // ---------- selección de archivos ----------

    fn pick_zip(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("ZIP de WhatsApp", &["zip"])
            .pick_file()
        {
            self.load_zip(&p);
        }
    }

    fn pick_template(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Plantilla Word", &["docx"])
            .pick_file()
        {
            self.set_template(&p);
        }
    }

    fn set_template(&mut self, p: &Path) {
        self.template_path = Some(p.to_path_buf());
        self.cfg.last_template = p.to_string_lossy().to_string();
        let _ = history::save_config(&self.cfg);
        self.log(format!("Plantilla: {}", p.display()));
    }

    fn accept_dropped_file(&mut self, p: &Path) {
        match p
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
        {
            Some(ref ext) if ext == "zip" => {
                self.log(format!("Archivo soltado: cargando {}…", p.display()));
                self.load_zip(p);
            }
            Some(ref ext) if ext == "docx" => {
                self.log(format!(
                    "Archivo soltado: usando {} como plantilla.",
                    p.display()
                ));
                self.set_template(p);
            }
            _ => self.log(format!(
                "Archivo '{}' no reconocido. Suelte un .zip del chat o una plantilla .docx.",
                p.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }

    // ---------- carga del .zip ----------

    fn load_zip(&mut self, path: &Path) {
        self.log.clear();
        self.warnings.clear();
        self.info.clear();
        self.novedades.clear();
        self.textures.clear();
        self.generated_path = None;
        self.success_msg = None;
        self.replace_armed = false;
        self.undo_formal = None;
        self.filter.clear();
        self.zip_path = None;

        let zip_hash = match history::sha256_file(path) {
            Ok(h) => {
                self.log(format!("SHA-256 del .zip: {}", &h[..16.min(h.len())]));
                h
            }
            Err(e) => {
                self.log(e);
                return;
            }
        };
        self.zip_hash = zip_hash.clone();

        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                self.log(format!("No se pudo abrir el .zip: {e}"));
                return;
            }
        };
        let mut zip = match zip::ZipArchive::new(file) {
            Ok(z) => z,
            Err(e) => {
                self.log(format!("El archivo no es un .zip válido: {e}"));
                return;
            }
        };
        self.zip_path = Some(path.to_path_buf());

        // Indexar .txt y FOTOS en memoria (sin extraer a disco). Los videos solo
        // se registran por nombre: leer sus bytes podía consumir cientos de MB
        // y luego se descartaban de todos modos.
        const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];
        const VIDEO_EXTS: &[&str] = &["mp4", "mov", "3gp", "avi", "mkv", "webm"];
        let mut txt_candidates: Vec<(u64, String, Vec<u8>)> = Vec::new();
        let mut media_files: HashMap<String, Vec<u8>> = HashMap::new();
        let mut media_names: HashSet<String> = HashSet::new();
        for i in 0..zip.len() {
            let Ok(mut f) = zip.by_index(i) else {
                continue;
            };
            if f.is_dir() {
                continue;
            }
            let Some(enc) = f.enclosed_name() else {
                continue;
            };
            let name = match enc.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_lowercase(),
                None => continue,
            };
            let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
            if ext == "txt" {
                let zip_name = f
                    .name()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default();
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    txt_candidates.push((buf.len() as u64, zip_name, buf));
                }
                continue;
            }
            if IMAGE_EXTS.contains(&ext) {
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    // Clave insensible a mayúsculas: Windows.zip a veces difiere
                    // en el caso respecto al .txt exportado.
                    media_files.insert(name.clone(), buf);
                    media_names.insert(name);
                }
            } else if VIDEO_EXTS.contains(&ext) {
                media_names.insert(name);
            }
        }
        self.media_files = media_files;

        let txt_path =
            if let Some((_, p, bytes)) = txt_candidates.into_iter().max_by_key(|(s, _, _)| *s) {
                self.log(format!(
                    "Historial: {} ({} mensajes aprox.)",
                    std::path::Path::new(&p)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    {
                        let preview = String::from_utf8_lossy(&bytes);
                        preview.lines().count()
                    }
                ));
                String::from_utf8_lossy(&bytes).to_string()
            } else {
                self.log("No se encontró ningún .txt dentro del .zip.".to_string());
                self.zip_path = None;
                self.media_files.clear();
                return;
            };
        let txt = txt_path;

        // Parseo + agrupación.
        let chat = parser::parse_chat_txt(&txt);
        let fmt = match chat.format {
            parser::ChatFormat::Bracket => "corchete (Android)",
            parser::ChatFormat::Dash => "guion (iOS)",
            parser::ChatFormat::Unknown => "desconocido (se intentó parsear igual)",
        };
        self.log(format!(
            "Formato detectado: {}. Mensajes útiles: {}.",
            fmt,
            chat.messages.len()
        ));
        let mut novs = grouping::group_novedades(&chat, &media_names);

        // Diagnóstico de fotos: referenciadas en el chat vs. presentes en el .zip.
        {
            let media_lower: HashSet<String> =
                self.media_files.keys().map(|k| k.to_lowercase()).collect();
            let mut refs: HashSet<String> = HashSet::new();
            let mut seen_missing: HashSet<String> = HashSet::new();
            for m in &chat.messages {
                if let Some(att) = &m.attachment {
                    if att.kind == parser::AttachmentKind::Image {
                        refs.insert(att.filename.clone());
                        let key = att.filename.to_lowercase();
                        if !media_lower.contains(&key) && seen_missing.insert(key) {
                            self.warnings.push(format!(
                                "La foto '{}' está referenciada en el chat pero NO se encontró dentro del .zip. Exporte de nuevo con 'Adjuntar archivos multimedia'.",
                                att.filename
                            ));
                        }
                    }
                }
            }
            self.log(format!(
                "Multimedia en el .zip: {} archivo(s); fotos referenciadas en el chat: {}.",
                media_names.len(),
                refs.len()
            ));
        }

        self.chat = Some(chat);

        // Control de duplicados: mismo .zip o mismo contenido de texto.
        let txt_hash = history::sha256_bytes(history::normalize_txt(&txt).as_bytes());
        self.txt_hash = txt_hash.clone();
        if let Some(prev) = history::already_processed(&self.hist, &zip_hash, &txt_hash) {
            self.warnings.push(format!(
                "¡ATENCIÓN! Este .zip (o su contenido de texto) ya fue procesado el {} y generó el reporte {} a {} ({} novedades). \
                 Si lo regenera, verifique que no esté duplicando trabajo por confusión de nombres de archivo.",
                prev.generated_at, prev.report_start, prev.report_end, prev.novedades
            ));
        }

        // Pre-selección: solo novedades con fecha posterior al último reporte.
        let last_end = history::last_report_end(&self.hist);
        let mut nuevas = 0;
        for n in &mut novs {
            let inc = match (n.fecha_mencionada, last_end) {
                (Some(f), Some(le)) => f > le,
                _ => true,
            };
            n.include = inc;
            if inc {
                nuevas += 1;
            }
        }
        if let Some(le) = last_end {
            self.info.push(format!(
                "Pre-selección automática: {} de {} novedades son posteriores al último reporte ({}). Las anteriores quedaron desmarcadas; puede volver a marcarlas si las necesita.",
                nuevas,
                novs.len(),
                le.format("%d-%m-%Y")
            ));
        }

        // Inferencia de fechas + inconsistencias.
        let (majority, outliers) = grouping::infer_report_range(&novs);
        if let Some(m) = majority {
            self.date_from = m.format("%d-%m-%Y").to_string();
            self.date_to = m.succ_opt().unwrap_or(m).format("%d-%m-%Y").to_string();
            self.info.push(format!(
                "Fecha inferida del cuerpo de las notas: {} (reporte desde las 08:00 del {} hasta las 08:00 del {}).",
                m.format("%d-%m-%Y"),
                self.date_from,
                self.date_to
            ));
        } else {
            self.warnings.push(
                "No se pudo inferir la fecha del reporte desde el texto; ingrésela manualmente."
                    .to_string(),
            );
        }
        // Inconsistencias de fecha, agrupadas por fecha para no spamear.
        {
            let mut by_date_warn: HashMap<String, Vec<usize>> = HashMap::new();
            let mut by_date_info: HashMap<String, Vec<usize>> = HashMap::new();
            for oid in outliers {
                if let Some(n) = novs.iter().find(|n| n.id == oid) {
                    let key = n
                        .fecha_mencionada
                        .map(|d| d.format("%d-%m-%Y").to_string())
                        .unwrap_or_else(|| "?".to_string());
                    let adjacent = match (n.fecha_mencionada, majority) {
                        (Some(f), Some(m)) => (f - m).num_days().abs() == 1,
                        _ => false,
                    };
                    if adjacent {
                        by_date_info.entry(key).or_default().push(oid);
                    } else {
                        by_date_warn.entry(key).or_default().push(oid);
                    }
                }
            }
            let maj_txt = majority
                .map(|m| m.format("%d-%m-%Y").to_string())
                .unwrap_or_else(|| "?".to_string());
            let fmt_ids = |ids: &Vec<usize>| {
                ids.iter()
                    .map(|i| format!("#{i}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            for (f, ids) in by_date_info {
                self.info.push(format!(
                    "Turno nocturno: {} novedad(es) ({}) mencionan el {} (día adyacente a la mayoría, {}); probablemente son del mismo operativo que cruzó la medianoche. Verifique que correspondan a este reporte.",
                    ids.len(),
                    fmt_ids(&ids),
                    f,
                    maj_txt
                ));
            }
            for (f, ids) in by_date_warn {
                self.warnings.push(format!(
                    "Posible inconsistencia: {} novedad(es) ({}) mencionan la fecha {} (distinta a la mayoría, {}). ¿Error de dedo del agente? Desmarque las que no correspondan a este reporte o corrija la fecha en el texto.",
                    ids.len(),
                    fmt_ids(&ids),
                    f,
                    maj_txt
                ));
            }
        }

        // Huérfanas y videos.
        let orphans = novs.iter().filter(|n| n.is_orphan).count();
        if orphans > 0 {
            self.warnings.push(format!(
                "{orphans} grupo(s) de fotos sin texto asociado (posible mensaje borrado). Revise la sección de huérfanas y reasígnelas a la novedad correcta."
            ));
        }
        let videos: usize = novs.iter().map(|n| n.videos_skipped.len()).sum();
        if videos > 0 {
            self.info.push(format!(
                "{videos} video(s) detectados: se excluyen del .docx (Word no los incrusta de forma práctica), pero sus novedades se conservan."
            ));
        }

        self.novedades = novs;
        self.selected = 0;
        self.log(format!(
            "Detectadas {} novedades. Revíselas en la lista antes de generar el documento.",
            self.novedades.len()
        ));
    }

    // ---------- formalización ----------

    fn spawn_formalize(&mut self, ids: Vec<usize>, use_ai: bool) {
        for id in ids {
            if self.llm_busy.contains(&id) {
                continue;
            }
            let text = match self.novedades.iter().find(|n| n.id == id) {
                Some(n) => n.body_raw.clone(),
                None => continue,
            };
            if text.trim().is_empty() {
                continue;
            }
            let (tx, rx) = channel();
            self.pending_llm.push((id, rx));
            self.llm_busy.insert(id);
            let api_key = self.cfg.api_key.clone();
            let provider = llm::LlmProvider::from_config(&self.cfg.provider);
            let model = self.cfg.model.clone();
            let anonymize = self.cfg.anonymize;
            std::thread::spawn(move || {
                let res = if use_ai {
                    llm::formalize_with_privacy(&api_key, provider, &model, &text, anonymize)
                } else {
                    Ok(llm::cleanup_offline(&text))
                };
                let _ = tx.send(res);
            });
        }
        self.llm_batch_total = self.llm_busy.len();
    }

    /// Formaliza varias novedades con concurrencia limitada. OpenRouter usa un
    /// solo hilo y 3 s entre peticiones (~20/min, el límite del plan gratuito);
    /// Anthropic/FreeLLMApi pueden usar más hilos.
    fn spawn_formalize_many(&mut self, ids: Vec<usize>) {
        let items: Vec<(usize, String)> = ids
            .into_iter()
            .filter(|id| !self.llm_busy.contains(id))
            .filter_map(|id| {
                let n = self.novedades.iter().find(|n| n.id == id)?;
                let t = n.body_raw.clone();
                if t.trim().is_empty() {
                    None
                } else {
                    Some((id, t))
                }
            })
            .collect();
        if items.is_empty() {
            return;
        }
        let api_key = self.cfg.api_key.clone();
        let provider = llm::LlmProvider::from_config(&self.cfg.provider);
        let model = self.cfg.model.clone();
        let anonymize = self.cfg.anonymize;

        let mut channels = HashMap::new();
        for (id, _) in &items {
            let (txi, rxi) = channel();
            self.pending_llm.push((*id, rxi));
            self.llm_busy.insert(*id);
            channels.insert(*id, txi);
        }
        let total = items.len();

        // Lanzamos hilos que procesan la cola compartida. OpenRouter gratuito
        // es el más restrictivo: 1 hilo y pausa de 3 s antes de cada llamada.
        let queue = std::sync::Arc::new(std::sync::Mutex::new(items));
        let num_workers = match provider {
            llm::LlmProvider::OpenRouter => 1,
            llm::LlmProvider::Anthropic => 2,
            llm::LlmProvider::FreeLLMApi => 3,
        }
        .min(total);
        let request_pause = match provider {
            llm::LlmProvider::OpenRouter => std::time::Duration::from_secs(3),
            _ => std::time::Duration::ZERO,
        };
        for _ in 0..num_workers {
            let q = std::sync::Arc::clone(&queue);
            let ak = api_key.clone();
            let md = model.clone();
            let chan_map = channels.clone();
            std::thread::spawn(move || loop {
                let item = {
                    let mut guard = q.lock().unwrap();
                    guard.pop()
                };
                let Some((id, text)) = item else { break };
                if !request_pause.is_zero() {
                    std::thread::sleep(request_pause);
                }
                let res = llm::formalize_with_privacy(&ak, provider, &md, &text, anonymize);
                if let Some(txi) = chan_map.get(&id) {
                    let _ = txi.send(res);
                }
            });
        }

        self.llm_batch_total = self.llm_busy.len();
        self.log(format!(
            "Formalizando {} novedades en paralelo ({} hilos) con {}…",
            total,
            num_workers,
            provider.label()
        ));
    }

    /// Textos formales actuales de las novedades que una acción masiva va
    /// a reescribir: el punto de deshacer de esa acción.
    fn snapshot_formal(&self) -> Vec<(usize, String)> {
        self.novedades
            .iter()
            .filter(|n| n.include && !n.body_raw.trim().is_empty())
            .map(|n| (n.id, n.body_formal.clone()))
            .collect()
    }

    /// Restaura los textos formales guardados por la última acción masiva
    /// y consume el deshacer. Solo se llama cuando ninguna formalización
    /// está en curso, para que un resultado tardío no vuelva a pisar el
    /// texto restaurado.
    fn undo_bulk_formalize(&mut self) {
        let Some((label, prev)) = self.undo_formal.take() else {
            return;
        };
        let mut k = 0;
        for (id, text) in prev {
            if let Some(n) = self.novedades.iter_mut().find(|n| n.id == id) {
                n.body_formal = text;
                k += 1;
            }
        }
        self.log(format!(
            "Deshecha la acción «{label}»: se restauraron {k} textos formales."
        ));
    }

    fn poll_llm(&mut self) {
        let mut results: Vec<(usize, Result<String, String>)> = Vec::new();
        self.pending_llm.retain(|(id, rx)| match rx.try_recv() {
            Ok(res) => {
                results.push((*id, res));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => {
                results.push((
                    *id,
                    Err("El hilo de formalización terminó sin responder.".to_string()),
                ));
                false
            }
        });
        for (id, res) in results {
            self.llm_busy.remove(&id);
            match res {
                Ok(t) => {
                    if let Some(n) = self.novedades.iter_mut().find(|n| n.id == id) {
                        n.body_formal = t;
                    }
                    self.log(format!(
                        "Novedad #{id}: texto formal generado. Revíselo antes de generar."
                    ));
                }
                Err(e) => self.log(format!("Novedad #{id}: error al formalizar: {e}")),
            }
        }
        if self.llm_busy.is_empty() {
            self.llm_batch_total = 0;
        }
    }

    fn refresh_novedad(n: &mut grouping::Novedad) {
        grouping::refresh_metadata(n);
    }

    /// Salta a la siguiente novedad incluida que aún no tiene texto formal.
    fn next_pending(&mut self) {
        let n = self.novedades.len();
        if n == 0 {
            return;
        }
        for k in 1..=n {
            let i = (self.selected + k) % n;
            let p = self.novedades[i].include
                && self.novedades[i].body_formal.trim().is_empty()
                && !self.novedades[i].body_raw.trim().is_empty();
            if p {
                self.selected = i;
                return;
            }
        }
        self.log("No quedan novedades pendientes de formalizar. Todo al día.");
    }

    /// Navegación desde el stepper: cada paso clicable (hecho o actual)
    /// lleva a su pantalla/acción. Los pendientes no llegan aquí (el
    /// stepper no los devuelve), así que no se puede saltar a un paso que
    /// aún no se alcanzó.
    fn goto_step(&mut self, idx: usize) {
        match idx {
            // Cargar: abrir el selector del .zip.
            0 => self.pick_zip(),
            // Analizar: la lista de novedades.
            1 => self.view = ViewMode::Edit,
            // Extraer: el editor, en la siguiente novedad sin formalizar.
            2 => {
                self.view = ViewMode::Edit;
                self.next_pending();
            }
            // Generar: igual que el botón dorado del inspector.
            3 => self.generate(),
            // Editar: la vista previa del documento.
            4 => {
                self.view = ViewMode::Preview;
                self.preview_seen = true;
            }
            // Exportar: abrir el .docx generado (si existe).
            5 => {
                if let Some(gp) = self.generated_path.clone() {
                    match open::that(&gp) {
                        Ok(()) => self.exported = true,
                        Err(e) => self.log(format!("No se pudo abrir el archivo: {e}")),
                    }
                }
            }
            _ => {}
        }
    }

    // ---------- generación ----------

    fn generate(&mut self) {
        let Some(desde) = parse_fecha(&self.date_from) else {
            self.warnings
                .push("GENERAR: la fecha 'desde' no es válida (use DD-MM-AAAA).".to_string());
            return;
        };
        let Some(hasta) = parse_fecha(&self.date_to) else {
            self.warnings
                .push("GENERAR: la fecha 'hasta' no es válida (use DD-MM-AAAA).".to_string());
            return;
        };
        if hasta < desde {
            self.warnings
                .push("GENERAR: la fecha 'hasta' es anterior a la fecha 'desde'.".to_string());
            return;
        }
        if !self.novedades.iter().any(|n| n.include) {
            self.warnings.push(
                "GENERAR: no hay ninguna novedad incluida. Marque al menos una antes de generar."
                    .to_string(),
            );
            return;
        }

        // Re-analizar por si el usuario editó textos.
        for n in &mut self.novedades {
            Self::refresh_novedad(n);
        }

        let start_s = desde.format("%Y-%m-%d").to_string();
        let end_s = hasta.format("%Y-%m-%d").to_string();

        // ¿Ya existe un reporte para este rango? Pedir confirmación para reemplazar.
        if !(self.replace_armed && self.replace_range == (start_s.clone(), end_s.clone())) {
            if let Some(prev) = history::existing_report_for(&self.hist, &start_s, &end_s) {
                self.replace_armed = true;
                self.replace_range = (start_s.clone(), end_s.clone());
                let prev_path = prev.output_path.clone();
                self.warnings.push(format!(
                    "Generar: ya existe un reporte para {} a {} ({}). Si el contenido anterior era incorrecto, pulse Generar otra vez para reemplazarlo (se sobrescribirá: {}).",
                    start_s, end_s, prev.generated_at, prev_path
                ));
                return;
            }
        }
        self.replace_armed = false;

        // Medios de las novedades incluidas: se usan DIRECTAMENTE desde la
        // caché en memoria, sin clonar todo el mapa ni volver a tocar el disco.
        let missing: Vec<String> = self
            .novedades
            .iter()
            .filter(|n| n.include)
            .flat_map(|n| n.photos.iter())
            .filter(|ph| self.find_media(ph).is_none())
            .cloned()
            .collect();
        for ph in missing {
            self.log(format!(
                "La foto {ph} no se encontró entre los archivos del .zip."
            ));
        }

        let out_dir = if self.cfg.output_dir.is_empty() {
            self.zip_path
                .as_ref()
                .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
                .unwrap_or_else(std::env::temp_dir)
        } else {
            PathBuf::from(&self.cfg.output_dir)
        };
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            self.warnings.push(format!(
                "GENERAR: no se pudo crear la carpeta de salida: {e}"
            ));
            return;
        }
        let out_name = format!(
            "Parte_Diario_{}_{}.docx",
            desde.format("%d-%m-%Y"),
            hasta.format("%d-%m-%Y")
        );
        let out_path = out_dir.join(out_name);

        match docx_editor::build_report(
            self.template_path.as_deref(),
            &out_path,
            &self.novedades,
            desde,
            hasta,
            &self.media_files,
        ) {
            Ok(res) => {
                for w in res.warnings {
                    self.warnings.push(format!("DOCX: {w}"));
                }
                self.log(format!(
                    "Parte Diario generado: {} ({} novedades, {} fotos incrustadas). Validación OOXML interna superada.",
                    out_path.display(),
                    res.novedades_incluidas,
                    res.images_embedded
                ));
                self.generated_path = Some(out_path.clone());
                self.success_msg = Some(format!(
                    "Parte Diario generado: {} novedades y {} fotos incrustadas.\nArchivo: {}",
                    res.novedades_incluidas,
                    res.images_embedded,
                    out_path
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                // Historial: reutilizar el SHA-256 del zip ya calculado al cargar
                // y guardar el txt_sha256 para que la detección de duplicados
                // por contenido funcione aunque el .zip se renombre.
                let entry = history::HistoryEntry {
                    zip_sha256: self.zip_hash.clone(),
                    txt_sha256: self.txt_hash.clone(),
                    zip_name: self
                        .zip_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    report_start: start_s,
                    report_end: end_s,
                    generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    output_path: out_path.to_string_lossy().to_string(),
                    novedades: res.novedades_incluidas,
                };
                // Si se reemplazó, quitar la entrada anterior del mismo rango.
                self.hist.entries.retain(|e| {
                    !(e.report_start == entry.report_start && e.report_end == entry.report_end)
                });
                self.hist.entries.push(entry);
                if let Err(e) = history::save_history(&self.hist) {
                    self.log(format!("No se pudo guardar el historial: {e}"));
                }
            }
            Err(e) => {
                self.warnings.push(format!("GENERAR: {e}"));
            }
        }
    }

    /// Busca un archivo multimedia por nombre, insensible a mayúsculas/
    /// minúsculas (los .zip de Windows a veces difieren en el caso).
    fn find_media(&self, name: &str) -> Option<&Vec<u8>> {
        if let Some(b) = self.media_files.get(name) {
            return Some(b);
        }
        let lower = name.to_lowercase();
        self.media_files
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v)
    }

    // ---------- miniaturas ----------

    fn thumb(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        bytes: &[u8],
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(key) {
            return Some(t.clone());
        }
        let img = image::load_from_memory(bytes).ok()?;
        let thumb = img.resize(240, 240, image::imageops::FilterType::Triangle);
        let rgb = thumb.to_rgb8();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let pixels = rgb
            .pixels()
            .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
            .collect();
        let ci = egui::ColorImage {
            size: [w, h],
            pixels,
        };
        let tex = ctx.load_texture(key.to_string(), ci, egui::TextureOptions::default());
        self.textures.insert(key.to_string(), tex.clone());
        Some(tex)
    }

    fn novedad_title(n: &grouping::Novedad) -> String {
        let base = if n.is_orphan {
            format!("#{} fotos huérfanas ({} fotos)", n.id, n.photos.len())
        } else if let Some(h) = n.headers.first() {
            let short: String = h.chars().take(42).collect();
            format!("#{} {}", n.id, short)
        } else {
            let short: String = n.body_raw.chars().take(42).collect();
            format!("#{} {}", n.id, short.replace('\n', " "))
        };
        let fecha = n
            .fecha_mencionada
            .map(|d| format!(", {}", d.format("%d/%m")))
            .unwrap_or_default();
        format!("{}{}, 📷{}", base, fecha, n.photos.len())
    }

    /// Extracto corto para la tarjeta de la lista (sin insignias redundantes).
    fn novedad_snippet(n: &grouping::Novedad) -> String {
        let body = if n.is_orphan {
            "Fotos sin texto asociado".to_string()
        } else if let Some(h) = n.headers.first() {
            h.clone()
        } else {
            n.body_raw.replace('\n', " ")
        };
        let mut s: String = body.chars().take(48).collect();
        if body.chars().count() > 48 {
            s.push('…');
        }
        if s.trim().is_empty() {
            "(sin texto)".to_string()
        } else {
            s
        }
    }

    /// Estado de los 6 pasos del flujo guiado, para el indicador de pasos
    /// de la barra superior.
    /// Estado de los 6 pasos del flujo (Concepto A).
    ///
    /// Mapeo a los pasos reales de la app:
    /// - Cargar: .zip del chat cargado.
    /// - Analizar: hay novedades para revisar.
    /// - Extraer: las novedades incluidas ya están formalizadas.
    /// - Generar: el .docx ya se generó.
    /// - Editar: el usuario ya abrió la vista previa del documento.
    /// - Exportar: el .docx generado ya se abrió.
    fn step_states(&self) -> [(String, StepState); 6] {
        let zip_ok = self.zip_path.is_some();
        let rev_ok = !self.novedades.is_empty();
        let form_ok = {
            let pendientes: Vec<&grouping::Novedad> = self
                .novedades
                .iter()
                .filter(|n| n.include && !n.body_raw.trim().is_empty())
                .collect();
            !pendientes.is_empty() && pendientes.iter().all(|n| !n.body_formal.trim().is_empty())
        };
        let gen_ok = self.generated_path.is_some();
        let edit_ok = self.preview_seen;
        let exp_ok = self.exported;
        let done = [zip_ok, rev_ok, form_ok, gen_ok, edit_ok, exp_ok];
        let current = done.iter().position(|d| !*d).unwrap_or(6);
        let labels = [
            "Cargar", "Analizar", "Extraer", "Generar", "Editar", "Exportar",
        ];
        std::array::from_fn(|i| {
            let state = if done[i] {
                StepState::Done
            } else if i == current {
                StepState::Current
            } else {
                StepState::Pending
            };
            (labels[i].to_string(), state)
        })
    }

    /// Lista de índices visibles según el filtro de búsqueda. Busca en el
    /// texto original, en el texto formal y en los encabezados, además del
    /// número de novedad con #N.
    fn visible_novedades(&self) -> Vec<usize> {
        let q = self.filter.trim().to_lowercase();
        if q.is_empty() {
            return (0..self.novedades.len()).collect();
        }
        let id_q = q.trim_start_matches('#');
        (0..self.novedades.len())
            .filter(|&i| {
                let n = &self.novedades[i];
                format!("{}", n.id) == id_q
                    || n.body_raw.to_lowercase().contains(&q)
                    || n.body_formal.to_lowercase().contains(&q)
                    || n.headers.join(" ").to_lowercase().contains(&q)
            })
            .collect()
    }
}

/// Nombre corto del modelo de IA para el inspector (p. ej.
/// "nemotron-3-ultra-550b-a55b:free" desde
/// "nvidia/nemotron-3-ultra-550b-a55b:free").
fn short_model(m: &str) -> String {
    let m = m.trim();
    let short = m.rsplit('/').next().unwrap_or(m);
    let chars: Vec<char> = short.chars().collect();
    if chars.len() > 30 {
        chars[..27].iter().collect::<String>() + "…"
    } else {
        short.to_string()
    }
}

fn parse_fecha(s: &str) -> Option<NaiveDate> {
    let s = s.trim().replace('/', "-");
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let (d, m, y) = (
        parts[0].parse::<u32>().ok()?,
        parts[1].parse::<u32>().ok()?,
        parts[2].parse::<i32>().ok()?,
    );
    let y = if y < 100 {
        if y < 70 {
            2000 + y
        } else {
            1900 + y
        }
    } else {
        y
    };
    NaiveDate::from_ymd_opt(y, m, d)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Reponer la paleta activa en cada fotograma (el dato temporal del
        // Context puede limpiarse entre fotogramas).
        set_palette(ctx, self.cfg.dark_mode);
        self.poll_llm();
        self.poll_updates(ctx);

        // ================= Atajos de teclado =================
        // Solo cuando ningún campo de texto tiene el foco (no interferir
        // con la edición): Ctrl+O cargar .zip, Ctrl+G generar, Ctrl+F
        // buscar en las novedades.
        if !ctx.wants_keyboard_input() {
            let cmd = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
            if cmd && ctx.input(|i| i.key_pressed(egui::Key::O)) {
                self.pick_zip();
            } else if cmd && ctx.input(|i| i.key_pressed(egui::Key::G)) {
                self.generate();
            } else if cmd && ctx.input(|i| i.key_pressed(egui::Key::F)) {
                if let Some(id) = self.search_field_id {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
        }

        // ================= Arrastrar y soltar =================
        let hovered = ctx.input(|i| i.raw.hovered_files.clone());
        if !hovered.is_empty() {
            let rect = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0x1B, 0x3A, 0x6B, 220),
            );
            painter.rect_stroke(
                rect.shrink(24.0),
                12.0,
                egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Suelte el archivo aquí\n(.zip del chat de WhatsApp o plantilla .docx)",
                egui::FontId::proportional(20.0),
                egui::Color32::WHITE,
            );
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            if let Some(p) = f.path {
                self.accept_dropped_file(&p);
            }
        }

        if self.selected >= self.novedades.len() && !self.novedades.is_empty() {
            self.selected = 0;
        }

        // ================= Aviso de actualización =================
        if let Some(update) = self.available_update.clone() {
            egui::TopBottomPanel::top("update_banner")
                .frame(
                    egui::Frame::none()
                        .fill(theme::palette(ctx).info_tint)
                        .stroke(egui::Stroke::new(1.0_f32, theme::palette(ctx).accent_line))
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "⬆ Nueva versión v{} disponible (tiene v{}).",
                                update.latest_version,
                                parte_diario_core::build_info::VERSION
                            ))
                            .color(pal(ui).text_primary)
                            .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if btn_tertiary(ui, "Ahora no").clicked() {
                                self.available_update = None;
                            }
                            if btn_primary(ui, "Instalar y reiniciar").clicked() {
                                self.start_install_update(ui.ctx());
                            }
                            if self.update_busy {
                                ui.spinner();
                            }
                        });
                    });
                });
        }

        // ================= Barra de estado =================
        egui::TopBottomPanel::bottom("abajo").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let n_inc = self.novedades.iter().filter(|n| n.include).count();
                let n_fotos: usize = self
                    .novedades
                    .iter()
                    .filter(|n| n.include)
                    .map(|n| n.photos.len())
                    .sum();
                ui.label(
                    egui::RichText::new(format!(
                        "{} novedades, {} incluidas, {} fotos",
                        self.novedades.len(),
                        n_inc,
                        n_fotos
                    ))
                    .size(12.0)
                    .color(pal(ui).text_secondary),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "v{} ({})",
                        parte_diario_core::build_info::VERSION,
                        parte_diario_core::build_info::GIT_HASH
                    ))
                    .size(12.0)
                    .color(pal(ui).text_muted),
                );
                // Progreso de formalización en curso.
                if !self.llm_busy.is_empty() {
                    ui.separator();
                    ui.spinner();
                    let done = self.llm_batch_total.saturating_sub(self.llm_busy.len());
                    ui.label(
                        egui::RichText::new(format!(
                            "Formalizando… {} de {}",
                            done, self.llm_batch_total
                        ))
                        .size(12.0)
                        .color(pal(ui).accent),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if btn_tertiary(ui, &format!("Registro ({})", self.log.len())).clicked() {
                        self.show_log = !self.show_log;
                    }
                });
            });
        });

        // ================= Barra lateral (200 px) =================
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                // Marca "PD" + nombre de la app.
                ui.horizontal(|ui| {
                    egui::Frame::none()
                        .fill(pal(ui).accent)
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("PD")
                                    .font(font_bold(17.0))
                                    .color(pal(ui).on_accent),
                            );
                        });
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Parte Diario")
                                .font(font_semibold(15.0))
                                .color(pal(ui).text_primary),
                        );
                        ui.label(
                            egui::RichText::new("Anti-Ruidos y Contaminación Sónica")
                                .size(11.0)
                                .color(pal(ui).text_muted),
                        );
                    });
                });
                ui.add_space(16.0);

                // Navegación: acciones del flujo (no vistas separadas).
                if ui.selectable_label(true, "+  Nuevo parte").clicked() {
                    self.pick_zip();
                }
                if ui.selectable_label(false, "Historial").clicked() {
                    self.show_history = true;
                }
                if ui.selectable_label(false, "Plantillas").clicked() {
                    self.pick_template();
                }
                if ui.selectable_label(false, "Ajustes").clicked() {
                    self.api_key_edit = self.cfg.api_key.clone();
                    self.provider_edit = self.cfg.provider.clone();
                    self.model_edit = self.cfg.model.clone();
                    self.output_dir_edit = self.cfg.output_dir.clone();
                    self.update_repository_edit = self.cfg.update_repository.clone();
                    self.check_updates_on_startup_edit = self.cfg.check_updates_on_startup;
                    self.dark_mode_edit = self.cfg.dark_mode;
                    self.compact_view_edit = self.cfg.compact_view;
                    self.show_settings = true;
                }
                if ui.selectable_label(false, "IA").clicked() {
                    self.api_key_edit = self.cfg.api_key.clone();
                    self.provider_edit = self.cfg.provider.clone();
                    self.model_edit = self.cfg.model.clone();
                    self.output_dir_edit = self.cfg.output_dir.clone();
                    self.dark_mode_edit = self.cfg.dark_mode;
                    self.compact_view_edit = self.cfg.compact_view;
                    self.show_settings = true;
                }

                // Tarjeta oscura "Turno de hoy" al pie de la barra.
                // (El layout bottom_up invierte el orden: el contenido de
                // la tarjeta se fuerza a top_down para que el título quede
                // arriba. La altura es fija para que la tarjeta sea
                // compacta, como en el concepto A, en vez de estirarse
                // hasta ocupar toda la barra.)
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 116.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            egui::Frame::none()
                                .fill(pal(ui).deep_bg)
                                .rounding(12.0)
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::LEFT),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new("Turno de hoy")
                                                    .font(font_semibold(13.5))
                                                    .color(pal(ui).deep_text),
                                            );
                                            ui.add_space(4.0);
                                            let hoy =
                                                chrono::Local::now().format("%d-%m-%Y").to_string();
                                            ui.label(
                                                egui::RichText::new(hoy)
                                                    .size(12.0)
                                                    .color(pal(ui).deep_sub),
                                            );
                                            ui.label(
                                                egui::RichText::new("08:00 a 08:00")
                                                    .size(12.0)
                                                    .color(pal(ui).deep_sub),
                                            );
                                            let n_inc =
                                                self.novedades.iter().filter(|n| n.include).count();
                                            ui.label(
                                                egui::RichText::new(if self.novedades.is_empty() {
                                                    "Sin cargar".to_string()
                                                } else {
                                                    format!(
                                                        "{} novedades, {} incluidas",
                                                        self.novedades.len(),
                                                        n_inc
                                                    )
                                                })
                                                .size(12.0)
                                                .color(pal(ui).deep_text),
                                            );
                                        },
                                    );
                                });
                        },
                    );
                });
            });

        // ================= Inspector derecho: Resumen (252 px) =================
        egui::SidePanel::right("resumen")
            .resizable(false)
            .default_width(252.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Resumen")
                        .font(font_semibold(16.0))
                        .color(pal(ui).text_primary),
                );
                ui.add_space(8.0);

                // Filas de estado del parte.
                let zip_name = self
                    .zip_path
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                let tpl_name = self
                    .template_path
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                let ia_ok = !self.cfg.api_key.trim().is_empty();
                let rows: [(&str, String); 5] = [
                    (
                        "Archivo",
                        zip_name.unwrap_or_else(|| "Sin cargar".to_string()),
                    ),
                    (
                        "Plantilla",
                        tpl_name.unwrap_or_else(|| "Sin plantilla".to_string()),
                    ),
                    (
                        "IA Conectada",
                        if ia_ok {
                            "Conectada".to_string()
                        } else {
                            "Sin API key".to_string()
                        },
                    ),
                    ("Modelo", short_model(&self.cfg.model)),
                    (
                        "Salida .docx",
                        if self.cfg.output_dir.trim().is_empty() {
                            "Junto al .zip".to_string()
                        } else {
                            self.cfg.output_dir.clone()
                        },
                    ),
                ];
                for (k, v) in rows {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(k).size(12.0).color(pal(ui).text_muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(v)
                                    .size(12.0)
                                    .strong()
                                    .color(pal(ui).text_primary),
                            );
                        });
                    });
                    ui.add_space(2.0);
                }
                ui.separator();

                // Rango de fechas del parte.
                overline(ui, "Rango (08:00 a 08:00)");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.date_from)
                            .hint_text("10-10-2026")
                            .desired_width(96.0),
                    );
                    ui.label(egui::RichText::new("a").color(pal(ui).text_secondary));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.date_to)
                            .hint_text("11-10-2026")
                            .desired_width(96.0),
                    );
                });
                ui.add_space(8.0);
                ui.separator();

                // Mini vista previa del documento (clic → vista completa).
                let n_inc = self.novedades.iter().filter(|n| n.include).count();
                let n_fotos: usize = self
                    .novedades
                    .iter()
                    .filter(|n| n.include)
                    .map(|n| n.photos.len())
                    .sum();
                let mini = egui::Frame::none()
                    .fill(pal(ui).bg_card)
                    .stroke(egui::Stroke::new(1.0_f32, pal(ui).border_subtle))
                    .rounding(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("PARTE DIARIO")
                                    .strong()
                                    .size(12.0)
                                    .color(pal(ui).accent),
                            );
                            ui.label(
                                egui::RichText::new("Anti-Ruidos y Contaminación Sónica, P.N.")
                                    .size(11.0)
                                    .color(pal(ui).text_muted),
                            );
                            ui.add_space(4.0);
                            let fecha_doc = parse_fecha(&self.date_from)
                                .map(|d| d.format("%d-%m-%Y").to_string());
                            ui.label(
                                egui::RichText::new(match fecha_doc {
                                    Some(f) => format!("Santo Domingo, D.N., {f}"),
                                    None => "Configure el rango de fechas.".to_string(),
                                })
                                .size(11.0)
                                .color(pal(ui).text_secondary),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!("{n_inc} novedades, {n_fotos} fotos"))
                                    .size(11.0)
                                    .color(pal(ui).text_muted),
                            );
                            if self.novedades.is_empty() {
                                ui.label(
                                    egui::RichText::new("Aún no hay novedades cargadas.")
                                        .size(11.0)
                                        .color(pal(ui).text_muted),
                                );
                            }
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if mini.clicked() {
                    self.view = ViewMode::Preview;
                    self.preview_seen = true;
                }
                mini.on_hover_text("Clic para ver la vista previa completa");
                ui.add_space(8.0);

                // Selector Editar | Vista previa.
                ui.horizontal(|ui| {
                    if seg_button(ui, "Editar", self.view == ViewMode::Edit).clicked() {
                        self.view = ViewMode::Edit;
                    }
                    if seg_button(ui, "Vista previa", self.view == ViewMode::Preview).clicked() {
                        self.view = ViewMode::Preview;
                        self.preview_seen = true;
                    }
                });
                ui.add_space(8.0);

                // Avisos (colapsados por defecto).
                let (nw, ni) = (self.warnings.len(), self.info.len());
                if nw + ni > 0 {
                    ui.horizontal(|ui| {
                        let txt = if self.warnings.is_empty() {
                            format!("ℹ {} nota{}", ni, if ni == 1 { "" } else { "s" })
                        } else {
                            format!("⚠ {} aviso{}", nw, if nw == 1 { "" } else { "s" })
                        };
                        if btn_tertiary(ui, &txt).clicked() {
                            self.alerts_expanded = !self.alerts_expanded;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if btn_tertiary(ui, "Descartar").clicked() {
                                self.warnings.clear();
                                self.info.clear();
                                self.alerts_expanded = false;
                            }
                        });
                    });
                    if self.alerts_expanded {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for w in &self.warnings {
                                    banner(
                                        ui,
                                        &format!("⚠ {w}"),
                                        pal(ui).warn_tint,
                                        pal(ui).warn_line,
                                    );
                                }
                                for i in &self.info {
                                    banner(
                                        ui,
                                        &format!("ℹ {i}"),
                                        pal(ui).info_tint,
                                        pal(ui).accent_line,
                                    );
                                }
                            });
                    }
                    ui.add_space(8.0);
                }

                // Acción final al pie del inspector. En un layout bottom_up
                // lo primero que se añade queda abajo: Generar va primero
                // (abajo del todo), luego Abrir y luego el aviso de éxito.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let missing: Vec<&str> = [
                        (self.template_path.is_none(), "cargar la plantilla .docx"),
                        (self.zip_path.is_none(), "cargar el .zip del chat"),
                        (
                            parse_fecha(&self.date_from).is_none()
                                || parse_fecha(&self.date_to).is_none(),
                            "escribir un rango de fechas válido (DD-MM-AAAA)",
                        ),
                    ]
                    .iter()
                    .filter_map(|(bad, msg)| if *bad { Some(*msg) } else { None })
                    .collect();
                    let resp = btn_gold(ui, "Generar Parte Diario");
                    let resp = if missing.is_empty() {
                        resp
                    } else {
                        resp.on_hover_text(format!("Antes debe: {}.", missing.join("; ")))
                    };
                    if resp.clicked() {
                        self.generate();
                    }
                    if let Some(gp) = self.generated_path.clone() {
                        ui.add_space(6.0);
                        if btn_secondary(ui, "Abrir .docx").clicked() {
                            match open::that(&gp) {
                                Ok(()) => self.exported = true,
                                Err(e) => self.log(format!("No se pudo abrir el archivo: {e}")),
                            }
                        }
                    }
                    if let Some(msg) = self.success_msg.clone() {
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(pal(ui).success_tint)
                            .stroke(egui::Stroke::new(1.0_f32, pal(ui).success_line))
                            .rounding(8.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    check_badge(ui, pal(ui).success);
                                    ui.label(
                                        egui::RichText::new(msg)
                                            .size(12.0)
                                            .color(pal(ui).text_primary),
                                    );
                                });
                            });
                    }
                });
            });

        // ================= Central: stepper + contenido =================
        egui::CentralPanel::default().show(ctx, |ui| {
            let steps = self.step_states();
            if let Some(idx) = stepper(ui, &steps) {
                self.goto_step(idx);
            }
            ui.add_space(8.0);

            if self.novedades.is_empty() {
                if self.view == ViewMode::Preview {
                    self.show_preview(ui, ctx);
                } else {
                    // Estado vacío: cargar el chat de esta noche.
                    // Bloque compacto bajado con un espacio superior fijo:
                    // centered_and_justified no sirve aquí (reparte el
                    // espacio entre los hijos y empuja los botones fuera
                    // del área visible).
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.label(
                            egui::RichText::new("Carga el chat de esta noche")
                                .font(font_semibold(22.0))
                                .color(pal(ui).text_primary),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Exporta el chat de WhatsApp como .zip y cárgalo aquí para armar el Parte Diario.",
                            )
                            .color(pal(ui).text_secondary),
                        );
                        ui.add_space(24.0);
                    });
                    // Zona de arrastre punteada.
                    let zone_w = ui.available_width().min(520.0);
                    let zone_h = 120.0;
                    ui.horizontal(|ui| {
                        let avail = ui.available_width();
                        ui.add_space(((avail - zone_w) / 2.0).max(0.0));
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(zone_w, zone_h),
                            egui::Sense::hover(),
                        );
                        dashed_rect(ui.painter(), rect, pal(ui).accent_line);
                        ui.painter().text(
                            rect.center() - egui::vec2(0.0, 10.0),
                            egui::Align2::CENTER_CENTER,
                            "Arrastra tu archivo .zip aquí",
                            egui::FontId::proportional(14.0),
                            pal(ui).text_secondary,
                        );
                        ui.painter().text(
                            rect.center() + egui::vec2(0.0, 12.0),
                            egui::Align2::CENTER_CENTER,
                            "o suéltalo sobre esta ventana",
                            egui::FontId::proportional(12.0),
                            pal(ui).text_muted,
                        );
                    });
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        ui.horizontal(|ui| {
                            if btn_action(ui, "Cargar .zip de WhatsApp").clicked() {
                                self.pick_zip();
                            }
                            if btn_secondary(ui, "Ver ejemplo de exportación").clicked() {
                                self.view = ViewMode::Preview;
                                self.preview_seen = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if btn_tertiary(ui, "Cargar plantilla .docx").clicked() {
                            self.pick_template();
                        }
                    });
                }
            } else if self.view == ViewMode::Preview {
                self.show_preview(ui, ctx);
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Novedades: búsqueda y acciones masivas ---
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Novedades ({})", self.novedades.len()))
                                .font(font_semibold(16.0))
                                .color(pal(ui).text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if btn_tertiary(ui, "Excluir todas").clicked() {
                                for n in &mut self.novedades {
                                    n.include = false;
                                }
                            }
                            if btn_tertiary(ui, "Incluir todas").clicked() {
                                for n in &mut self.novedades {
                                    n.include = true;
                                }
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let search_resp = ui.add(
                            egui::TextEdit::singleline(&mut self.filter)
                                .id_salt("search_field")
                                .hint_text("Buscar en texto original, formal o #número…")
                                .desired_width(f32::INFINITY),
                        );
                        // Guardar el id para enfocarlo con Ctrl+F.
                        self.search_field_id = Some(search_resp.id);
                        if !self.filter.is_empty() && btn_tertiary(ui, "×").clicked() {
                            self.filter.clear();
                        }
                    });
                    ui.add_space(6.0);
                    let bw = (ui.available_width() - 8.0) / 2.0;
                    // Mientras corre una formalización masiva, el botón se
                    // deshabilita (visualmente) y el progreso se muestra
                    // junto a él, no solo en la barra lateral.
                    let llm_running = !self.llm_busy.is_empty();
                    let llm_done = self.llm_batch_total.saturating_sub(self.llm_busy.len());
                    ui.horizontal(|ui| {
                        let formalize_label = if llm_running {
                            format!("Formalizando… {llm_done} de {}", self.llm_batch_total)
                        } else {
                            "Formalizar todas con IA".to_string()
                        };
                        if ui
                            .add_enabled(
                                !llm_running,
                                egui::Button::new(
                                    egui::RichText::new(formalize_label)
                                        .strong()
                                        .color(pal(ui).accent_hover),
                                )
                                .fill(pal(ui).accent_tint)
                                .stroke(egui::Stroke::new(1.0_f32, pal(ui).accent_line))
                                .rounding(10.0)
                                .min_size(egui::vec2(bw, 0.0)),
                            )
                            .clicked()
                        {
                            if self.cfg.api_key.trim().is_empty() {
                                self.log("Sin API key: configure su key de OpenRouter (gratis) en Ajustes, o use la limpieza sin conexión.".to_string());
                                self.api_key_edit = self.cfg.api_key.clone();
                                self.provider_edit = self.cfg.provider.clone();
                                self.model_edit = self.cfg.model.clone();
                                self.output_dir_edit = self.cfg.output_dir.clone();
                                self.dark_mode_edit = self.cfg.dark_mode;
                                self.compact_view_edit = self.cfg.compact_view;
                                self.show_settings = true;
                            } else {
                                let ids: Vec<usize> = self
                                    .novedades
                                    .iter()
                                    .filter(|n| n.include && !n.body_raw.trim().is_empty())
                                    .map(|n| n.id)
                                    .collect();
                                if !ids.is_empty() {
                                    // Punto de deshacer antes de reescribir.
                                    let prev = self.snapshot_formal();
                                    self.undo_formal =
                                        Some(("Formalizar todas con IA".to_string(), prev));
                                    self.spawn_formalize_many(ids);
                                }
                            }
                        }
                        if ui
                            .add_sized(
                                [bw, 0.0],
                                egui::Button::new(
                                    egui::RichText::new("Limpieza a todas (sin conexión)")
                                        .color(pal(ui).text_primary),
                                )
                                .fill(pal(ui).bg_hover)
                                .stroke(egui::Stroke::new(1.0_f32, pal(ui).border_subtle))
                                .rounding(8.0),
                            )
                            .clicked()
                        {
                            // Punto de deshacer antes de reescribir.
                            let prev = self.snapshot_formal();
                            let mut k = 0;
                            for n in self
                                .novedades
                                .iter_mut()
                                .filter(|n| n.include && !n.body_raw.trim().is_empty())
                            {
                                n.body_formal = llm::cleanup_offline(&n.body_raw);
                                k += 1;
                            }
                            if k > 0 {
                                self.undo_formal =
                                    Some(("Limpieza a todas".to_string(), prev));
                            }
                            self.log(format!(
                                "Limpieza sin conexión aplicada a {k} novedades."
                            ));
                        }
                    });
                    // Deshacer de la última acción masiva: aparece cuando
                    // hay punto de deshacer y ninguna formalización corre
                    // (un resultado tardío no debe pisar lo restaurado).
                    if !llm_running {
                        if let Some((label, _)) = self.undo_formal.clone() {
                            ui.horizontal(|ui| {
                                if btn_tertiary(ui, &format!("Deshacer «{label}»")).clicked() {
                                    self.undo_bulk_formalize();
                                }
                                ui.label(
                                    egui::RichText::new("Restaura los textos formales anteriores.")
                                        .size(12.0)
                                        .color(pal(ui).text_muted),
                                );
                            });
                            ui.add_space(4.0);
                        }
                    }
                    // Progreso en línea, junto a la acción que lo disparó:
                    // spinner + "X de Y" mientras la formalización masiva
                    // corre (el aviso de la barra lateral se mantiene).
                    if llm_running {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new(format!(
                                    "Formalizando con IA… {llm_done} de {} novedades. Puede seguir revisando mientras tanto.",
                                    self.llm_batch_total
                                ))
                                .size(12.0)
                                .color(pal(ui).accent),
                            );
                        });
                        ui.add_space(4.0);
                    }
                    ui.add_space(8.0);

                    // --- Lista de novedades ---
                    let visible = self.visible_novedades();
                    if visible.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                egui::RichText::new("Sin coincidencias").color(pal(ui).text_secondary),
                            );
                        });
                    } else {
                    for i in visible {
                        let (sel, snippet, fecha, nphotos, is_orphan, include, formalizado, busy) = {
                            let n = &self.novedades[i];
                            (
                                self.selected == i,
                                Self::novedad_snippet(n),
                                n.fecha_mencionada.map(|d| d.format("%d/%m").to_string()),
                                n.photos.len(),
                                n.is_orphan,
                                n.include,
                                !n.body_formal.trim().is_empty(),
                                self.llm_busy.contains(&n.id),
                            )
                        };
                        let resp = egui::Frame::none()
                            .fill(if sel { pal(ui).accent_tint } else { pal(ui).bg_card })
                            .stroke(if sel {
                                egui::Stroke::new(1.5_f32, pal(ui).accent_line)
                            } else {
                                egui::Stroke::new(1.0_f32, pal(ui).border_subtle)
                            })
                            .rounding(10.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let mut inc = include;
                                    if ui.checkbox(&mut inc, "").changed() && inc != include {
                                        self.novedades[i].include = inc;
                                    }
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "#{} {}",
                                                self.novedades[i].id, snippet
                                            ))
                                            .strong()
                                            .size(13.0)
                                            .color(pal(ui).text_primary),
                                        );
                                        ui.add_space(3.0);
                                        ui.horizontal(|ui| {
                                            if let Some(f) = fecha {
                                                pill(ui, &f);
                                            }
                                            pill(ui, &format!("📷 {nphotos}"));
                                            if is_orphan {
                                                pill_tint(ui, "huérfana", pal(ui).warn_tint, pal(ui).warning);
                                            }
                                            if !include {
                                                pill(ui, "excluida");
                                            }
                                            if busy {
                                                pill_tint(ui, "formalizando…", pal(ui).accent_tint, pal(ui).accent);
                                            } else if formalizado {
                                                pill_tint(ui, "Formalizado", pal(ui).success_tint, pal(ui).success);
                                            }
                                        });
                                    });
                                });
                            })
                            .response;
                        // Seleccionar al hacer clic en cualquier parte de la
                        // tarjeta (incluido su checkbox). NO usar aquí
                        // .interact(Sense::click()): registraría una interacción
                        // sobre TODO el rectángulo DESPUÉS del checkbox y le
                        // robaría los clics (el checkbox dejaba de funcionar).
                        if resp.contains_pointer()
                            && ui.input(|i| i.pointer.primary_clicked())
                        {
                            self.selected = i;
                        }
                        ui.add_space(6.0);
                    }
                    }
                    ui.add_space(8.0);

                    // --- Editor de la novedad seleccionada ---
                    let idx = self.selected;
                    // --- encabezado de la novedad + navegación ---
                    card(ui, |ui| {
                        ui.horizontal(|ui| {
                            if btn_secondary(ui, "◀").clicked() {
                                self.selected = self.selected.saturating_sub(1);
                            }
                            if btn_secondary(ui, "▶").clicked()
                                && self.selected + 1 < self.novedades.len()
                            {
                                self.selected += 1;
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "Novedad #{}",
                                    self.novedades[idx].id
                                ))
                                .font(font_semibold(16.0))
                                .color(pal(ui).text_primary),
                            );
                            let mut inc = self.novedades[idx].include;
                            if ui
                                .checkbox(&mut inc, egui::RichText::new("Incluir en el reporte"))
                                .changed()
                            {
                                self.novedades[idx].include = inc;
                            }
                            let (fecha_txt, sender) = {
                                let n = &self.novedades[idx];
                                (
                                    n.fecha_mencionada
                                        .map(|d| d.format("%d-%m-%Y").to_string())
                                        .unwrap_or_else(|| "sin fecha".to_string()),
                                    n.sender.clone(),
                                )
                            };
                            if self.novedades[idx].is_orphan {
                                pill_tint(ui, "📷 Fotos huérfanas", pal(ui).warn_tint, pal(ui).warning);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if btn_tertiary(ui, "Siguiente pendiente").clicked() {
                                        self.next_pending();
                                    }
                                    ui.small(
                                        egui::RichText::new(format!("{sender}, {fecha_txt}"))
                                            .color(pal(ui).text_secondary),
                                    );
                                },
                            );
                        });
                    });
                    ui.add_space(8.0);

                    // --- encabezados ---
                    // Sección lisa (sin tarjeta): el trabajo de texto no
                    // necesita un marco; el título con su regla basta.
                    ui.vertical(|ui| {
                        section_title(ui, "Encabezados institucionales");
                        let headers_text = self.novedades[idx].headers.join("\n");
                        let mut tmp = headers_text.clone();
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut tmp)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(2),
                            )
                            .changed()
                        {
                            self.novedades[idx].headers = tmp
                                .lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                        }
                    });
                    ui.add_space(8.0);

                    // --- texto crudo ---
                    ui.vertical(|ui| {
                        section_title(ui, "Texto original (así lo escribió el agente)");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.novedades[idx].body_raw)
                                .desired_width(f32::INFINITY)
                                .desired_rows(5),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let busy = self.llm_busy.contains(&self.novedades[idx].id);
                            let id = self.novedades[idx].id;
                            // Botones deshabilitados (visualmente) mientras
                            // esta novedad se formaliza: antes parecían
                            // activos y el clic no hacía nada.
                            let r1 = btn_primary_enabled(ui, "Formalizar con IA", !busy);
                            if r1.clicked() && !busy {
                                if self.cfg.api_key.trim().is_empty() {
                                    self.log("Sin API key: configure su key en Ajustes o use la limpieza sin conexión.".to_string());
                                    self.api_key_edit = self.cfg.api_key.clone();
                                    self.provider_edit = self.cfg.provider.clone();
                                    self.model_edit = self.cfg.model.clone();
                                    self.output_dir_edit = self.cfg.output_dir.clone();
                                    self.dark_mode_edit = self.cfg.dark_mode;
                                    self.compact_view_edit = self.cfg.compact_view;
                                    self.show_settings = true;
                                } else {
                                    self.spawn_formalize(vec![id], true);
                                }
                            }
                            let r2 = btn_secondary_enabled(ui, "Limpieza sin conexión", !busy);
                            if r2.clicked() && !busy {
                                self.spawn_formalize(vec![id], false);
                            }
                            if busy {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new("formalizando…").color(pal(ui).text_secondary),
                                );
                            }
                        });
                    });
                    ui.add_space(8.0);

                    // --- texto formal ---
                    ui.vertical(|ui| {
                        section_title(ui, "Texto formal (revise y edite antes de generar)");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.novedades[idx].body_formal)
                                .desired_width(f32::INFINITY)
                                .desired_rows(5),
                        );
                    });
                    ui.add_space(8.0);

                    // --- fotos ---
                    let (photos, videos) = {
                        let n = &self.novedades[idx];
                        (n.photos.clone(), n.videos_skipped.clone())
                    };
                    card(ui, |ui| {
                        ui.horizontal(|ui| {
                            section_title(ui, &format!("Fotos ({})", photos.len()));
                        });
                        if !videos.is_empty() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "🎬 {} video(s) omitidos del documento: {}",
                                    videos.len(),
                                    videos.join(", ")
                                ))
                                .size(12.0)
                                .color(pal(ui).text_secondary),
                            );
                        }
                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for ph in &photos {
                                    let key = format!("thumb-{ph}");
                                    let exists = self.find_media(ph).is_some();
                                    let tex = if let Some(t) = self.textures.get(&key).cloned() {
                                        Some(t)
                                    } else if exists {
                                        let bytes = self.find_media(ph).cloned();
                                        bytes.and_then(|b| self.thumb(ctx, &key, &b))
                                    } else {
                                        None
                                    };
                                    ui.vertical(|ui| {
                                        egui::Frame::none()
                                            .fill(pal(ui).bg_hover)
                                            .rounding(8.0)
                                            .inner_margin(6.0)
                                            .show(ui, |ui| {
                                                match tex {
                                                    Some(tex) => {
                                                        ui.add(
                                                            egui::Image::new(&tex)
                                                                .max_height(150.0)
                                                                .rounding(6.0),
                                                        );
                                                    }
                                                    None if exists => {
                                                        ui.label("🖼");
                                                    }
                                                    None => {
                                                        ui.label("🖼 (no encontrada)");
                                                    }
                                                }
                                            });
                                        ui.small(
                                            egui::RichText::new(ph.clone())
                                                .size(10.0)
                                                .color(pal(ui).text_secondary),
                                        );
                                    });
                                }
                            });
                        });
                    });

                    // --- reasignación de huérfanas ---
                    let is_orphan = self.novedades[idx].is_orphan;
                    if is_orphan {
                        ui.add_space(8.0);
                        card(ui, |ui| {
                            section_title(ui, "Reasignar fotos huérfanas");
                            ui.label(
                                egui::RichText::new(
                                    "Estas fotos no tenían texto asociado (posible mensaje borrado). Elija arriba la novedad de destino y muévalas.",
                                )
                                .size(12.0)
                                .color(pal(ui).text_secondary),
                            );
                            ui.add_space(4.0);
                            let targets: Vec<(usize, String)> = self
                                .novedades
                                .iter()
                                .filter(|n| !n.is_orphan)
                                .map(|n| (n.id, Self::novedad_title(n)))
                                .collect();
                            let orphan_id = self.novedades[idx].id;
                            let sel_val: usize = {
                                let e = self.orphan_target.entry(orphan_id).or_insert(0);
                                if *e == 0 {
                                    if let Some((first, _)) = targets.first() {
                                        *e = *first;
                                    }
                                }
                                *e
                            };
                            if targets.is_empty() {
                                ui.label(
                                    egui::RichText::new(
                                        "No hay ninguna novedad de destino. Cree o incluya una novedad primero.",
                                    )
                                    .size(12.0)
                                    .color(pal(ui).text_secondary),
                                );
                            } else {
                                ui.horizontal(|ui| {
                                    let mut sel_local = sel_val;
                                    egui::ComboBox::from_id_salt(format!("orphan{idx}"))
                                        .selected_text(
                                            targets
                                                .iter()
                                                .find(|(id, _)| *id == sel_local)
                                                .map(|(_, t)| t.clone())
                                                .unwrap_or_else(|| "(elegir)".to_string()),
                                        )
                                        .show_ui(ui, |ui| {
                                            for (id, t) in &targets {
                                                ui.selectable_value(&mut sel_local, *id, t);
                                            }
                                        });
                                    self.orphan_target.insert(orphan_id, sel_local);
                                    if btn_primary(ui, "Mover fotos a esa novedad").clicked() {
                                        let target_id = sel_local;
                                        if let Some(t_idx) = self
                                            .novedades
                                            .iter()
                                            .position(|n| n.id == target_id && !n.is_orphan)
                                        {
                                            let photos_mv = std::mem::take(
                                                &mut self.novedades[idx].photos,
                                            );
                                            let videos_mv = std::mem::take(
                                                &mut self.novedades[idx].videos_skipped,
                                            );
                                            let k = photos_mv.len();
                                            self.novedades[t_idx].photos.extend(photos_mv);
                                            self.novedades[t_idx]
                                                .videos_skipped
                                                .extend(videos_mv);
                                            self.novedades[idx].include = false;
                                            self.log(format!(
                                                "{k} foto(s) huérfanas movidas a la novedad #{target_id}."
                                            ));
                                        } else {
                                            self.log(
                                                "No se movieron las fotos: la novedad destino ya no existe."
                                                    .to_string(),
                                            );
                                        }
                                    }
                                });
                            }
                        });
                    }
                });
            }
        });

        // ================= Ajustes =================
        if self.show_settings {
            let mut keep_open = true;
            egui::Window::new("Ajustes")
                .id(egui::Id::new("settings_win"))
                .collapsible(false)
                .resizable(false)
                .default_width(430.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        overline(ui, "Inteligencia artificial");
                        section_title(ui, "Proveedor de IA");
                        let prev_provider = self.provider_edit.clone();
                        egui::ComboBox::from_id_salt("provider")
                            .selected_text(llm::LlmProvider::from_config(&self.provider_edit).label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.provider_edit,
                                    llm::LlmProvider::OpenRouter.as_str().to_string(),
                                    llm::LlmProvider::OpenRouter.label(),
                                );
                                ui.selectable_value(
                                    &mut self.provider_edit,
                                    llm::LlmProvider::Anthropic.as_str().to_string(),
                                    llm::LlmProvider::Anthropic.label(),
                                );
                                ui.selectable_value(
                                    &mut self.provider_edit,
                                    llm::LlmProvider::FreeLLMApi.as_str().to_string(),
                                    llm::LlmProvider::FreeLLMApi.label(),
                                );
                            });
                        if self.provider_edit != prev_provider {
                            self.model_edit = llm::LlmProvider::from_config(&self.provider_edit)
                                .default_model()
                                .to_string();
                            // Forzar un repaint para que el hint_text se actualice si el modelo por defecto cambia.
                            ui.ctx().request_repaint();
                        }
                        ui.add_space(4.0);
                        let current_provider = llm::LlmProvider::from_config(&self.provider_edit);
                        let is_openrouter = current_provider == llm::LlmProvider::OpenRouter;
                        let is_freellmapi = current_provider == llm::LlmProvider::FreeLLMApi;

                        if is_openrouter {
                            ui.label(egui::RichText::new("API key de OpenRouter (gratis)").strong());
                            ui.small("Consígala sin tarjeta en https://openrouter.ai/keys y péguela aquí.");
                        } else if is_freellmapi {
                            ui.label(egui::RichText::new("API key de FreeLLMApi (Local)").strong());
                            ui.small("Clave unificada de su servicio local. Debe estar corriendo en http://127.0.0.1:31415.");
                            ui.small("Modelo recomendado: auto. La app desactiva el razonamiento para que no se filtre al informe.");
                        } else {
                            ui.label(egui::RichText::new("API key de Anthropic (Claude, de pago)").strong());
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.api_key_edit)
                                .password(true)
                                .desired_width(f32::INFINITY),
                        );
                        ui.small("Se guarda localmente en su equipo. No se comparte con nadie.");
                        ui.checkbox(
                            &mut self.anonymize_edit,
                            "Anonimizar cédulas, teléfonos y correos antes de enviar a la IA",
                        );
                        ui.small("Los datos reales se restauran en el documento final.");
                        ui.add_space(4.0);
                        section_title(ui, "Modelo");
                        ui.text_edit_singleline(&mut self.model_edit);
                        if is_openrouter {
                            ui.small("Modelos gratis que puede probar (terminan en :free):");
                            ui.small("• google/gemma-4-31b-it:free (recomendado, sin razonamiento)");
                            ui.small("• google/gemma-4-26b-a4b-it:free");
                            ui.small("• nvidia/nemotron-3.5-lightning:free");
                            ui.small("Límites del plan gratuito: ~20 peticiones/min, ~50/día.");
                        } else if is_freellmapi {
                            ui.small("Modelos locales: auto (recomendado), gemma-4-31b-it, gemini-3.5-flash.");
                            ui.small("Si un modelo falla, pruebe auto: el enrutador elige uno disponible.");
                        }
                        ui.add_space(8.0);
                        overline(ui, "Destino del reporte");
                        section_title(ui, "Carpeta de salida");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.output_dir_edit);
                            if btn_secondary(ui, "Examinar…").clicked() {
                                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                    self.output_dir_edit = p.to_string_lossy().to_string();
                                }
                            }
                        });
                        ui.small("Vacía = junto al .zip de WhatsApp.");
                        ui.add_space(10.0);
                        overline(ui, "Actualizaciones");
                        section_title(ui, "Repositorio de GitHub");
                        ui.text_edit_singleline(&mut self.update_repository_edit);
                        ui.small("Formato: usuario/repositorio. Ejemplo: elpirinpi/partediario.");
                        if self.update_repository_edit.trim().is_empty() {
                            ui.small(
                                egui::RichText::new(
                                    "Sin repositorio configurado: la búsqueda automática al iniciar está desactivada.",
                                )
                                .color(pal(ui).text_muted),
                            );
                        }
                        ui.checkbox(
                            &mut self.check_updates_on_startup_edit,
                            "Buscar actualizaciones al iniciar",
                        );
                        ui.horizontal(|ui| {
                            if btn_secondary(ui, "Buscar actualización").clicked() {
                                self.start_update_check(ui.ctx());
                            }
                            if self.available_update.is_some()
                                && btn_primary(ui, "Instalar y reiniciar").clicked()
                            {
                                self.start_install_update(ui.ctx());
                            }
                            if btn_tertiary(ui, "Restaurar respaldo").clicked() {
                                self.start_rollback(ui.ctx());
                            }
                        });
                        if !self.update_message.is_empty() {
                            ui.small(self.update_message.clone());
                        }
                        if self.update_busy {
                            ui.spinner();
                        }
                        ui.add_space(10.0);
                        overline(ui, "Apariencia");
                        section_title(ui, "Tema y densidad");
                        ui.checkbox(&mut self.dark_mode_edit, "Modo oscuro");
                        ui.small("Para trabajar de noche: fondo oscuro, mismo azul institucional.");
                        ui.checkbox(
                            &mut self.compact_view_edit,
                            "Vista compacta (menos espacio, más novedades en pantalla)",
                        );
                        ui.small("Atajos: Ctrl+O cargar .zip · Ctrl+G generar · Ctrl+F buscar.");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if btn_primary(ui, "Guardar").clicked() {
                            self.cfg.api_key = self.api_key_edit.clone();
                            self.cfg.provider = self.provider_edit.clone();
                            self.cfg.model = self.model_edit.clone();
                            self.cfg.output_dir = self.output_dir_edit.clone();
                            self.cfg.anonymize = self.anonymize_edit;
                            self.cfg.update_repository = self.update_repository_edit.clone();
                            self.cfg.check_updates_on_startup = self.check_updates_on_startup_edit;
                            // Apariencia: aplicar de inmediato (tema y
                            // densidad) y persistir en la configuración.
                            let theme_changed = self.cfg.dark_mode != self.dark_mode_edit
                                || self.cfg.compact_view != self.compact_view_edit;
                            self.cfg.dark_mode = self.dark_mode_edit;
                            self.cfg.compact_view = self.compact_view_edit;
                            if theme_changed {
                                apply_theme(
                                    ui.ctx(),
                                    self.cfg.dark_mode,
                                    self.cfg.compact_view,
                                );
                            }
                            match history::save_config(&self.cfg) {
                                Ok(()) => self.log("Ajustes guardados.".to_string()),
                                Err(e) => {
                                    self.log(format!("No se pudieron guardar los ajustes: {e}"));
                                }
                            }
                            keep_open = false;
                        }
                        if btn_tertiary(ui, "Cancelar").clicked() {
                            keep_open = false;
                        }
                    });
                });
            self.show_settings = keep_open;
        }

        // ================= Registro =================
        if self.show_log {
            let mut open = true;
            egui::Window::new("Registro de actividad")
                .id(egui::Id::new("log_win"))
                .open(&mut open)
                .default_width(580.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} línea(s)", self.log.len()))
                                .size(12.0)
                                .color(pal(ui).text_secondary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if btn_tertiary(ui, "Limpiar registro").clicked() {
                                self.log.clear();
                            }
                        });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for l in &self.log {
                                ui.monospace(l);
                            }
                        });
                });
            self.show_log = open;
        }

        // ================= Historial de partes generados =================
        if self.show_history {
            let mut open = true;
            egui::Window::new("Historial de partes")
                .id(egui::Id::new("history_win"))
                .open(&mut open)
                .default_width(560.0)
                .show(ctx, |ui| {
                    if self.hist.entries.is_empty() {
                        ui.label(
                            egui::RichText::new("Aún no hay partes generados.")
                                .color(pal(ui).text_secondary),
                        );
                    } else {
                        let mut entries = self.hist.entries.clone();
                        entries.reverse();
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for e in entries {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{} a {}",
                                                    e.report_start, e.report_end
                                                ))
                                                .strong()
                                                .color(pal(ui).text_primary),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}, {} novedades, {}",
                                                    e.generated_at, e.novedades, e.zip_name
                                                ))
                                                .size(11.5)
                                                .color(pal(ui).text_secondary),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let p = PathBuf::from(&e.output_path);
                                                if p.exists()
                                                    && btn_secondary(ui, "Abrir").clicked()
                                                {
                                                    if let Err(err) = open::that(&p) {
                                                        self.log(format!(
                                                            "No se pudo abrir el archivo: {err}"
                                                        ));
                                                    }
                                                }
                                            },
                                        );
                                    });
                                    ui.separator();
                                }
                            });
                    }
                });
            self.show_history = open;
        }
        if !self.pending_llm.is_empty() {
            ctx.request_repaint();
        }
    }
}
