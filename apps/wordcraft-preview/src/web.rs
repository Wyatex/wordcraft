//! Browser runner and bridge used by the JavaScript API.

use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::Value;
use wasm_bindgen::JsCast as _;

use crate::preview_session::{PreviewDoc, ThemeMode, UiLang};
use crate::preview_ui::PreviewViewer;

const CANVAS_ID: &str = "wordcraft_preview_canvas";
const LOADING_ID: &str = "wordcraft_preview_loading";
const READY_EVENT: &str = "wordcraft-preview-ready";
const MAX_QUEUED_FILES: usize = 8;
const MAX_QUEUED_BYTES: usize = 256 * 1024 * 1024;
const MAX_SYSTEM_FONTS: usize = 500;
const MAX_SYSTEM_FONT_BYTES: usize = 256 * 1024 * 1024;
const MAX_SYSTEM_FONTS_TRIED: u32 = 4000;

pub(crate) const READ_ONLY_ERROR: &str = "wordcraft-preview is strictly read-only";

#[derive(Clone, Debug, Default)]
pub(crate) struct PreviewState {
    pub page_count: u32,
    pub current_page: u32,
    pub selection: String,
    pub find_count: u32,
}

type FileQueue = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

struct PreviewBridge {
    inbox: FileQueue,
    commands: VecDeque<(String, Value)>,
}

static BRIDGE: LazyLock<Arc<Mutex<PreviewBridge>>> =
    LazyLock::new(|| Arc::new(Mutex::new(PreviewBridge { inbox: Arc::new(Mutex::new(Vec::new())), commands: VecDeque::new() })));

static STATE: LazyLock<Arc<Mutex<PreviewState>>> = LazyLock::new(|| Arc::new(Mutex::new(PreviewState::default())));

static CONTEXT: LazyLock<Arc<Mutex<Option<egui::Context>>>> = LazyLock::new(|| Arc::new(Mutex::new(None)));

static FONTS_DIRTY: LazyLock<std::sync::atomic::AtomicBool> = LazyLock::new(|| std::sync::atomic::AtomicBool::new(false));

pub(crate) fn state() -> PreviewState {
    STATE.lock().map(|s| s.clone()).unwrap_or_else(|poisoned| poisoned.into_inner().clone())
}

pub(crate) fn queue_command(id: &str, params: Value) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 {
        return Err("preview command id is invalid".into());
    }
    let mut bridge = BRIDGE.lock().map_err(|_| "preview bridge is unavailable".to_string())?;
    if bridge.commands.len() >= 256 {
        return Err("preview command queue is full".into());
    }
    bridge.commands.push_back((id.to_string(), params));
    drop(bridge);
    request_repaint();
    Ok(())
}

pub(crate) fn queue_file(name: String, bytes: Vec<u8>) -> Result<(), String> {
    if name.is_empty() || name.len() > 1024 {
        return Err("preview filename is invalid".into());
    }
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("preview files are limited to 128 MiB".into());
    }
    let bridge = BRIDGE.lock().map_err(|_| "preview bridge is unavailable".to_string())?;
    enqueue_file(&bridge.inbox, name, bytes)?;
    drop(bridge);
    request_repaint();
    Ok(())
}

fn enqueue_file(inbox: &FileQueue, name: String, bytes: Vec<u8>) -> Result<(), String> {
    let mut pending = inbox.lock().map_err(|_| "preview file inbox is unavailable".to_string())?;
    let pending_bytes = pending.iter().map(|(_, data)| data.len()).fold(0usize, usize::saturating_add);
    if pending.len() >= MAX_QUEUED_FILES || pending_bytes.saturating_add(bytes.len()) > MAX_QUEUED_BYTES {
        return Err("preview file queue is full".into());
    }
    pending.push((name, bytes));
    Ok(())
}

fn request_repaint() {
    match CONTEXT.lock() {
        Ok(guard) => {
            if let Some(context) = guard.as_ref() {
                context.request_repaint();
            }
        }
        Err(poisoned) => {
            if let Some(context) = poisoned.into_inner().as_ref() {
                context.request_repaint();
            }
        }
    }
}

/// Query the browser's Local Font Access API and install a bounded set of local faces.
pub(crate) fn load_system_fonts(ctx: egui::Context) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let query_fn = match js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("queryLocalFonts")) {
        Ok(value) if value.is_function() => js_sys::Function::from(value),
        _ => {
            log::info!("Local Font Access is not supported by this browser");
            return;
        }
    };
    let fonts_promise = match query_fn.call0(&window) {
        Ok(value) => js_sys::Promise::from(value),
        Err(error) => {
            log::warn!("queryLocalFonts call failed: {error:?}");
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let fonts_value = match wasm_bindgen_futures::JsFuture::from(fonts_promise).await {
            Ok(value) => value,
            Err(error) => {
                log::warn!("Local Font Access was denied: {error:?}");
                return;
            }
        };
        let font_list = js_sys::Array::from(&fonts_value);
        let mut loaded = 0usize;
        let mut total_bytes = 0usize;
        for index in 0..font_list.length().min(MAX_SYSTEM_FONTS_TRIED) {
            if loaded >= MAX_SYSTEM_FONTS || total_bytes >= MAX_SYSTEM_FONT_BYTES {
                break;
            }
            let font_data = font_list.get(index);
            let Ok(blob_method) = js_sys::Reflect::get(&font_data, &wasm_bindgen::JsValue::from_str("blob")) else {
                continue;
            };
            if !blob_method.is_function() {
                continue;
            }
            let Ok(blob_value) = js_sys::Function::from(blob_method).call0(&font_data) else {
                continue;
            };
            let Ok(blob_value) = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(blob_value)).await else {
                continue;
            };
            let Ok(blob) = blob_value.dyn_into::<web_sys::Blob>() else {
                continue;
            };
            let size = blob.size().clamp(0.0, usize::MAX as f64) as usize;
            if size > wordcraft_fonts::MAX_FONT_FILE_BYTES || total_bytes.saturating_add(size) > MAX_SYSTEM_FONT_BYTES {
                continue;
            }
            let Ok(array_buffer) = wasm_bindgen_futures::JsFuture::from(blob.array_buffer()).await else {
                continue;
            };
            let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
            if bytes.len() > wordcraft_fonts::MAX_FONT_FILE_BYTES {
                continue;
            }
            total_bytes = total_bytes.saturating_add(bytes.len());
            let postscript_name = js_sys::Reflect::get(&font_data, &wasm_bindgen::JsValue::from_str("postscriptName"))
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_else(|| format!("local-font-{index}"));
            let name = format!("{postscript_name}.ttf");
            match wordcraft_fonts::FontDb::global().install_font(&name, &bytes) {
                Ok(_) => loaded += 1,
                Err(wordcraft_fonts::UserFontError::AlreadyAvailable(_)) => {}
                Err(error) => log::warn!("local font {name} skipped: {error}"),
            }
        }
        log::info!("local font loading finished: {loaded} new faces, {total_bytes} bytes");
        if loaded > 0 {
            FONTS_DIRTY.store(true, std::sync::atomic::Ordering::Release);
        }
        ctx.request_repaint();
    });
}

pub(crate) fn request_system_fonts() -> Result<(), String> {
    let context = CONTEXT
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
        .ok_or_else(|| "preview is still starting".to_string())?;
    load_system_fonts(context);
    Ok(())
}

pub(crate) fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            log::error!("WordCraft Preview: no browser document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("WordCraft Preview: missing canvas #{CANVAS_ID}");
            return;
        };

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    PreviewViewer::install_fonts(&cc.egui_ctx);
                    let inbox = BRIDGE.lock().map(|bridge| bridge.inbox.clone()).unwrap_or_else(|poisoned| poisoned.into_inner().inbox.clone());
                    Ok(Box::new(PreviewShell { doc: PreviewDoc::new(), viewer: PreviewViewer::new(), inbox }))
                }),
            )
            .await;

        let started = result.is_ok();
        if let Some(loading) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => loading.remove(),
                Err(error) => {
                    loading.set_inner_html(&format!("<p>WordCraft Preview failed to start: {error:?}</p><p>A browser with WebGL2 is required.</p>"));
                }
            }
        }
        if started
            && let Some(window) = web_sys::window()
            && let Ok(event) = web_sys::CustomEvent::new(READY_EVENT)
        {
            let _ = window.dispatch_event(&event);
        }
    });
}

struct PreviewShell {
    doc: PreviewDoc,
    viewer: PreviewViewer,
    inbox: FileQueue,
}

impl PreviewShell {
    fn command(&mut self, ctx: &egui::Context, id: &str, params: Value) -> Result<(), String> {
        match id {
            "preview.zoomIn" => self.doc.zoom_in(),
            "preview.zoomOut" => self.doc.zoom_out(),
            "preview.zoomToFit" => self.doc.zoom_to_fit(),
            "view.zoom" => {
                let pct = params.get("value").and_then(Value::as_f64).unwrap_or(100.0);
                self.doc.set_zoom(pct);
            }
            "preview.goToPage" => {
                let page = params.get("page").and_then(Value::as_u64).unwrap_or(1) as u32;
                self.doc.go_to_page(page)?;
            }
            "select.text" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                let occ = params.get("occurrence").and_then(Value::as_u64).unwrap_or(1) as u32;
                self.doc.select_text(text, occ)?;
            }
            "edit.find" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                let match_case = params.get("matchCase").and_then(Value::as_bool).unwrap_or(false);
                let whole_word = params.get("wholeWord").and_then(Value::as_bool).unwrap_or(false);
                self.doc.find(text, match_case, whole_word);
            }
            "edit.findNext" => {
                self.doc.find_next();
            }
            "edit.findPrevious" => {
                self.doc.find_previous();
            }
            "ui.language" => {
                let code = params.get("value").and_then(Value::as_str).unwrap_or("auto");
                let Some(lang) = UiLang::from_code(code) else {
                    return Err("unsupported interface language".into());
                };
                self.doc.lang = lang;
            }
            "view.interfaceTheme" => {
                let mode = params.get("value").and_then(Value::as_str).unwrap_or("system");
                self.doc.theme = match mode {
                    "light" => ThemeMode::Light,
                    "dark" => ThemeMode::Dark,
                    _ => ThemeMode::Auto,
                };
            }
            "preview.loadSystemFonts" => {
                if request_system_fonts().is_err() {
                    load_system_fonts(ctx.clone());
                }
            }
            "preview.setEditable" => return Err(READ_ONLY_ERROR.into()),
            _ => return Err(format!("command `{id}` is unavailable in read-only preview")),
        }
        Ok(())
    }

    fn update_state(&self) {
        let next = PreviewState {
            page_count: self.doc.page_count(),
            current_page: self.doc.current_page,
            selection: self.doc.selected_text(),
            find_count: self.doc.find_count(),
        };
        if let Ok(mut state) = STATE.lock() {
            *state = next;
        }
    }
}

impl eframe::App for PreviewShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(mut context) = CONTEXT.lock() {
            *context = Some(ctx.clone());
        }
        if FONTS_DIRTY.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.doc.relayout();
            self.viewer.clear_textures();
        }
        let commands = BRIDGE
            .lock()
            .map(|mut bridge| bridge.commands.drain(..).collect::<Vec<_>>())
            .unwrap_or_else(|poisoned| poisoned.into_inner().commands.drain(..).collect());
        for (id, params) in commands {
            if let Err(error) = self.command(ctx, &id, params) {
                log::warn!("preview command {id} failed: {error}");
            }
        }
        let queued_files =
            self.inbox.lock().map(|mut pending| std::mem::take(&mut *pending)).unwrap_or_else(|poisoned| std::mem::take(&mut *poisoned.into_inner()));
        for (name, bytes) in queued_files {
            if let Err(error) = self.doc.load_bytes(&name, &bytes) {
                log::warn!("failed to load preview file {name}: {error}");
                self.doc.last_error = Some(error);
            } else {
                self.viewer.clear_textures();
            }
        }
        self.update_state();
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        for file in std::mem::take(&mut raw.dropped_files) {
            let inbox = self.inbox.clone();
            let context = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let name = file.path().file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "dropped-document".into());
                if let Some(web_file) = file.web_file()
                    && (!web_file.size().is_finite() || web_file.size() > 128.0 * 1024.0 * 1024.0)
                {
                    log::warn!("dropped preview file exceeds 128 MiB");
                    return;
                }
                match file.bytes_async().await {
                    Ok(bytes) if bytes.len() <= 128 * 1024 * 1024 => {
                        if let Err(error) = enqueue_file(&inbox, name, bytes) {
                            log::warn!("dropped preview file was not queued: {error}");
                        } else {
                            context.request_repaint();
                        }
                    }
                    Ok(_) => log::warn!("dropped preview file exceeds 128 MiB"),
                    Err(error) => log::warn!("could not read dropped preview file: {error}"),
                }
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.viewer.ui(&ctx, ui, &mut self.doc);
        self.update_state();
    }
}
