//! Dialogs. Every dialog is a framework `StackDialog` on the window's `dialog_host`, opened by its name through
//! [`open`]; [`host`] drives them. Every dialog ends by running commands (or shows one's result), so agents get the
//! same result without the dialog.

use craft::ui::dialog::DialogId;
#[cfg(feature = "full")]
use craft::ui::patterns::form_dialogs::AboutTab;
use serde_json::Value;

use crate::WordApp;

#[cfg(feature = "full")]
mod bookmarks;
#[cfg(feature = "full")]
mod customize_keyboard;
#[cfg(feature = "full")]
mod enclose;
#[cfg(feature = "full")]
mod find;
#[cfg(feature = "full")]
mod find_replace;
#[cfg(feature = "full")]
mod font;
#[cfg(feature = "full")]
mod forms;
pub(crate) mod host;
#[cfg(feature = "full")]
mod link;
#[cfg(feature = "full")]
mod macros;
#[cfg(feature = "full")]
mod mailings;
#[cfg(feature = "full")]
pub(crate) mod merge_dialogs;
#[cfg(feature = "full")]
mod page_setup;
#[cfg(feature = "full")]
mod paragraph;
#[cfg(feature = "full")]
mod paste_special;
#[cfg(feature = "full")]
mod phonetic;
pub(crate) mod prompts;
#[cfg(feature = "full")]
mod search_commands;
#[cfg(feature = "full")]
mod simple_forms;
#[cfg(feature = "full")]
pub(crate) mod style_forms;
#[cfg(feature = "full")]
mod styles;

#[cfg(feature = "full")]
pub(crate) use find::{NOT_IN_SEARCH, SEARCH_SUGGESTIONS, find_in_navigation_pane};
#[cfg(feature = "full")]
pub use forms::{CustomPropertiesForm, TableForm, TableProperties};
#[cfg(test)]
pub(crate) use host::close_all;
#[cfg(feature = "full")]
#[cfg(test)]
pub(crate) use mailings::{FilterRow, RecipientDraft, RecipientFilter, RecipientListDialog, hold_rows, merge_order, visible_recipients};
#[cfg(feature = "full")]
pub(crate) use page_setup::PAPER_LABELS;

/// Opens the dialog `name` for a script, a button or a command, with `params`; the module of the name reads what it
/// needs of them. Returns the dialog's id, or `None` when no dialog opened (an unknown name, nothing to show, or a
/// pane such as Restrict Editing).
pub fn open(app: &mut WordApp, name: &str, params: &Value) -> Option<DialogId> {
    route(app, name, params, None)
}

/// The name table: the one place a dialog name picks its module. `parent` is the dialog the new one opens over.
#[cfg(not(feature = "full"))]
fn route(app: &mut WordApp, name: &str, params: &Value, parent: Option<DialogId>) -> Option<DialogId> {
    match name {
        "password" | "encryptPassword" | "discardRecovery" => prompts::open(app, name, params, parent),
        _ => None,
    }
}

/// The name table: the one place a dialog name picks its module. `parent` is the dialog the new one opens over.
#[cfg(feature = "full")]
fn route(app: &mut WordApp, name: &str, params: &Value, parent: Option<DialogId>) -> Option<DialogId> {
    match name {
        "about" => host::open_about(app, AboutTab::About),
        "contributors" => host::open_about(app, AboutTab::Contributors),
        "models" => host::open_about(app, AboutTab::Models),
        "languageData" => host::open_about(app, AboutTab::App(0)),
        // The Restrict Editing pane is the one name that is not a dialog.
        "restrictEditing" => {
            app.restrict.open = true;
            None
        }
        "fontDownload" => crate::font_download::open(app, name, params, parent),
        "font" => font::open(app, name, params, parent),
        // The font colour's More Colors… (the colour picker keeps its own answer route).
        "colors" => crate::colors::open(app, name, params),
        "phonetic" => phonetic::open(app, name, params, parent),
        "enclose" => enclose::open(app, name, params, parent),
        "defineBullet" => dialogs_bullet::open(app, name, params, parent),
        "paragraph" | "asianTypography" => paragraph::open(app, name, params, parent),
        "tabs" | "borders" | "pageBorders" | "shading" => dialogs_para::open(app, name, params, parent),
        "defineList" | "trackChangesOptions" => dialogs_lists::open(app, name, params, parent),
        "dateTime" | "columns" | "symbol" | "specialCharacters" | "field" | "insertSmartArt" | "changeChartType" | "insertChart" | "chartData"
        | "dropCap" => dialogs_insert::open(app, name, params, parent),
        "pageSetup" => page_setup::open(app, name, params, parent),
        "link" => link::open(app, name, params, parent),
        "bookmark" | "icons" => bookmarks::open(app, name, params, parent),
        "pasteSpecial" => paste_special::open(app, name, params, parent),
        "renameDocument" | "insertTable" | "watermark" | "fieldCode" | "editFieldCode" | "tableCellMargins" => {
            simple_forms::open(app, name, params, parent)
        }
        "wordCount" | "zoom" => dialogs_view::open(app, name, params, parent),
        "tableProperties" | "customProperties" => forms::open(app, name, params, parent),
        "lineNumbers" | "hyphenation" | "manualHyphenation" | "language" | "envelopes" | "labels" | "tableFormula" | "tableSort" | "splitCells"
        | "textToTable" | "tableToText" | "pictureSize" | "pageNumbers" => dialogs_layout::open(app, name, params, parent),
        "caption" | "captionNumbering" | "newLabel" | "crossReference" | "index" | "markEntry" | "toc" | "tocOptions" | "tocStyles"
        | "tableOfFigures" | "sourceManager" | "editSource" | "createSource" => dialogs_refs::open(app, name, params, parent),
        "compare" | "combine" | "autoCorrect" => dialogs_review::open(app, name, params, parent),
        "noteOptions" => dialogs_notes::open(app, name, params, parent),
        "customizeKeyboard" => customize_keyboard::open(app, name, params, parent),
        "customizeRibbon" | "customizeQuickAccess" => dialogs_ribbon::open(app, name, params, parent),
        "macros" => macros::open(app, name, params, parent),
        "controlProperties" => dialogs_controls::open(app, name, params, parent),
        // Search opens in the title bar's box when the window shows it; its module decides.
        "commands" => search_commands::open(app, name, params, parent),
        "find" | "replace" | "goto" => find_replace::open(app, name, params, parent),
        "password" | "encryptPassword" | "discardRecovery" => prompts::open(app, name, params, parent),
        "newStyle" | "modifyStyle" => style_forms::open(app, name, params, parent),
        "manageStyles" => styles::open(app, name, params, parent),
        "templates" | "organizer" => dialogs_templates::open(app, name, params, parent),
        "newTableStyle" | "modifyTableStyle" => dialogs_table_style::open(app, name, params, parent),
        "recipientList" | "newRecipientList" => mailings::open(app, name, params, parent),
        "insertMergeField" | "selectTable" | "findRecipient" | "mergeRecords" | "printRecords" | "ruleIf" | "ruleSkipIf" | "matchFields"
        | "checkErrors" => merge_dialogs::open(app, name, params, parent),
        _ => None,
    }
}

#[cfg(feature = "full")]
pub mod dialogs_bullet;
#[cfg(feature = "full")]
pub mod dialogs_chart;
#[cfg(feature = "full")]
pub mod dialogs_controls;
#[cfg(feature = "full")]
pub mod dialogs_insert;
#[cfg(feature = "full")]
pub mod dialogs_layout;
#[cfg(feature = "full")]
pub mod dialogs_lists;
#[cfg(feature = "full")]
pub mod dialogs_notes;
#[cfg(feature = "full")]
pub mod dialogs_para;
#[cfg(feature = "full")]
pub mod dialogs_refs;
#[cfg(feature = "full")]
pub mod dialogs_review;
#[cfg(feature = "full")]
pub mod dialogs_ribbon;
#[cfg(feature = "full")]
pub mod dialogs_smart_art;
#[cfg(feature = "full")]
pub mod dialogs_table_style;
#[cfg(feature = "full")]
pub mod dialogs_templates;
#[cfg(feature = "full")]
mod dialogs_view;
#[cfg(feature = "full")]
#[cfg(test)]
mod tests;
