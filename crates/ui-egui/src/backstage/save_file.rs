//! The compact Save dialog, as Word 365's "Save this file": what Ctrl+S or the Quick Access
//! Toolbar's Save shows for a document that was never saved. A file name (its `.docx` shown
//! greyed inside the field), a location picked from the recent folders, Documents and Desktop
//! (Browse… opens the system folder picker), and Save, which writes `<folder>/<name>.docx`.
//! More options… goes to File › Save As for the other file types and folders.
//!
//! The browser has no folders: there Save keeps downloading the document as before.

use std::path::{Path, PathBuf};

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde::Serialize;
use serde_json::json;

use super::save_as::{self, file_name, folder_key, folder_name, folder_rows};
use crate::theme::{Tokens, fit_text, medium, regular};
use crate::{WordApp, icons};

/// The type the compact dialog saves: Word's own.
const EXT: &str = "docx";
/// The dialog's content width.
const WIDTH: f32 = 440.0;
/// Most recent folders the location list offers before Documents and Desktop.
const RECENT_SHOWN: usize = 8;

/// The dialog's state between frames.
#[derive(Clone, Debug, Serialize)]
pub struct SaveFileForm {
    /// The file name without its extension.
    pub name: String,
    /// The folder it saves in.
    pub folder: PathBuf,
    /// What went wrong with the last Save, or the name typed.
    pub message: String,
    /// A file that is there already: the next Save replaces it.
    #[serde(skip)]
    replace: Option<PathBuf>,
    /// The name field still has to take the keyboard, with its text selected.
    #[serde(skip)]
    focus: bool,
    #[serde(skip)]
    home: Option<PathBuf>,
    /// Documents and Desktop, the ones that exist.
    #[serde(skip)]
    standard: Vec<(&'static str, PathBuf)>,
}

impl SaveFileForm {
    /// The form for the document shown, or `None` without a folder to save in (no home folder):
    /// the system Save As dialog asks instead.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn read(app: &mut WordApp) -> Option<SaveFileForm> {
        // A folder picked for a dialog that has since closed doesn't belong to this one.
        app.dialog_pick = None;
        let home = save_as::home();
        let standard: Vec<(&'static str, PathBuf)> =
            save_as::standard_folders(home.as_deref()).into_iter().filter(|(n, _)| *n != "Downloads").collect();
        let folder = save_as::default_folder_in(app, &standard, home.as_deref())?;
        let suggested = app.default_save_name();
        let name = match suggested.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => suggested,
        };
        Some(SaveFileForm { name, folder, message: String::new(), replace: None, focus: true, home, standard })
    }
}

impl WordApp {
    /// Ctrl+S or Save on a document that was never saved: the compact Save dialog. False when it
    /// can't be shown (the browser, another dialog open, no folder to offer); the caller then asks
    /// in the system Save As dialog as before.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_save_file(&mut self) -> bool {
        if self.services.download.is_some() || self.dialog.is_some() || self.file_dialog_open() {
            return false;
        }
        let Some(form) = SaveFileForm::read(self) else { return false };
        self.dialog = Some(crate::dialogs::Dialog::SaveFile { form: Box::new(form) });
        true
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn open_save_file(&mut self) -> bool {
        false
    }
}

/// The dialog's body; true to close.
pub(crate) fn body(app: &mut WordApp, ui: &mut Ui, f: &mut SaveFileForm) -> bool {
    let t = Tokens::get(ui.ctx());
    if let Some(p) = app.dialog_pick.take().filter(|p| !p.trim().is_empty()) {
        f.folder = PathBuf::from(p);
        f.replace = None;
        f.message.clear();
    }
    ui.set_width(WIDTH);
    ui.add_space(4.0);
    let mut save = name_field(ui, f, &t);
    ui.add_space(12.0);
    let label = ui.label(egui::RichText::new(tl!("Choose a location")).font(regular(12.5)).color(t.text_dim));
    location(app, ui, f, &t, label.id);
    if !f.message.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(f.message.as_str()).color(ui.visuals().error_fg_color));
    }
    if let Some(path) = &f.replace {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let text = crate::i18n::fmt(tl!("{name} already exists. Choose Save again to replace it."), &[("name", &crate::i18n::isolate(&name))]);
        ui.add_space(6.0);
        ui.label(egui::RichText::new(text).color(ui.visuals().warn_fg_color));
    }
    ui.add_space(14.0);
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, t.border));
    ui.add_space(10.0);
    let mut cancel = false;
    let mut more = false;
    ui.horizontal(|ui| {
        // More options…: a link in the app colour.
        let link = egui::Button::new(egui::RichText::new(tl!("More options…")).color(t.accent_text)).frame(false);
        let link = ui.add(link).on_hover_cursor(egui::CursorIcon::PointingHand);
        if link.hovered() {
            let r = link.rect;
            ui.painter().hline(r.x_range().shrink(4.0), r.bottom() - 3.0, Stroke::new(1.0, t.accent_text));
        }
        more = link.clicked();
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            // Both buttons as wide as the wider one (the shared push button spec).
            ui.spacing_mut().item_spacing.x = crate::widgets::PUSH_GAP;
            let w = crate::widgets::push_width(ui, &[tl!("Save"), tl!("Cancel")]);
            let save_button = |ui: &mut Ui| ui.add(crate::widgets::primary_button(tl!("Save")).min_width(w));
            // Cancel: no fill, a neutral grey outline. In the platform's order (see
            // `ok_cancel_responses`): macOS puts Cancel left of Save.
            let cancel_button = |ui: &mut Ui| ui.add(crate::widgets::neutral_button(tl!("Cancel")).min_width(w));
            let (ok, cancelb) = if cfg!(target_os = "macos") {
                let ok = save_button(ui);
                (ok, cancel_button(ui))
            } else {
                let c = cancel_button(ui);
                (save_button(ui), c)
            };
            save |= ok.clicked();
            cancel |= cancelb.clicked();
        });
    });
    // Escape closes an open location list first, not the dialog.
    if crate::dialogs::cancel_key_pressed(ui) && !egui::Popup::is_any_open(ui.ctx()) {
        cancel = true;
    }
    // Enter saves when nothing else takes it (a button or the location box would).
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && ui.memory(|m| m.focused().is_none()) && !egui::Popup::is_any_open(ui.ctx()) {
        save = true;
    }
    if cancel {
        return true;
    }
    if more {
        // File › Save As, with the name and folder chosen here.
        save_as::seed_page(app, ui.ctx(), &f.name, Some(f.folder.clone()));
        app.ui.backstage = true;
        app.ui.backstage_page = "saveAs".into();
        return true;
    }
    save && write(app, f)
}

/// The file name field, its text selected when the dialog opens and `.docx` shown greyed at its
/// far end. True when Enter was pressed in it.
fn name_field(ui: &mut Ui, f: &mut SaveFileForm, t: &Tokens) -> bool {
    let label = ui.label(egui::RichText::new(tl!("File name")).font(regular(12.5)).color(t.text_dim));
    let font = regular(14.0);
    let suffix = format!(".{EXT}");
    let suffix_w = ui.painter().layout_no_wrap(suffix.clone(), font.clone(), t.text_dim).size().x;
    let rtl = crate::rtl::on();
    // Room kept at the far end for the extension, inside the frame.
    let room = (suffix_w + 14.0).clamp(0.0, 120.0) as i8;
    let margin =
        if rtl { egui::Margin { left: room, right: 8, top: 6, bottom: 6 } } else { egui::Margin { left: 8, right: room, top: 6, bottom: 6 } };
    let mut out = egui::TextEdit::singleline(&mut f.name).font(font.clone()).margin(margin).desired_width(WIDTH).show(ui);
    let field = egui::Response::clone(&out.response).labelled_by(label.id);
    if f.focus {
        f.focus = false;
        field.request_focus();
        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(f.name.chars().count()));
        out.state.cursor.set_char_range(Some(all));
        out.state.store(ui.ctx(), field.id);
    }
    if field.changed() {
        f.replace = None;
        f.message.clear();
    }
    let r = field.rect;
    let at = pos2(crate::rtl::trail_x(rtl, r, 8.0), r.center().y);
    let anchor = if rtl { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
    ui.painter().text(at, anchor, suffix, font, t.text_dim);
    field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// The location box: the folder's icon, name and path; a click lists the other folders.
fn location(app: &mut WordApp, ui: &mut Ui, f: &mut SaveFileForm, t: &Tokens, label: egui::Id) {
    let home = f.home.as_deref().map(|h| h.to_string_lossy().into_owned());
    let (r, resp) = ui.allocate_exact_size(vec2(WIDTH, 50.0), Sense::click());
    let resp = resp.labelled_by(label);
    let shown = folder_name(&f.folder.to_string_lossy(), home.as_deref());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, shown.clone()));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let fill = if resp.hovered() || open { t.hover } else { t.input };
    ui.painter().rect_filled(r, 4.0, fill);
    let border = if resp.has_focus() || open { t.accent } else { t.input_border };
    ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    folder_face(ui, r.shrink2(vec2(0.0, 2.0)), &f.folder, home.as_deref(), t, 36.0);
    // The list's arrow at the far end.
    let rtl = crate::rtl::on();
    let c = pos2(crate::rtl::trail_x(rtl, r, 18.0), r.center().y);
    let s = Stroke::new(1.2, t.text);
    ui.painter().line_segment([c + vec2(-4.5, -2.0), c + vec2(0.0, 2.5)], s);
    ui.painter().line_segment([c + vec2(0.0, 2.5), c + vec2(4.5, -2.0)], s);
    let mut picked = None;
    let mut browse = false;
    crate::rtl::menu(&resp).show(|ui| {
        ui.set_min_width(WIDTH);
        for folder in choices(app, f) {
            let (row, item) = ui.allocate_exact_size(vec2(WIDTH, 44.0), Sense::click());
            let item_name = folder_name(&folder.to_string_lossy(), home.as_deref());
            item.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, true, item_name.clone()));
            let current = folder_key(&folder.to_string_lossy()) == folder_key(&f.folder.to_string_lossy());
            if item.hovered() || current {
                ui.painter().rect_filled(row, 3.0, if item.hovered() { t.hover } else { t.checked });
            }
            folder_face(ui, row, &folder, home.as_deref(), t, 12.0);
            if item.clicked() {
                picked = Some(folder);
                ui.close();
            }
        }
        ui.separator();
        if ui.add(egui::Button::new(tl!("Browse…")).frame(false).min_size(vec2(WIDTH, 30.0))).clicked() {
            browse = true;
            ui.close();
        }
    });
    if let Some(folder) = picked {
        f.folder = folder;
        f.replace = None;
        f.message.clear();
    }
    if browse {
        let dir = Some(f.folder.to_string_lossy().into_owned());
        let _ = app.ask_file(crate::file_dialogs::FileDialogRequest::Folder { dir }, crate::file_dialogs::AfterPick::ForDialog);
    }
}

/// A folder's icon, its name (medium) and under it its full path (smaller, dimmed), in `r`;
/// `trail` is the room kept at the far end.
fn folder_face(ui: &Ui, r: Rect, folder: &Path, home: Option<&str>, t: &Tokens, trail: f32) {
    let rtl = crate::rtl::on();
    let path = folder.to_string_lossy();
    let icon = Rect::from_center_size(pos2(crate::rtl::lead_x(rtl, r, 22.0), r.center().y), vec2(22.0, 22.0));
    icons::paint(ui.painter(), icon, "folder", t.icon, t.accent);
    let text_w = (r.width() - 44.0 - trail).max(0.0);
    let (name_font, path_font) = (medium(13.5), regular(11.5));
    let name = fit_text(ui.painter(), &crate::i18n::isolate(&folder_name(&path, home)), &name_font, 0.0, t.text, text_w);
    let full = fit_text(ui.painter(), &crate::i18n::isolate_path(&path), &path_font, 0.0, t.text_dim, text_w);
    let x = crate::rtl::lead_x(rtl, r, 44.0);
    let anchor = if rtl { Align2::RIGHT_BOTTOM } else { Align2::LEFT_BOTTOM };
    ui.painter().text(pos2(x, r.center().y + 1.0), anchor, name, name_font, t.text);
    let anchor = if rtl { Align2::RIGHT_TOP } else { Align2::LEFT_TOP };
    ui.painter().text(pos2(x, r.center().y + 2.0), anchor, full, path_font, t.text_dim);
}

/// The folders the location list offers: the recent ones (pinned first), then Documents and
/// Desktop, each once.
fn choices(app: &WordApp, f: &SaveFileForm) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let recent = folder_rows(&app.ui.recent_folders, &app.ui.recent, &app.ui.pinned_folders)
        .into_iter()
        .map(|r| PathBuf::from(r.path))
        .filter(|p| p.is_dir())
        .take(RECENT_SHOWN);
    let standard = f.standard.iter().map(|(_, p)| p.clone());
    std::iter::once(f.folder.clone()).chain(recent).chain(standard).filter(|p| seen.insert(folder_key(&p.to_string_lossy()))).collect()
}

/// Save as `<folder>/<name>.docx`, asking once before replacing a file. True once saved.
fn write(app: &mut WordApp, f: &mut SaveFileForm) -> bool {
    let name = match file_name(&f.name, EXT) {
        Ok(n) => n,
        Err(e) => {
            f.message = tl!(e).to_string();
            return false;
        }
    };
    let path = f.folder.join(&name);
    if path.exists() && f.replace.as_ref() != Some(&path) {
        f.replace = Some(path);
        f.message.clear();
        return false;
    }
    f.replace = None;
    match app.run("file.saveAs", json!({"path": path.to_string_lossy()})) {
        Ok(v) if v.get("saved").and_then(serde_json::Value::as_bool) == Some(true) => true,
        Ok(_) => false,
        Err(e) => {
            f.message = e;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::backstage::save_as::RecentFolder;
    use crate::dialogs::Dialog;
    use crate::{Services, WordApp};

    /// Ctrl+S on a new document opens the compact dialog in the most recent folder, with the
    /// suggested name; Enter writes `<folder>/<name>.docx` and closes it.
    #[test]
    fn save_on_a_new_document_asks_in_the_compact_dialog() {
        let dir = std::env::temp_dir().join(format!("deskwerk-docs-ui-save-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let doc = deskwerk_docs_doc::Document::from_text("Quarterly report");
        let mut a = WordApp::new(deskwerk_docs_engine::Session::new(doc), Services::default());
        a.ui.recent_folders = vec![RecentFolder { path: dir.to_string_lossy().into_owned(), used: 1 }];
        let r = a.run("file.save", json!({})).unwrap();
        assert_eq!(r["pending"], "saveFile", "{r}");
        assert!(!a.file_dialog_open(), "no system dialog");
        let Some(Dialog::SaveFile { form }) = &a.dialog else { panic!("not the Save dialog: {:?}", a.dialog) };
        assert_eq!(form.folder, dir);
        assert!(!form.name.is_empty() && !form.name.ends_with(".docx"), "{}", form.name);
        let path = dir.join(format!("{}.docx", form.name));

        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert!(matches!(h.state().dialog, Some(Dialog::SaveFile { .. })), "the dialog stays open");
        h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(4);
        assert!(h.state().dialog.is_none(), "Save closes the dialog");
        assert!(path.is_file(), "written as {}", path.display());
        assert_eq!(h.state().session.path.as_deref(), Some(path.as_path()));
        assert!(!h.state().session.dirty);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
