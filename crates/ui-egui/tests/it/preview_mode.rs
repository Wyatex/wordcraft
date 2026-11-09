//! Read-only preview mode UI tests: hiding the top ribbon and keeping edits blocked while
//! find and selection remain active.

use egui_kittest::kittest::{By, NodeT, Queryable};
use serde_json::json;
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;
use wordcraft_ui_egui::{Services, WordApp};

fn build_app(text: &str, show_ribbon: bool, read_only: bool) -> egui_kittest::Harness<'static, WordApp> {
    let mut session = Session::new(Document::from_text(text));
    session.set_read_only(read_only);
    let mut app = WordApp::new(session, Services::default());
    app.ui.language = "en".into();
    app.show_ribbon = show_ribbon;
    let mut h = crate::harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    for _ in 0..4 {
        crate::harness::frame(&mut h);
    }
    h
}

fn has_exact_label(h: &egui_kittest::Harness<'static, WordApp>, target: &str) -> bool {
    h.query_all(By::new().label_contains(target)).any(|n| n.accesskit_node().label().as_deref().map(str::trim) == Some(target))
}

#[test]
fn show_ribbon_false_hides_ribbon_tabs() {
    let hidden = build_app("Preview body", false, true);
    for tab in ["Home", "Insert", "Layout", "Help"] {
        assert!(!has_exact_label(&hidden, tab), "{tab} ribbon tab must be hidden when show_ribbon is false");
    }

    let shown = build_app("Preview body", true, false);
    assert!(has_exact_label(&shown, "Home"), "Home ribbon tab is visible when show_ribbon is true");
}

#[test]
fn read_only_preview_blocks_text_input_and_allows_find() {
    let mut h = build_app("hello preview world", false, true);
    h.state_mut().run("edit.find", json!({"text": "preview"})).expect("find works in read-only preview");
    assert!(!h.state().session.find.results.is_empty());
    assert!(h.state_mut().run("text.insert", json!({"text": "X"})).is_err());
    assert!(h.state_mut().run("view.markdownSourceText", json!({})).is_err());
    assert_eq!(h.state().session.doc.plain_text(StoryRef::Body), "hello preview world");
}
