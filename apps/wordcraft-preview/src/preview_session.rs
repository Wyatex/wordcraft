//! Document state, layout, search, selection, and format loading for WordCraft Preview.

use std::sync::Arc;

use wordcraft_doc::{Document, Pos, StoryRef};
use wordcraft_geom::Region;
use wordcraft_layout::{DocLayout, LayoutCache, LayoutOptions, layout};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 5.0;
const ZOOM_STEP: f32 = 1.15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiLang {
    #[default]
    Auto,
    En,
    ZhHans,
}

impl UiLang {
    pub fn supported() -> &'static [(&'static str, &'static str)] {
        &[("en", "English"), ("zh-hans", "简体中文")]
    }

    pub fn is_supported(code: &str) -> bool {
        let trimmed = code.trim();
        trimmed.eq_ignore_ascii_case("auto") || Self::supported().iter().any(|(c, _)| c.eq_ignore_ascii_case(trimmed))
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let trimmed = code.trim();
        if trimmed.eq_ignore_ascii_case("auto") {
            Some(Self::Auto)
        } else if trimmed.eq_ignore_ascii_case("en") {
            Some(Self::En)
        } else if trimmed.eq_ignore_ascii_case("zh-hans") {
            Some(Self::ZhHans)
        } else {
            None
        }
    }
}

pub struct PreviewDoc {
    pub doc: Document,
    layout_cache: LayoutCache,
    pub layout: Arc<DocLayout>,
    pub doc_rev: u64,
    pub loaded: bool,
    pub zoom: f32,
    pub fit_width: bool,
    pub selection: Option<(Pos, Pos)>,
    pub find_query: String,
    pub find_matches: Vec<(Pos, Pos)>,
    pub find_index: usize,
    pub current_page: u32,
    pub theme: ThemeMode,
    pub lang: UiLang,
    pub pending_scroll_page: Option<u32>,
    pub pending_scroll_pos: Option<Pos>,
    pub last_error: Option<String>,
}

impl Default for PreviewDoc {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewDoc {
    pub fn new() -> Self {
        let region = Region::default();
        let mut doc = Document::new();
        let (_, w, h) = region.paper();
        doc.last_section.page_w = w;
        doc.last_section.page_h = h;
        doc.ensure_nonempty();
        let mut cache = LayoutCache::new();
        let doc_layout = Arc::new(layout(&doc, &mut cache, &LayoutOptions::default()));
        Self {
            doc,
            layout_cache: cache,
            layout: doc_layout,
            doc_rev: 1,
            loaded: false,
            zoom: 1.0,
            fit_width: false,
            selection: None,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_index: 0,
            current_page: 1,
            theme: ThemeMode::Auto,
            lang: UiLang::Auto,
            pending_scroll_page: None,
            pending_scroll_pos: None,
            last_error: None,
        }
    }

    pub fn relayout(&mut self) {
        self.layout_cache = LayoutCache::new();
        self.layout = Arc::new(layout(&self.doc, &mut self.layout_cache, &LayoutOptions::default()));
        self.doc_rev = self.doc_rev.wrapping_add(1);
    }

    pub fn load_bytes(&mut self, filename: &str, bytes: &[u8]) -> Result<(), String> {
        let mut doc = parse_document(filename, bytes)?;
        doc.ensure_nonempty();
        self.doc = doc;
        self.loaded = true;
        self.selection = None;
        self.find_query.clear();
        self.find_matches.clear();
        self.find_index = 0;
        self.current_page = 1;
        self.pending_scroll_page = Some(0);
        self.pending_scroll_pos = None;
        self.last_error = None;
        self.relayout();
        Ok(())
    }

    pub fn page_count(&self) -> u32 {
        self.layout.pages.len().min(u32::MAX as usize) as u32
    }

    pub fn zoom_in(&mut self) {
        self.fit_width = false;
        self.zoom = (self.zoom * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn zoom_out(&mut self) {
        self.fit_width = false;
        self.zoom = (self.zoom / ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn set_zoom(&mut self, percentage: f64) {
        self.fit_width = false;
        self.zoom = ((percentage as f32) / 100.0).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn zoom_to_fit(&mut self) {
        self.fit_width = true;
    }

    pub fn reset_zoom(&mut self) {
        self.fit_width = false;
        self.zoom = 1.0;
    }

    pub fn go_to_page(&mut self, page: u32) -> Result<(), String> {
        if page == 0 {
            return Err("page numbers start at 1".into());
        }
        let max_page = self.page_count().max(1);
        let clamped = page.min(max_page);
        self.current_page = clamped;
        self.pending_scroll_page = Some(clamped - 1);
        Ok(())
    }

    pub fn selected_text(&self) -> String {
        let Some((a, b)) = &self.selection else {
            return String::new();
        };
        if a == b {
            return String::new();
        }
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        self.doc.copy_range(start, end).plain_text()
    }

    pub fn select_all(&mut self) {
        let start = self.doc.start_of(StoryRef::Body);
        let end = self.doc.end_of(StoryRef::Body);
        self.selection = Some((start, end));
    }

    pub fn select_text(&mut self, text: &str, occurrence: u32) -> Result<(), String> {
        if text.is_empty() {
            return Err("selection text must not be empty".into());
        }
        let target = occurrence.saturating_sub(1) as usize;
        let mut count = 0usize;
        for path in self.doc.para_paths(StoryRef::Body) {
            let Some(para) = self.doc.para(StoryRef::Body, &path) else {
                continue;
            };
            for (start, _) in para.text.match_indices(text) {
                if count == target {
                    let a = Pos { story: StoryRef::Body, path: path.clone(), off: start };
                    let b = Pos { story: StoryRef::Body, path, off: start + text.len() };
                    self.selection = Some((a.clone(), b));
                    self.pending_scroll_pos = Some(a);
                    return Ok(());
                }
                count += 1;
            }
        }
        Err("text not found in document".into())
    }

    pub fn find(&mut self, query: &str, match_case: bool, whole_word: bool) -> u32 {
        self.find_query = query.to_string();
        self.find_matches.clear();
        self.find_index = 0;
        if query.is_empty() {
            return 0;
        }
        for path in self.doc.para_paths(StoryRef::Body) {
            let Some(para) = self.doc.para(StoryRef::Body, &path) else {
                continue;
            };
            for (start, end) in collect_matches(&para.text, query, match_case, whole_word) {
                let a = Pos { story: StoryRef::Body, path: path.clone(), off: start };
                let b = Pos { story: StoryRef::Body, path: path.clone(), off: end };
                self.find_matches.push((a, b));
            }
        }
        if let Some((a, b)) = self.find_matches.first() {
            self.selection = Some((a.clone(), b.clone()));
            self.pending_scroll_pos = Some(a.clone());
        }
        self.find_matches.len().min(u32::MAX as usize) as u32
    }

    pub fn find_next(&mut self) -> bool {
        if self.find_matches.is_empty() {
            return false;
        }
        self.find_index = (self.find_index + 1) % self.find_matches.len();
        if let Some((a, b)) = self.find_matches.get(self.find_index) {
            self.selection = Some((a.clone(), b.clone()));
            self.pending_scroll_pos = Some(a.clone());
        }
        true
    }

    pub fn find_previous(&mut self) -> bool {
        if self.find_matches.is_empty() {
            return false;
        }
        self.find_index = if self.find_index == 0 { self.find_matches.len() - 1 } else { self.find_index - 1 };
        if let Some((a, b)) = self.find_matches.get(self.find_index) {
            self.selection = Some((a.clone(), b.clone()));
            self.pending_scroll_pos = Some(a.clone());
        }
        true
    }

    pub fn ordered_selection(&self) -> Option<(Pos, Pos)> {
        let (a, b) = self.selection.as_ref()?;
        if a <= b { Some((a.clone(), b.clone())) } else { Some((b.clone(), a.clone())) }
    }

    pub fn select_word_at(&mut self, pos: &Pos) {
        if let Some(para) = self.doc.para_at(pos) {
            let (s, e) = para.word_at(pos.off);
            let a = Pos { story: pos.story, path: pos.path.clone(), off: s };
            let b = Pos { story: pos.story, path: pos.path.clone(), off: e };
            self.selection = Some((a, b));
        }
    }

    pub fn select_paragraph_at(&mut self, pos: &Pos) {
        if let Some(para) = self.doc.para_at(pos) {
            let a = Pos { story: pos.story, path: pos.path.clone(), off: 0 };
            let b = Pos { story: pos.story, path: pos.path.clone(), off: para.len() };
            self.selection = Some((a, b));
        }
    }

    pub fn find_count(&self) -> u32 {
        self.find_matches.len().min(u32::MAX as usize) as u32
    }
}

fn parse_document(filename: &str, bytes: &[u8]) -> Result<Document, String> {
    let ext = filename.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "docx" | "docm" | "dotx" | "dotm" => wordcraft_docx::read(bytes).map_err(|e| e.to_string()),
        "xml" => wordcraft_docx::read_xml(bytes).map_err(|e| e.to_string()),
        #[cfg(feature = "docbin")]
        "doc" | "dot" => {
            wordcraft_doc::cfb_guard::check(bytes).map_err(|e| e.to_string())?;
            let shared = craft_doc_binary::read(bytes).map_err(|e| e.to_string())?;
            wordcraft_docx::from_shared_model(shared).map_err(|e| e.to_string())
        }
        #[cfg(not(feature = "extra-formats"))]
        "txt" | "text" => {
            let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n");
            Ok(Document::from_text(&text))
        }
        #[cfg(feature = "extra-formats")]
        "rtf" | "txt" | "text" | "md" | "markdown" | "html" | "htm" | "xhtml" | "mht" | "mhtml" => {
            let page = Document::new().last_section;
            match wordcraft_formats::import_on(&ext, bytes, &page) {
                Some(res) => res,
                None => Err(format!("Unsupported document format: .{ext}")),
            }
        }
        _ if bytes.starts_with(b"PK\x03\x04") => wordcraft_docx::read(bytes).map_err(|e| e.to_string()),
        _ => Err(format!("Unsupported document format: {filename}")),
    }
}

fn collect_matches(haystack: &str, needle: &str, match_case: bool, whole_word: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut start = 0usize;
    while start < haystack.len() {
        let Some(end) = match_at(haystack, start, needle, match_case) else {
            let step = haystack[start..].chars().next().map_or(1, char::len_utf8);
            start += step;
            continue;
        };
        if !whole_word || is_whole_word(haystack, start, end) {
            out.push((start, end));
            start = end.max(start + 1);
        } else {
            let step = haystack[start..].chars().next().map_or(1, char::len_utf8);
            start += step;
        }
    }
    out
}

fn match_at(haystack: &str, start: usize, needle: &str, match_case: bool) -> Option<usize> {
    if match_case {
        return haystack[start..].starts_with(needle).then_some(start + needle.len());
    }
    let mut h_chars = haystack[start..].char_indices();
    for nc in needle.chars() {
        let (_, hc) = h_chars.next()?;
        if !chars_equal_ignore_case(hc, nc) {
            return None;
        }
    }
    let end = match h_chars.next() {
        Some((rel, _)) => start + rel,
        None => haystack.len(),
    };
    Some(end)
}

fn chars_equal_ignore_case(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_whole_word(haystack: &str, start: usize, end: usize) -> bool {
    let left_ok = haystack[..start].chars().next_back().is_none_or(|c| !is_word_char(c));
    let right_ok = haystack[end..].chars().next().is_none_or(|c| !is_word_char(c));
    left_ok && right_ok
}
