//! wasm-bindgen API for the WordCraft Preview package.

use serde_json::json;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Blob, File};

use crate::preview_session::UiLang;
use crate::web;

const MAX_FILE_BYTES: f64 = 128.0 * 1024.0 * 1024.0;

#[wasm_bindgen]
pub struct WordCraftPreview;

#[wasm_bindgen]
impl WordCraftPreview {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WordCraftPreview, JsValue> {
        Ok(Self)
    }

    /// Load a WordCraft-supported document from a browser File or Blob.
    #[wasm_bindgen(js_name = loadFile)]
    pub async fn load_file(&mut self, file: JsValue, filename: Option<String>) -> Result<(), JsValue> {
        let (name, bytes) = if let Some(file) = file.dyn_ref::<File>() {
            let name = filename.unwrap_or_else(|| file.name());
            let bytes = read_blob_bytes(file.as_ref()).await?;
            (name, bytes)
        } else if let Some(blob) = file.dyn_ref::<Blob>() {
            let name = filename.unwrap_or_else(|| "Document.docx".into());
            let bytes = read_blob_bytes(blob).await?;
            (name, bytes)
        } else {
            return Err(JsValue::from_str("Expected a File or Blob object"));
        };
        web::queue_file(name, bytes).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen(js_name = zoomIn)]
    pub fn zoom_in(&self) -> Result<(), JsValue> {
        send_command("preview.zoomIn", json!({}))
    }

    #[wasm_bindgen(js_name = zoomOut)]
    pub fn zoom_out(&self) -> Result<(), JsValue> {
        send_command("preview.zoomOut", json!({}))
    }

    #[wasm_bindgen(js_name = zoomToFit)]
    pub fn zoom_to_fit(&self) -> Result<(), JsValue> {
        send_command("preview.zoomToFit", json!({}))
    }

    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&self, percentage: f64) -> Result<(), JsValue> {
        if !percentage.is_finite() {
            return Err(JsValue::from_str("zoom percentage must be finite"));
        }
        send_command("view.zoom", json!({"value": percentage.clamp(10.0, 500.0)}))
    }

    #[wasm_bindgen(js_name = setEditable)]
    pub fn set_editable(&self, editable: bool) -> Result<(), JsValue> {
        if editable {
            return Err(JsValue::from_str(web::READ_ONLY_ERROR));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = setReadOnly)]
    pub fn set_read_only(&self, read_only: bool) -> Result<(), JsValue> {
        self.set_editable(!read_only)
    }

    #[wasm_bindgen(js_name = getPageCount)]
    pub fn get_page_count(&self) -> u32 {
        web::state().page_count
    }

    #[wasm_bindgen(js_name = getCurrentPage)]
    pub fn get_current_page(&self) -> u32 {
        web::state().current_page
    }

    #[wasm_bindgen(js_name = goToPage)]
    pub fn go_to_page(&self, page: u32) -> Result<(), JsValue> {
        if page == 0 {
            return Err(JsValue::from_str("page numbers start at 1"));
        }
        send_command("preview.goToPage", json!({"page": page}))
    }

    /// Returns the selected plain text. Selection changes are reflected after the next frame.
    #[wasm_bindgen(js_name = getSelection)]
    pub fn get_selection(&self) -> String {
        web::state().selection
    }

    /// Selects the first occurrence of `text` in the document. The name is retained for parity
    /// with GridCraft Preview; use `selectText` when selecting a specific occurrence.
    #[wasm_bindgen(js_name = selectRange)]
    pub fn select_range(&self, text: String) -> Result<(), JsValue> {
        self.select_text(text, 1)
    }

    #[wasm_bindgen(js_name = selectText)]
    pub fn select_text(&self, text: String, occurrence: u32) -> Result<(), JsValue> {
        if text.is_empty() {
            return Err(JsValue::from_str("selection text must not be empty"));
        }
        if text.len() > 100_000 {
            return Err(JsValue::from_str("selection text is too long"));
        }
        send_command("select.text", json!({"text": text, "occurrence": occurrence.max(1)}))
    }

    #[wasm_bindgen(js_name = find)]
    pub fn find(&self, query: String, options: JsValue) -> Result<u32, JsValue> {
        if query.len() > 100_000 {
            return Err(JsValue::from_str("search query is too long"));
        }
        let mut params = serde_json::Map::new();
        params.insert("text".into(), serde_json::Value::String(query));
        if options.is_object() {
            for key in ["matchCase", "wholeWord", "regex"] {
                if let Ok(value) = js_sys::Reflect::get(&options, &JsValue::from_str(key))
                    && let Some(value) = value.as_bool()
                {
                    params.insert(key.into(), serde_json::Value::Bool(value));
                }
            }
        }
        send_command("edit.find", serde_json::Value::Object(params))?;
        Ok(web::state().find_count)
    }

    #[wasm_bindgen(js_name = findNext)]
    pub fn find_next(&self) -> Result<bool, JsValue> {
        send_command("edit.findNext", json!({}))?;
        Ok(web::state().find_count > 0)
    }

    #[wasm_bindgen(js_name = findPrevious)]
    pub fn find_previous(&self) -> Result<bool, JsValue> {
        send_command("edit.findPrevious", json!({}))?;
        Ok(web::state().find_count > 0)
    }

    #[wasm_bindgen(js_name = setLanguage)]
    pub fn set_language(&self, language: String) -> Result<(), JsValue> {
        if !UiLang::is_supported(&language) {
            return Err(JsValue::from_str("unsupported interface language"));
        }
        send_command("ui.language", json!({"value": language}))
    }

    /// Languages whose catalogs are embedded in this build, as `[code, native name]` pairs.
    #[wasm_bindgen(js_name = getLanguages)]
    pub fn get_languages(&self) -> js_sys::Array {
        let languages = js_sys::Array::new();
        for &(code, name) in UiLang::supported() {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from_str(code));
            pair.push(&JsValue::from_str(name));
            languages.push(&pair);
        }
        languages
    }

    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&self, theme: String) -> Result<(), JsValue> {
        let mode = match theme.as_str() {
            "auto" => "system",
            "light" | "dark" => theme.as_str(),
            _ => return Err(JsValue::from_str("theme must be light, dark, or auto")),
        };
        send_command("view.interfaceTheme", json!({"value": mode}))
    }

    /// Ask the browser for installed local fonts and add readable faces to the preview font DB.
    /// Browsers may deny this permission; that failure is reported asynchronously in the console.
    #[wasm_bindgen(js_name = loadSystemFonts)]
    pub fn load_system_fonts(&self) -> Result<(), JsValue> {
        web::request_system_fonts().or_else(|_| send_command("preview.loadSystemFonts", json!({})))
    }
}

fn send_command(id: &str, params: serde_json::Value) -> Result<(), JsValue> {
    web::queue_command(id, params).map_err(|error| JsValue::from_str(&error))
}

async fn read_blob_bytes(blob: &Blob) -> Result<Vec<u8>, JsValue> {
    if !blob.size().is_finite() || blob.size() > MAX_FILE_BYTES {
        return Err(JsValue::from_str("preview files are limited to 128 MiB"));
    }
    let reader = web_sys::FileReader::new()?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let reader_for_load = reader.clone();
        let reject_for_load = reject.clone();
        let onload = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(move |_| {
            if let Ok(value) = reader_for_load.result() {
                let _ = resolve.call1(&JsValue::NULL, &value);
            } else {
                let _ = reject_for_load.call1(&JsValue::NULL, &JsValue::from_str("Failed to read preview file"));
            }
        });
        let onerror = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(move |_| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("Failed to read preview file"));
        });
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        reader.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onload.forget();
        onerror.forget();
    });
    reader.read_as_array_buffer(blob)?;
    let value = wasm_bindgen_futures::JsFuture::from(promise).await?;
    let bytes = js_sys::Uint8Array::new(&value).to_vec();
    if bytes.len() as f64 > MAX_FILE_BYTES {
        return Err(JsValue::from_str("preview files are limited to 128 MiB"));
    }
    Ok(bytes)
}
