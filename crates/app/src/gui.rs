//! Interfaz de escritorio (egui/eframe) — dark mode profesional.
//!
//! Flujo guiado (todo visible en pantalla, sin manual):
//! 1. "Cargar .zip de WhatsApp" (botón o arrastrar el archivo a la ventana).
//! 2. "Cargar plantilla .docx" con membrete institucional.
//! 3. Revisar/editar novedades en el panel lateral (tarjetas con insignias).
//! 4. Formalizar con IA o con limpieza sin conexión.
//! 5. Verificar rango de fechas (autocompletado).
//! 6. "Generar Parte Diario" → .docx validado + botón para abrirlo.
//!
//! Principios de diseño (Material Design dark + GitHub/Google dark):
//! - Nada de negro/blanco puro; elevación por aclarar superficies.
//! - Acento desaturado claro; texto en 3 niveles de énfasis.
//! - UNA acción primaria por región; insignias semánticas suavizadas.

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
    textures: HashMap<String, egui::TextureHandle>,
    show_settings: bool,
    api_key_edit: String,
    provider_edit: String,
    model_edit: String,
    output_dir_edit: String,
    anonymize_edit: bool,
    update_repository_edit: String,
    check_updates_on_startup_edit: bool,
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
    brand_icon: Option<egui::TextureHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Forzar SIEMPRE tema oscuro: egui 0.29 sigue el tema del sistema por
        // defecto (Windows en modo claro ⇒ Visuals::light ⇒ paneles blancos
        // mezclados con los widgets oscuros). Se aplica a AMBOS estilos para
        // que ningún cambio de tema del OS pueda romper la interfaz.
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let visuals = app_visuals();
        cc.egui_ctx
            .set_visuals_of(egui::Theme::Dark, visuals.clone());
        cc.egui_ctx.set_visuals_of(egui::Theme::Light, visuals);
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(14.0, 10.0);
            style.spacing.indent = 16.0;
            style.spacing.interact_size = egui::vec2(32.0, 28.0); // botones grandes, fátiga cero
        });
        let cfg = history::load_config();
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
            brand_icon: Self::load_brand_icon(&cc.egui_ctx),
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

    /// Logo real de la app (assets/icono-parte-diario.png) embebido en el
    /// binario, reescalado a 64px para la marca de la barra superior.
    fn load_brand_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let bytes = include_bytes!("../../../assets/icono-parte-diario.png");
        let img = image::load_from_memory(bytes).ok()?.resize_exact(
            64,
            64,
            image::imageops::FilterType::Lanczos3,
        );
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        let pixels = rgba
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let ci = egui::ColorImage {
            size: [w, h],
            pixels,
        };
        Some(ctx.load_texture("brand-icon", ci, egui::TextureOptions::LINEAR))
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
                "¡ATENCIÓN! Este .zip (o su contenido de texto) YA fue procesado el {} y generó el reporte {} → {} ({} novedades). \
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
                    "GENERAR: ya existe un reporte para {} → {} ({}). Si el contenido anterior era incorrecto, pulse GENERAR otra vez para REEMPLAZARLO (se sobrescribirá: {}).",
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
                    "✓ Parte Diario generado: {} ({} novedades, {} fotos incrustadas). Validación OOXML interna superada.",
                    out_path.display(),
                    res.novedades_incluidas,
                    res.images_embedded
                ));
                self.generated_path = Some(out_path.clone());
                self.success_msg = Some(format!(
                    "Parte Diario generado: {} novedades y {} fotos incrustadas.",
                    res.novedades_incluidas, res.images_embedded
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
            .map(|d| format!(" · {}", d.format("%d/%m")))
            .unwrap_or_default();
        format!("{}{} · 📷{}", base, fecha, n.photos.len())
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

    /// Lista de índices visibles según el filtro de búsqueda.
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
                    || n.headers.join(" ").to_lowercase().contains(&q)
            })
            .collect()
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

// ==================== Sistema de diseño (dark theme) ====================
// Principios (Material Design dark + GitHub/Google dark):
// - Nada de negro puro ni blanco puro: grises oscuros y off-white.
// - La elevación se comunica ACLARANDO superficies, no con sombras.
// - Acento desaturado y claro (#8AB4F8): los saturados "vibran" en dark.
// - Texto en 3 niveles de énfasis (87% / 60% / 40%), nunca plano.
// - Semánticos suavizados para fondo oscuro.

// --- Superficies (nivel de elevación) ---
const BG_BASE: egui::Color32 = egui::Color32::from_rgb(0x13, 0x13, 0x16); // nivel 0
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0x1B, 0x1C, 0x1F); // nivel 1: tarjetas
const BG_OVERLAY: egui::Color32 = egui::Color32::from_rgb(0x23, 0x24, 0x28); // nivel 2: diálogos/menús
const BG_HOVER: egui::Color32 = egui::Color32::from_rgb(0x26, 0x27, 0x2C);
const BG_ACTIVE: egui::Color32 = egui::Color32::from_rgb(0x2E, 0x2F, 0x34);
const BG_INSET: egui::Color32 = egui::Color32::from_rgb(0x0E, 0x0E, 0x10); // campos (hundido)
const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(0x2A, 0x2B, 0x30);

// --- Texto (jerarquía por énfasis) ---
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0xE4, 0xE4, 0xE7); // 87%
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0xA1, 0xA1, 0xAA); // 60%
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(0x63, 0x63, 0x6B); // ~40%

// --- Acento desaturado (el saturado vibra en dark) ---
const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x8A, 0xB4, 0xF8);
const ON_ACCENT: egui::Color32 = egui::Color32::from_rgb(0x0B, 0x12, 0x20); // texto oscuro sobre acento

// --- Semánticos suavizados para dark ---
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x81, 0xC9, 0x95);
const WARNING: egui::Color32 = egui::Color32::from_rgb(0xFD, 0xD6, 0x63);

static ACCENT_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 30));
static ACCENT_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 110));
static WARN_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0xFD, 0xD6, 0x63, 22));
static WARN_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0xFD, 0xD6, 0x63, 90));
static INFO_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x8A, 0xB4, 0xF8, 22));
static SUCCESS_TINT: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x81, 0xC9, 0x95, 26));
static SUCCESS_LINE: std::sync::LazyLock<egui::Color32> =
    std::sync::LazyLock::new(|| egui::Color32::from_rgba_unmultiplied(0x81, 0xC9, 0x95, 100));

fn app_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_BASE;
    v.extreme_bg_color = BG_INSET;
    v.window_fill = BG_OVERLAY;
    v.faint_bg_color = BG_HOVER;
    v.code_bg_color = BG_INSET;
    v.warn_fg_color = WARNING;
    v.error_fg_color = egui::Color32::from_rgb(0xF2, 0x8B, 0x82);
    v.hyperlink_color = ACCENT;

    let r = egui::Rounding::same(8.0);
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
    v.window_rounding = egui::Rounding::same(12.0);
    v.menu_rounding = egui::Rounding::same(8.0);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_SUBTLE);
    v
}

/// Botón primario: UNA sola acción principal por vista. Relleno de acento
/// desaturado con texto oscuro (alto contraste sin neón).
fn btn_primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).strong().color(ON_ACCENT))
            .fill(ACCENT)
            .rounding(8.0),
    )
}

/// Botón secundario: superficie sutil + borde fino, para acciones de apoyo.
fn btn_secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(TEXT_PRIMARY))
            .fill(BG_HOVER)
            .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
            .rounding(8.0),
    )
}

/// Botón terciario: solo texto, para acciones de baja prioridad.
fn btn_tertiary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(TEXT_SECONDARY))
            .fill(egui::Color32::TRANSPARENT)
            .rounding(8.0),
    )
}

/// Insignia neutra pequeña (texto secundario, nada de blanco sobre neón).
fn pill(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(BG_HOVER)
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(TEXT_SECONDARY));
        });
}

/// Insignia semántica con tinte suave (ej. "huérfana", "formalizado").
fn pill_tint(ui: &mut egui::Ui, text: &str, tint: egui::Color32, fg: egui::Color32) {
    egui::Frame::none()
        .fill(tint)
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(fg));
        });
}

/// Banda de aviso: tinte suave + borde tenue del color semántico.
fn banner(ui: &mut egui::Ui, text: &str, tint: egui::Color32, line: egui::Color32) {
    egui::Frame::none()
        .fill(tint)
        .stroke(egui::Stroke::new(1.0_f32, line))
        .rounding(8.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.5).color(TEXT_PRIMARY));
        });
    ui.add_space(6.0);
}

/// Tarjeta de nivel 1: superficie elevada + borde sutil.
fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(BG_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(12.0)
        .inner_margin(14.0)
        .show(ui, |ui| add(ui))
        .inner
}

/// Etiqueta superior de sección (overline): 11px, secundaria, mayúsculas.
fn overline(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.5)
            .color(TEXT_MUTED),
    );
    ui.add_space(2.0);
}

/// Título de sección dentro de una tarjeta.
fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .strong()
            .size(13.5)
            .color(TEXT_PRIMARY),
    );
    ui.add_space(4.0);
}

/// Celda de estado "checklist" de la barra superior: etiqueta + valor con
/// ✓ (listo) o ⋯ (pendiente) para que el usuario sepa qué falta de un vistazo.
fn checklist_cell(ui: &mut egui::Ui, label: &str, value: &str, ready: bool) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let mark = if ready { "✓" } else { "⋯" };
            let col = if ready { SUCCESS } else { TEXT_MUTED };
            ui.label(egui::RichText::new(mark).size(12.0).strong().color(col));
            ui.label(egui::RichText::new(label).size(10.5).color(TEXT_MUTED));
        });
        ui.label(egui::RichText::new(value).size(12.0).color(if ready {
            TEXT_PRIMARY
        } else {
            TEXT_SECONDARY
        }));
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_llm();
        self.poll_updates(ctx);

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
                egui::Color32::from_rgba_unmultiplied(0x0B, 0x12, 0x20, 210),
            );
            painter.rect_stroke(rect.shrink(24.0), 12.0, egui::Stroke::new(2.0_f32, ACCENT));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Suelte el archivo aquí\n(.zip del chat de WhatsApp o plantilla .docx)",
                egui::FontId::proportional(20.0),
                TEXT_PRIMARY,
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
                        .fill(*INFO_TINT)
                        .stroke(egui::Stroke::new(1.0_f32, *ACCENT_LINE))
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
                            .color(TEXT_PRIMARY)
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

        // ================= Barra superior =================
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Marca: cuadrado de acento tenue con el logo real de la app.
                egui::Frame::none()
                    .fill(*ACCENT_TINT)
                    .stroke(egui::Stroke::new(1.0_f32, *ACCENT_LINE))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(7.0, 6.0))
                    .show(ui, |ui| {
                        if let Some(icon) = &self.brand_icon {
                            ui.add(
                                egui::Image::new(icon)
                                    .fit_to_exact_size(egui::vec2(26.0, 26.0))
                                    .rounding(6.0),
                            );
                        } else {
                            ui.label(egui::RichText::new("📋").size(20.0));
                        }
                    });
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Parte Diario")
                            .size(17.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.label(
                        egui::RichText::new("Anti-Ruidos y Contaminación Sónica · P.N.")
                            .size(11.0)
                            .color(TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "v{} · {}",
                            parte_diario_core::build_info::VERSION,
                            parte_diario_core::build_info::GIT_HASH
                        ))
                        .size(9.5)
                        .color(TEXT_MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Chips de avisos (colapsados por defecto: no roban espacio).
                    let (nw, ni) = (self.warnings.len(), self.info.len());
                    if nw + ni > 0 {
                        let (tint, line, txt) = if self.warnings.is_empty() {
                            (
                                *INFO_TINT,
                                *ACCENT_LINE,
                                format!("ℹ {} nota{}", ni, if ni == 1 { "" } else { "s" }),
                            )
                        } else {
                            (
                                *WARN_TINT,
                                *WARN_LINE,
                                format!("⚠ {} aviso{}", nw, if nw == 1 { "" } else { "s" }),
                            )
                        };
                        let chip = egui::Frame::none()
                            .fill(tint)
                            .stroke(egui::Stroke::new(1.0_f32, line))
                            .rounding(10.0)
                            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(txt).size(12.0).color(TEXT_PRIMARY));
                            })
                            .response
                            .interact(egui::Sense::click());
                        if chip.clicked() {
                            self.alerts_expanded = !self.alerts_expanded;
                        }
                        chip.on_hover_text("Clic para ver u ocultar los avisos");
                    }
                    if btn_tertiary(ui, "⚙ Ajustes").clicked() {
                        // Re-sincronizar edits con la config guardada al abrir.
                        self.api_key_edit = self.cfg.api_key.clone();
                        self.provider_edit = self.cfg.provider.clone();
                        self.model_edit = self.cfg.model.clone();
                        self.output_dir_edit = self.cfg.output_dir.clone();
                        self.update_repository_edit = self.cfg.update_repository.clone();
                        self.check_updates_on_startup_edit = self.cfg.check_updates_on_startup;
                        self.show_settings = true;
                    }
                    if btn_secondary(ui, "📄 Plantilla").clicked() {
                        self.pick_template();
                    }
                    if btn_secondary(ui, "📂 Cargar .zip").clicked() {
                        self.pick_zip();
                    }
                });
            });
            ui.add_space(6.0);

            // Fila de contexto: checklist de pasos + rango + generar
            egui::Frame::none()
                .fill(BG_CARD)
                .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
                .rounding(10.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let zip_name = self
                            .zip_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Sin cargar".to_string());
                        checklist_cell(
                            ui,
                            "Chat",
                            &truncate_middle(&zip_name, 28),
                            self.zip_path.is_some(),
                        );
                        ui.separator();
                        let tpl_name = self
                            .template_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Sin cargar".to_string());
                        checklist_cell(
                            ui,
                            "Plantilla",
                            &truncate_middle(&tpl_name, 28),
                            self.template_path.is_some(),
                        );
                        ui.separator();
                        ui.vertical(|ui| {
                            overline(ui, "Rango (08:00 → 08:00)");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.date_from)
                                        .hint_text("10-10-2026")
                                        .desired_width(90.0),
                                );
                                ui.label(egui::RichText::new("→").color(TEXT_SECONDARY));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.date_to)
                                        .hint_text("11-10-2026")
                                        .desired_width(90.0),
                                );
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                            let resp = btn_primary(ui, "⚡ Generar Parte Diario");
                            let resp = if missing.is_empty() {
                                resp
                            } else {
                                resp.on_hover_text(format!("Antes debe: {}.", missing.join("; ")))
                            };
                            if resp.clicked() {
                                self.generate();
                            }
                            if let Some(gp) = self.generated_path.clone() {
                                if btn_secondary(ui, "📂 Abrir .docx").clicked() {
                                    if let Err(e) = open::that(&gp) {
                                        self.log(format!("No se pudo abrir el archivo: {e}"));
                                    }
                                }
                            }
                        });
                    });
                });

            // Mensaje de éxito tras generar (verde suave, con acción directa).
            if let Some(msg) = self.success_msg.clone() {
                ui.add_space(6.0);
                egui::Frame::none()
                    .fill(*SUCCESS_TINT)
                    .stroke(egui::Stroke::new(1.0_f32, *SUCCESS_LINE))
                    .rounding(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("✓ {msg}"))
                                    .size(12.5)
                                    .color(TEXT_PRIMARY),
                            );
                            if let Some(gp) = self.generated_path.clone() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if btn_tertiary(ui, "✕").clicked() {
                                            self.success_msg = None;
                                        }
                                        if btn_secondary(ui, "Abrir .docx").clicked() {
                                            if let Err(e) = open::that(&gp) {
                                                self.log(format!(
                                                    "No se pudo abrir el archivo: {e}"
                                                ));
                                            }
                                        }
                                    },
                                );
                            }
                        });
                    });
            }

            // Avisos expandibles (colapsados por defecto).
            if self.alerts_expanded && (!self.warnings.is_empty() || !self.info.is_empty()) {
                ui.add_space(6.0);
                egui::Frame::none()
                    .fill(BG_CARD)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .rounding(10.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Avisos y notas")
                                    .strong()
                                    .size(12.5)
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if btn_tertiary(ui, "Descartar todos").clicked() {
                                        self.warnings.clear();
                                        self.info.clear();
                                        self.alerts_expanded = false;
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for w in &self.warnings {
                                    banner(ui, &format!("⚠ {w}"), *WARN_TINT, *WARN_LINE);
                                }
                                for i in &self.info {
                                    banner(ui, &format!("ℹ {i}"), *INFO_TINT, *ACCENT_LINE);
                                }
                            });
                    });
            }
            ui.add_space(4.0);
        });

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
                        "{} novedades · {} incluidas · {} fotos",
                        self.novedades.len(),
                        n_inc,
                        n_fotos
                    ))
                    .size(12.0)
                    .color(TEXT_SECONDARY),
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
                        .color(ACCENT),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if btn_tertiary(ui, &format!("📝 Registro ({})", self.log.len())).clicked() {
                        self.show_log = !self.show_log;
                    }
                });
            });
        });

        // ================= Lateral: novedades =================
        egui::SidePanel::left("lista")
            .resizable(true)
            .default_width(360.0)
            .min_width(280.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Novedades ({})", self.novedades.len()))
                            .size(16.0)
                            .strong(),
                    );
                });
                ui.add_space(6.0);

                // Búsqueda.
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .id_salt("search_field")
                            .hint_text("Buscar por texto o #número…")
                            .desired_width(f32::INFINITY),
                    );
                    if !self.filter.is_empty() && btn_tertiary(ui, "✕").clicked() {
                        self.filter.clear();
                    }
                });
                ui.add_space(6.0);

                // Acciones masivas a ancho completo.
                let bw = ui.available_width();
                if ui
                    .add_sized(
                        [bw, 0.0],
                        egui::Button::new(
                            egui::RichText::new("✨ Formalizar TODAS con IA")
                                .color(TEXT_PRIMARY),
                        )
                        .fill(BG_HOVER)
                        .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .rounding(8.0),
                    )
                    .clicked()
                {
                    if self.cfg.api_key.trim().is_empty() {
                        self.log("Sin API key: configure su key de OpenRouter (gratis) en Ajustes, o use la limpieza sin conexión.".to_string());
                        self.api_key_edit = self.cfg.api_key.clone();
                        self.provider_edit = self.cfg.provider.clone();
                        self.model_edit = self.cfg.model.clone();
                        self.output_dir_edit = self.cfg.output_dir.clone();
                        self.show_settings = true;
                    } else {
                        let ids: Vec<usize> = self
                            .novedades
                            .iter()
                            .filter(|n| n.include && !n.body_raw.trim().is_empty())
                            .map(|n| n.id)
                            .collect();
                        self.spawn_formalize_many(ids);
                    }
                }
                if ui
                    .add_sized(
                        [bw, 0.0],
                        egui::Button::new(
                            egui::RichText::new("🧹 Limpieza a TODAS (sin conexión)")
                                .color(TEXT_PRIMARY),
                        )
                        .fill(BG_HOVER)
                        .stroke(egui::Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .rounding(8.0),
                    )
                    .clicked()
                {
                    let mut k = 0;
                    for n in self
                        .novedades
                        .iter_mut()
                        .filter(|n| n.include && !n.body_raw.trim().is_empty())
                    {
                        n.body_formal = llm::cleanup_offline(&n.body_raw);
                        k += 1;
                    }
                    self.log(format!(
                        "Limpieza sin conexión aplicada a {k} novedades."
                    ));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if btn_tertiary(ui, "✓ Incluir todas").clicked() {
                        for n in &mut self.novedades {
                            n.include = true;
                        }
                    }
                    if btn_tertiary(ui, "✗ Excluir todas").clicked() {
                        for n in &mut self.novedades {
                            n.include = false;
                        }
                    }
                });
                ui.separator();

                let visible = self.visible_novedades();
                if visible.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                egui::RichText::new("Sin coincidencias")
                                    .color(TEXT_SECONDARY),
                            );
                        });
                    });
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
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
                            .fill(if sel { *ACCENT_TINT } else { BG_CARD })
                            .stroke(if sel {
                                egui::Stroke::new(1.5_f32, *ACCENT_LINE)
                            } else {
                                egui::Stroke::new(1.0_f32, BORDER_SUBTLE)
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
                                            .color(TEXT_PRIMARY),
                                        );
                                        ui.add_space(3.0);
                                        ui.horizontal(|ui| {
                                            if let Some(f) = fecha {
                                                pill(ui, &f);
                                            }
                                            pill(ui, &format!("📷 {nphotos}"));
                                            if is_orphan {
                                                pill_tint(ui, "huérfana", *WARN_TINT, WARNING);
                                            }
                                            if !include {
                                                pill(ui, "excluida");
                                            }
                                            if busy {
                                                pill_tint(ui, "formalizando…", *ACCENT_TINT, ACCENT);
                                            } else if formalizado {
                                                pill_tint(ui, "✓ formalizado", *SUCCESS_TINT, SUCCESS);
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
                });
            });

        // ================= Central: editor =================
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.novedades.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(70.0);
                        ui.label(egui::RichText::new("📂").size(56.0));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Sin novedades").size(20.0).strong());
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Cargue el .zip exportado del chat de WhatsApp para comenzar.",
                            )
                            .color(TEXT_SECONDARY),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "También puede arrastrar el archivo hasta esta ventana.",
                            )
                            .color(TEXT_MUTED)
                            .size(12.0),
                        );
                        ui.add_space(16.0);
                        if btn_primary(ui, "📂 Cargar .zip del chat").clicked() {
                            self.pick_zip();
                        }
                        ui.add_space(8.0);
                        if btn_secondary(ui, "📄 Cargar plantilla .docx").clicked() {
                            self.pick_template();
                        }
                    });
                });
            } else {
                let idx = self.selected;
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                                .strong()
                                .size(15.0)
                                .color(TEXT_PRIMARY),
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
                                pill_tint(ui, "📷 FOTOS HUÉRFANAS", *WARN_TINT, WARNING);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if btn_tertiary(ui, "Siguiente pendiente →").clicked() {
                                        self.next_pending();
                                    }
                                    ui.small(
                                        egui::RichText::new(format!("{sender} · {fecha_txt}"))
                                            .color(TEXT_SECONDARY),
                                    );
                                },
                            );
                        });
                    });
                    ui.add_space(8.0);

                    // --- encabezados ---
                    card(ui, |ui| {
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
                    card(ui, |ui| {
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
                            let r1 = btn_primary(ui, "✨ Formalizar con IA");
                            if r1.clicked() && !busy {
                                if self.cfg.api_key.trim().is_empty() {
                                    self.log("Sin API key: configure su key en Ajustes o use la limpieza sin conexión.".to_string());
                                    self.api_key_edit = self.cfg.api_key.clone();
                                    self.provider_edit = self.cfg.provider.clone();
                                    self.model_edit = self.cfg.model.clone();
                                    self.output_dir_edit = self.cfg.output_dir.clone();
                                    self.show_settings = true;
                                } else {
                                    self.spawn_formalize(vec![id], true);
                                }
                            }
                            let r2 = btn_secondary(ui, "🧹 Limpieza sin conexión");
                            if r2.clicked() && !busy {
                                self.spawn_formalize(vec![id], false);
                            }
                            if busy {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new("formalizando…").color(TEXT_SECONDARY),
                                );
                            }
                        });
                    });
                    ui.add_space(8.0);

                    // --- texto formal ---
                    card(ui, |ui| {
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
                                .color(TEXT_SECONDARY),
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
                                            .fill(BG_HOVER)
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
                                                .color(TEXT_SECONDARY),
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
                                .color(TEXT_SECONDARY),
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
                                    .color(TEXT_SECONDARY),
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
            egui::Window::new("⚙ Ajustes")
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
                            ui.small("Consígala sin tarjeta en https://openrouter.ai/keys → péguela aquí.");
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
                                .color(TEXT_MUTED),
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
            egui::Window::new("📝 Registro de actividad")
                .id(egui::Id::new("log_win"))
                .open(&mut open)
                .default_width(580.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} línea(s)", self.log.len()))
                                .size(12.0)
                                .color(TEXT_SECONDARY),
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

        if !self.pending_llm.is_empty() {
            ctx.request_repaint();
        }
    }
}

/// Abrevia un nombre de archivo por el centro: "informe…final.docx".
fn truncate_middle(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let head = keep / 2;
    let tail = keep - head;
    let out: String = chars[..head]
        .iter()
        .chain(std::iter::once(&'…'))
        .chain(chars[chars.len() - tail..].iter())
        .collect();
    out
}
