//! Pure-egui read-only page viewer UI for WordCraft Preview.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{
    Color32, ColorImage, Context, CursorIcon, FontData, FontDefinitions, FontFamily, Key, Pos2, Rect, ScrollArea, TextureHandle, TextureOptions, Ui,
    Vec2,
};
use wordcraft_doc::{Pos, StoryRef};
use wordcraft_render::{RenderOptions, render_page};

use crate::preview_session::{PreviewDoc, ThemeMode, UiLang};

const PT_TO_PX: f32 = 96.0 / 72.0;
const PAGE_GAP: f32 = 16.0;
const PAGE_PADDING: f32 = 24.0;
const MAX_CACHED_TEXTURES: usize = 24;
const MAX_RENDER_SIDE: f32 = 3072.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    doc_rev: u64,
    page_idx: usize,
    scale_pct: u32,
    dark: bool,
}

pub struct PreviewViewer {
    textures: HashMap<TextureKey, (u64, TextureHandle)>,
    tick: u64,
    drag_anchor: Option<Pos>,
}

impl Default for PreviewViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewViewer {
    pub fn new() -> Self {
        Self { textures: HashMap::new(), tick: 0, drag_anchor: None }
    }

    pub fn install_fonts(ctx: &Context) {
        let mut defs = FontDefinitions::empty();
        if let Some(bytes) = wordcraft_fonts::bundled().first().copied() {
            defs.font_data.insert("fallback".to_owned(), Arc::new(FontData::from_static(bytes)));
            defs.families.insert(FontFamily::Proportional, vec!["fallback".to_owned()]);
            defs.families.insert(FontFamily::Monospace, vec!["fallback".to_owned()]);
        }
        ctx.set_fonts(defs);
    }

    pub fn clear_textures(&mut self) {
        self.textures.clear();
    }

    pub fn ui(&mut self, ctx: &Context, ui: &mut Ui, state: &mut PreviewDoc) {
        let dark = apply_theme(ctx, state.theme);
        handle_shortcuts(ctx, state);
        self.tick = self.tick.wrapping_add(1);

        let bg = if dark { Color32::from_rgb(30, 31, 34) } else { Color32::from_rgb(232, 234, 237) };
        let frame = egui::Frame::NONE.fill(bg);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            if !state.loaded {
                paint_empty_state(ui, state, dark);
                return;
            }
            self.render_pages(ctx, ui, state, dark);
        });
    }

    fn render_pages(&mut self, ctx: &Context, ui: &mut Ui, state: &mut PreviewDoc, dark: bool) {
        let avail = ui.available_size();
        if state.fit_width {
            let max_w = state.layout.pages.iter().map(|p| p.w).fold(595.0f32, f32::max);
            let target = ((avail.x - PAGE_PADDING * 2.0).max(120.0)) / (max_w * PT_TO_PX);
            state.zoom = target.clamp(0.1, 5.0);
        }
        let pt_to_ui = PT_TO_PX * state.zoom;
        let ppp = ctx.pixels_per_point();
        let target_scroll_page = state.pending_scroll_page.take();
        let target_scroll_pos = state.pending_scroll_pos.take().and_then(|pos| state.layout.caret(&pos));
        let page_count = state.layout.pages.len();

        ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let scroll_clip = ui.clip_rect();
            let center_y = scroll_clip.center().y;
            let content_w = avail.x.max(page_max_ui_width(state, pt_to_ui) + PAGE_PADDING * 2.0);
            ui.set_min_width(content_w);

            ui.add_space(PAGE_GAP);
            for page_idx in 0..page_count {
                let page_w = state.layout.pages[page_idx].w * pt_to_ui;
                let page_h = state.layout.pages[page_idx].h * pt_to_ui;
                let x_off = ((content_w - page_w) * 0.5).max(PAGE_PADDING);

                let (row_rect, _) = ui.allocate_exact_size(Vec2::new(content_w, page_h), egui::Sense::hover());
                let page_rect = Rect::from_min_size(Pos2::new(row_rect.min.x + x_off, row_rect.min.y), Vec2::new(page_w, page_h));

                if target_scroll_page == Some(page_idx as u32) {
                    ui.scroll_to_rect(page_rect.expand(PAGE_GAP), Some(egui::Align::TOP));
                }
                if let Some(ref caret) = target_scroll_pos
                    && caret.page == page_idx
                {
                    let caret_rect = Rect::from_min_size(
                        Pos2::new(page_rect.min.x + caret.x * pt_to_ui, page_rect.min.y + caret.top * pt_to_ui),
                        Vec2::new(4.0, (caret.height * pt_to_ui).max(16.0)),
                    );
                    ui.scroll_to_rect(caret_rect.expand(48.0), Some(egui::Align::Center));
                }
                if page_rect.top() <= center_y && page_rect.bottom() >= center_y {
                    state.current_page = (page_idx + 1) as u32;
                }

                let visible = page_rect.intersects(scroll_clip.expand(page_h * 0.5 + 120.0));
                if visible {
                    paint_page_card(ui, page_rect, dark);
                    let raw_scale = pt_to_ui * ppp;
                    let side_cap = MAX_RENDER_SIDE / state.layout.pages[page_idx].w.max(state.layout.pages[page_idx].h).max(1.0);
                    let render_scale = raw_scale.min(side_cap).max(0.25);
                    let tex = self.page_texture(ctx, state, page_idx, render_scale, dark);
                    ui.painter().image(tex, page_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    paint_page_overlays(ui, state, page_idx, page_rect, pt_to_ui);
                    self.handle_page_input(ctx, ui, state, page_idx, page_rect, pt_to_ui);
                }

                if page_idx + 1 < page_count {
                    ui.add_space(PAGE_GAP);
                }
            }
            ui.add_space(PAGE_GAP);
        });
    }

    fn page_texture(&mut self, ctx: &Context, state: &PreviewDoc, page_idx: usize, render_scale: f32, dark: bool) -> egui::TextureId {
        let key = TextureKey { doc_rev: state.doc_rev, page_idx, scale_pct: (render_scale * 100.0).round() as u32, dark };
        if let Some((used, handle)) = self.textures.get_mut(&key) {
            *used = self.tick;
            return handle.id();
        }
        if self.textures.len() >= MAX_CACHED_TEXTURES
            && let Some(oldest_key) = self.textures.iter().min_by_key(|(_, (t, _))| *t).map(|(k, _)| *k)
        {
            self.textures.remove(&oldest_key);
        }
        let opts = RenderOptions { dark, ..Default::default() };
        let rendered = render_page(&state.doc, &state.layout.pages[page_idx], render_scale, &opts);
        let rgba = rendered.to_straight();
        let image = ColorImage::from_rgba_unmultiplied([rendered.width as usize, rendered.height as usize], &rgba);
        let name = format!("wc_page_{}_{}_{}", key.doc_rev, page_idx, key.scale_pct);
        let handle = ctx.load_texture(name, image, TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key, (self.tick, handle));
        id
    }

    fn handle_page_input(&mut self, ctx: &Context, ui: &mut Ui, state: &mut PreviewDoc, page_idx: usize, page_rect: Rect, pt_to_ui: f32) {
        let resp = ui.interact(page_rect, ui.scope_id().with(("page", page_idx)), egui::Sense::click_and_drag());
        let resp = resp.on_hover_cursor(CursorIcon::Text);

        let hit_at = |screen_pos: Pos2| -> Option<Pos> {
            let px = (screen_pos.x - page_rect.min.x) / pt_to_ui;
            let py = (screen_pos.y - page_rect.min.y) / pt_to_ui;
            let story = state.layout.story_at(page_idx, px, py).unwrap_or(StoryRef::Body);
            state.layout.hit(page_idx, px, py, story)
        };

        if resp.triple_clicked()
            && let Some(pos) = resp.interact_pointer_pos().and_then(hit_at)
        {
            state.select_paragraph_at(&pos);
            self.drag_anchor = None;
        } else if resp.double_clicked()
            && let Some(pos) = resp.interact_pointer_pos().and_then(hit_at)
        {
            state.select_word_at(&pos);
            self.drag_anchor = None;
        } else if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos().and_then(hit_at)
        {
            self.drag_anchor = Some(pos.clone());
            state.selection = Some((pos.clone(), pos));
        } else if resp.dragged()
            && let Some(anchor) = self.drag_anchor.clone()
            && let Some(focus) = resp.interact_pointer_pos().and_then(hit_at)
        {
            state.selection = Some((anchor, focus));
        } else if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos().and_then(hit_at)
        {
            state.selection = Some((pos.clone(), pos));
            self.drag_anchor = None;
        }

        let is_zh = matches!(state.lang, UiLang::ZhHans);
        let copy_label = if is_zh { "Copy / 复制" } else { "Copy" };
        let select_all_label = if is_zh { "Select All / 全选" } else { "Select All" };
        resp.context_menu(|ui| {
            let selected = state.selected_text();
            if ui.add_enabled(!selected.is_empty(), egui::Button::new(copy_label)).clicked() {
                ctx.copy_text(selected);
                ui.close();
            }
            if ui.button(select_all_label).clicked() {
                state.select_all();
                ui.close();
            }
        });
    }
}

fn page_max_ui_width(state: &PreviewDoc, pt_to_ui: f32) -> f32 {
    state.layout.pages.iter().map(|p| p.w * pt_to_ui).fold(0.0f32, f32::max)
}

fn paint_page_card(ui: &Ui, page_rect: Rect, dark: bool) {
    let shadow = Rect::from_min_max(page_rect.min + Vec2::new(0.0, 2.0), page_rect.max + Vec2::new(2.0, 4.0));
    ui.painter().rect_filled(shadow, 2.0, Color32::from_black_alpha(if dark { 70 } else { 28 }));
    let card_bg = if dark { Color32::from_rgb(0x33, 0x33, 0x33) } else { Color32::WHITE };
    ui.painter().rect_filled(page_rect, 0.0, card_bg);
}

struct PageDraw {
    idx: usize,
    rect: Rect,
    pt_to_ui: f32,
}

fn paint_page_overlays(ui: &Ui, state: &PreviewDoc, page_idx: usize, page_rect: Rect, pt_to_ui: f32) {
    let painter = ui.painter();
    let page = PageDraw { idx: page_idx, rect: page_rect, pt_to_ui };
    let active_idx = state.find_index;
    for (idx, (a, b)) in state.find_matches.iter().enumerate() {
        let color =
            if idx == active_idx { Color32::from_rgba_unmultiplied(255, 150, 50, 120) } else { Color32::from_rgba_unmultiplied(255, 235, 59, 100) };
        paint_range(painter, state, a, b, &page, color);
    }
    if let Some((a, b)) = state.ordered_selection()
        && a != b
    {
        paint_range(painter, state, &a, &b, &page, Color32::from_rgba_unmultiplied(51, 132, 240, 85));
    }
}

fn paint_range(painter: &egui::Painter, state: &PreviewDoc, a: &Pos, b: &Pos, page: &PageDraw, color: Color32) {
    for (pi, rect) in state.layout.selection_rects(&state.doc, a, b, page.idx) {
        if pi != page.idx {
            continue;
        }
        let ui_rect = Rect::from_min_size(
            Pos2::new(page.rect.min.x + rect.x * page.pt_to_ui, page.rect.min.y + rect.y * page.pt_to_ui),
            Vec2::new((rect.w * page.pt_to_ui).max(2.0), (rect.h * page.pt_to_ui).max(2.0)),
        );
        painter.rect_filled(ui_rect, 0.0, color);
    }
}

fn paint_empty_state(ui: &mut Ui, state: &PreviewDoc, dark: bool) {
    let color = if dark { Color32::from_gray(170) } else { Color32::from_gray(110) };
    let msg = if let Some(ref err) = state.last_error { err.as_str() } else { "Drop a document here or open a file to preview" };
    ui.centered_and_justified(|ui| {
        ui.label(egui::RichText::new(msg).color(color).size(15.0));
    });
}

fn handle_shortcuts(ctx: &Context, state: &mut PreviewDoc) {
    let (cmd_c, cmd_a, cmd_plus, cmd_minus, cmd_zero, zoom_delta) = ctx.input(|i| {
        let cmd = i.modifiers.command || i.modifiers.ctrl;
        let plus = cmd && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals));
        let minus = cmd && i.key_pressed(Key::Minus);
        let zero = cmd && i.key_pressed(Key::Num0);
        let zd = if cmd && (i.zoom_delta() - 1.0).abs() > 0.001 { Some(i.zoom_delta()) } else { None };
        (cmd && i.key_pressed(Key::C), cmd && i.key_pressed(Key::A), plus, minus, zero, zd)
    });
    if cmd_c {
        let text = state.selected_text();
        if !text.is_empty() {
            ctx.copy_text(text);
        }
    }
    if cmd_a && state.loaded {
        state.select_all();
    }
    if cmd_plus {
        state.zoom_in();
    }
    if cmd_minus {
        state.zoom_out();
    }
    if cmd_zero {
        state.reset_zoom();
    }
    if let Some(zd) = zoom_delta {
        state.fit_width = false;
        state.zoom = (state.zoom * zd).clamp(0.1, 5.0);
    }
}

fn apply_theme(ctx: &Context, theme: ThemeMode) -> bool {
    let dark = match theme {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::Auto => browser_prefers_dark(),
    };
    if ctx.global_style().visuals.dark_mode != dark {
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    }
    dark
}

fn browser_prefers_dark() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(Some(mql)) = window.match_media("(prefers-color-scheme: dark)") else {
        return false;
    };
    mql.matches()
}
