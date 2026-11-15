//! File › Save As, laid out as Word's: places on the left (Recent, This PC, Browse), and on the
//! right a quick-save row (file name, file type, Save) above the folders to save in. Recent lists
//! the folders documents were opened from or saved to, pinned ones first, then by date (Today,
//! Yesterday, This Week, Last Week, Older); This PC lists the document's own folder and the
//! standard ones (Documents, Desktop, Downloads). A click on a folder opens the system Save As
//! dialog there; Browse opens it where the quick-save row would save.
//!
//! The file type decides the format written: a Word template (.dotx) is one of the types, not a
//! button of its own, and choosing a template type saves into the user templates folder, which
//! File › New lists under Personal (#2852).
//!
//! In the browser there are no folders: the page is the quick-save row, and Save downloads.

use std::path::{Path, PathBuf};

use egui::{Rect, Sense, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::theme::{Tokens, medium, regular, semibold};
use crate::{WordApp, icons};

/// A folder a document was opened from or saved to, and when (Unix seconds), kept in `ui.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentFolder {
    pub path: String,
    pub used: i64,
}

/// Most folders kept in the Recent list's history, and most pinned.
pub const MAX_RECENT_FOLDERS: usize = 50;
/// Most folders Recent shows below the pinned ones.
const SHOWN_FOLDERS: usize = 30;

/// Formats the file type list offers: (extension, label). Word's own document first, as the
/// default; then what the app writes faithfully. The system dialog lists every format the
/// engine saves ([`save_filters`]).
pub const PAGE_FORMATS: [(&str, &str); 13] = [
    ("docx", "Word document (*.docx)"),
    ("docm", "Word Macro-Enabled Document (*.docm)"),
    ("dotx", "Word Template (*.dotx)"),
    ("dotm", "Word Macro-Enabled Template (*.dotm)"),
    ("pdf", "PDF document (*.pdf)"),
    ("xml", "Word XML Document (*.xml)"),
    ("odt", "OpenDocument Text (*.odt)"),
    ("ott", "OpenDocument Text Template (*.ott)"),
    ("fodt", "Flat OpenDocument Text (*.fodt)"),
    ("rtf", "Rich Text Format (*.rtf)"),
    ("html", "Web page (*.html)"),
    ("md", "Markdown (*.md)"),
    ("txt", "Plain text (*.txt)"),
];

/// The system Save As dialog's extra types, after [`PAGE_FORMATS`]: every extension the engine
/// saves is in one of the two lists.
const DIALOG_ONLY_FORMATS: [(&str, &str); 3] = [("tex", "LaTeX (*.tex)"), ("png", "Page image (*.png)"), ("json", "JSON (*.json)")];

/// The label of a save format's extension, in English (the catalog key).
pub fn format_label(ext: &str) -> Option<&'static str> {
    PAGE_FORMATS.iter().chain(DIALOG_ONLY_FORMATS.iter()).find(|(e, _)| *e == ext).map(|(_, l)| *l)
}

/// Is `ext` a Word template type, which saves into the user templates folder?
pub fn is_template(ext: &str) -> bool {
    matches!(ext, "dotx" | "dotm")
}

/// The extensions the system Save As dialog offers, the suggested name's own first (rfd makes a
/// filter's first extension the dialog's default, which the panels append to a name typed
/// without one; with no filter the name came back bare and the save failed, #689), then Word's
/// document and the rest in [`PAGE_FORMATS`] order.
pub fn save_exts(name: &str) -> Vec<String> {
    let all = || PAGE_FORMATS.iter().chain(DIALOG_ONLY_FORMATS.iter()).map(|(e, _)| *e).filter(|e| wordcraft_engine::io::SAVE_EXTS.contains(e));
    let suggested = Path::new(name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .filter(|e| wordcraft_engine::io::SAVE_EXTS.contains(&e.as_str()))
        .unwrap_or_else(|| "docx".to_string());
    let mut exts = vec![suggested.clone()];
    exts.extend(all().filter(|e| *e != suggested).map(str::to_string));
    // A format the engine saves that has no label here still gets an entry.
    let unlabelled: Vec<String> =
        wordcraft_engine::io::SAVE_EXTS.iter().filter(|e| !exts.iter().any(|x| x == *e)).map(|e| (*e).to_string()).collect();
    exts.extend(unlabelled);
    exts
}

/// The system Save As dialog's "Save as type" list (#1399): one named, translated entry per
/// format, the suggested name's own first ([`save_exts`]). rfd doesn't say which entry was
/// picked, so the extension in the name decides the format.
pub fn save_filters(name: &str) -> Vec<(String, Vec<String>)> {
    save_exts(name)
        .into_iter()
        .map(|e| {
            let label = format_label(&e).map_or_else(|| format!("{} (*.{e})", e.to_ascii_uppercase()), |l| tl!(l).to_string());
            (label, vec![e])
        })
        .collect()
}

/// Put the folder of `file` first in `list`, used `now`. A file without a folder (a name in the
/// browser) changes nothing.
pub fn note_folder(list: &mut Vec<RecentFolder>, file: &str, now: i64) {
    let Some(folder) = Path::new(file).parent().map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.trim().is_empty()) else {
        return;
    };
    let key = folder_key(&folder);
    list.retain(|f| folder_key(&f.path) != key);
    list.insert(0, RecentFolder { path: folder, used: now });
    list.truncate(MAX_RECENT_FOLDERS);
}

/// Two spellings of the same folder compare equal: trailing separators, and on Windows case and
/// slashes, don't count.
pub(crate) fn folder_key(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let p = if trimmed.is_empty() { path } else { trimmed };
    if cfg!(windows) { p.replace('/', "\\").to_lowercase() } else { p.to_string() }
}

/// A folder row on the Recent list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FolderRow {
    pub path: String,
    /// When it was last used, if known (folders of older Recent documents aren't dated).
    pub used: Option<i64>,
    pub pinned: bool,
}

/// The Recent list: the pinned folders in their order, then the folders used, newest first,
/// then the folders of the recent documents with no date, each folder once.
pub(crate) fn folder_rows(used: &[RecentFolder], recent_docs: &[String], pinned: &[String]) -> Vec<FolderRow> {
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    let when = |key: &str| used.iter().find(|f| folder_key(&f.path) == key).map(|f| f.used);
    for p in pinned.iter().take(MAX_RECENT_FOLDERS) {
        let key = folder_key(p);
        if !p.trim().is_empty() && seen.insert(key.clone()) {
            rows.push(FolderRow { path: p.clone(), used: when(&key), pinned: true });
        }
    }
    let mut dated: Vec<&RecentFolder> = used.iter().take(MAX_RECENT_FOLDERS).collect();
    // Stable: equal times keep the list's own order.
    dated.sort_by_key(|f| std::cmp::Reverse(f.used));
    let undated = recent_docs.iter().take(crate::MAX_RECENT).filter_map(|d| Path::new(d).parent().map(|p| p.to_string_lossy().into_owned()));
    let candidates = dated.into_iter().map(|f| (f.path.clone(), Some(f.used))).chain(undated.map(|p| (p, None)));
    let mut shown = 0;
    for (path, used) in candidates {
        if shown >= SHOWN_FOLDERS {
            break;
        }
        if path.trim().is_empty() || !seen.insert(folder_key(&path)) {
            continue;
        }
        rows.push(FolderRow { path, used, pinned: false });
        shown += 1;
    }
    rows
}

/// Recent's date groups, as Word's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DateGroup {
    Pinned,
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    Older,
}

impl DateGroup {
    fn label(self) -> &'static str {
        match self {
            DateGroup::Pinned => "Pinned",
            DateGroup::Today => "Today",
            DateGroup::Yesterday => "Yesterday",
            DateGroup::ThisWeek => "This Week",
            DateGroup::LastWeek => "Last Week",
            DateGroup::Older => "Older",
        }
    }
}

/// The group of a folder used on local day `day` (days since 1970-01-01), seen on local day
/// `today`. Weeks start on Monday; a day in the future (a clock put back) is Today; an undated
/// folder is Older.
pub(crate) fn date_group(day: Option<i64>, today: i64) -> DateGroup {
    let Some(day) = day else { return DateGroup::Older };
    let ago = today.saturating_sub(day);
    // 1970-01-01 was a Thursday: Monday is 0.
    let week_start = today.saturating_sub(today.saturating_add(3).rem_euclid(7));
    if ago <= 0 {
        DateGroup::Today
    } else if ago == 1 {
        DateGroup::Yesterday
    } else if day >= week_start {
        DateGroup::ThisWeek
    } else if day >= week_start.saturating_sub(7) {
        DateGroup::LastWeek
    } else {
        DateGroup::Older
    }
}

/// The local day (days since 1970-01-01) of Unix time `secs`.
fn local_day(secs: i64) -> i64 {
    let unix = u64::try_from(secs.max(0)).unwrap_or(0);
    wordcraft_engine::cmd::local_civil(unix, wordcraft_engine::cmd::local_offset_secs(unix)).4
}

/// A folder's date on its row: the time today and yesterday, the date before.
fn shown_date(secs: i64, group: DateGroup) -> String {
    let unix = u64::try_from(secs.max(0)).unwrap_or(0);
    let (y, m, d, (h, min, _), _) = wordcraft_engine::cmd::local_civil(unix, wordcraft_engine::cmd::local_offset_secs(unix));
    match group {
        DateGroup::Today | DateGroup::Yesterday => format!("{h:02}:{min:02}"),
        _ => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// The folders whose names are shown translated when they are the home folder's own.
const KNOWN_FOLDERS: [&str; 3] = ["Documents", "Desktop", "Downloads"];

fn segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|s| !s.is_empty()).collect()
}

/// A folder's path as Word shows it: its folders joined with » from the home folder's own
/// (Documents, Desktop…, translated) or the home folder itself, the middle shortened when long.
pub(crate) fn breadcrumb(folder: &str, home: Option<&str>) -> String {
    let all = segments(folder);
    let home_parts = home.map(segments).unwrap_or_default();
    let mut parts: Vec<String> = if !home_parts.is_empty() && all.len() >= home_parts.len() && same_folders(&all[..home_parts.len()], &home_parts) {
        let rest = &all[home_parts.len()..];
        match rest.first() {
            Some(first) if KNOWN_FOLDERS.contains(first) => {
                std::iter::once(tl!(first).to_string()).chain(rest.iter().skip(1).map(|s| (*s).to_string())).collect()
            }
            _ => home_parts.last().into_iter().chain(rest.iter()).map(|s| (*s).to_string()).collect(),
        }
    } else {
        all.iter().map(|s| (*s).to_string()).collect()
    };
    if parts.is_empty() {
        return folder.to_string();
    }
    if parts.len() > 4 {
        let tail = parts.split_off(parts.len() - 2);
        parts.truncate(1);
        parts.push("…".into());
        parts.extend(tail);
    }
    parts.join(" » ")
}

fn same_folders(a: &[&str], b: &[&str]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| if cfg!(windows) { x.eq_ignore_ascii_case(y) } else { x == y })
}

/// A folder's name on its row: its last folder, translated when it is one of the home folder's
/// own; the path itself for a drive or the root.
pub(crate) fn folder_name(folder: &str, home: Option<&str>) -> String {
    let all = segments(folder);
    let Some(last) = all.last() else { return folder.to_string() };
    let home_parts = home.map(segments).unwrap_or_default();
    if KNOWN_FOLDERS.contains(last) && all.len() == home_parts.len() + 1 && same_folders(&all[..home_parts.len()], &home_parts) {
        return tl!(last).to_string();
    }
    (*last).to_string()
}

/// The user's home folder.
pub(crate) fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { ["USERPROFILE", "HOME"] } else { ["HOME", "USERPROFILE"] };
    var.iter().find_map(std::env::var_os).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The standard folders This PC lists, as (English name, folder), the ones that exist.
pub(crate) fn standard_folders(home: Option<&Path>) -> Vec<(&'static str, PathBuf)> {
    let Some(home) = home else { return Vec::new() };
    #[cfg(all(unix, not(target_os = "macos")))]
    let xdg = xdg_user_dirs(home);
    let mut out = Vec::new();
    for (name, _xdg_key) in [("Documents", "XDG_DOCUMENTS_DIR"), ("Desktop", "XDG_DESKTOP_DIR"), ("Downloads", "XDG_DOWNLOAD_DIR")] {
        let mut candidates = Vec::new();
        #[cfg(all(unix, not(target_os = "macos")))]
        candidates.extend(xdg.iter().filter(|(k, _)| k == _xdg_key).map(|(_, p)| p.clone()));
        candidates.push(home.join(name));
        // Windows with OneDrive folder backup moves them there.
        if cfg!(windows) {
            candidates.push(home.join("OneDrive").join(name));
        }
        if let Some(found) = candidates.into_iter().find(|p| p.is_dir()) {
            out.push((name, found));
        }
    }
    out
}

/// Linux and the BSDs: the folders `user-dirs.dirs` names (`XDG_DOCUMENTS_DIR="$HOME/Docs"`),
/// which may be in the desktop's language.
#[cfg(all(unix, not(target_os = "macos")))]
fn xdg_user_dirs(home: &Path) -> Vec<(String, PathBuf)> {
    let config = std::env::var_os("XDG_CONFIG_HOME").filter(|c| !c.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let Ok(text) = std::fs::read_to_string(config.join("user-dirs.dirs")) else { return Vec::new() };
    parse_user_dirs(&text, home)
}

/// The `KEY="value"` lines of a `user-dirs.dirs` file, `$HOME` expanded; a value that is the
/// home folder itself means the folder is turned off.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
pub(crate) fn parse_user_dirs(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    text.lines()
        .take(200)
        .filter_map(|l| {
            let (k, v) = l.trim().split_once('=')?;
            if k.starts_with('#') {
                return None;
            }
            let v = v.trim().trim_matches('"');
            let path = match v.strip_prefix("$HOME") {
                Some(rest) => home.join(rest.trim_start_matches('/')),
                None if v.starts_with('/') => PathBuf::from(v),
                None => return None,
            };
            (path != home).then(|| (k.trim().to_string(), path))
        })
        .collect()
}

/// Where Save As is: the place on the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Recent,
    ThisPc,
}

/// The page's state between frames, for one document.
#[derive(Clone)]
struct PageState {
    /// The document and file it was made for: another one starts afresh.
    key: (u64, Option<PathBuf>),
    place: Place,
    /// The file name field, without the extension.
    name: String,
    ext: &'static str,
    /// The folder the quick-save row saves into, when chosen (the templates folder for a
    /// template type); otherwise [`default_folder`].
    folder: Option<PathBuf>,
    /// A file that is there already: the next Save replaces it.
    replace: Option<PathBuf>,
    /// A problem with the typed name.
    name_error: Option<&'static str>,
    standard: Vec<(&'static str, PathBuf)>,
    home: Option<PathBuf>,
}

fn state_id(app: &WordApp) -> egui::Id {
    app.window_id("backstage_save_as_page")
}

fn page_state(app: &WordApp, ctx: &egui::Context, web: bool) -> PageState {
    let key = (app.session.document_id(), app.session.path.clone());
    if let Some(s) = ctx.data(|d| d.get_temp::<PageState>(state_id(app))).filter(|s| s.key == key) {
        return s;
    }
    let suggested = app.default_save_name();
    let (stem, ext) = match suggested.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_ascii_lowercase()),
        _ => (suggested.clone(), "docx".into()),
    };
    let ext = PAGE_FORMATS.iter().map(|(e, _)| *e).find(|e| *e == ext).unwrap_or("docx");
    let home = if web { None } else { home() };
    let standard = if web { Vec::new() } else { standard_folders(home.as_deref()) };
    PageState { key, place: Place::Recent, name: stem, ext, folder: None, replace: None, name_error: None, standard, home }
}

/// The document's own folder, when it has one on disk.
fn document_folder(app: &WordApp) -> Option<PathBuf> {
    app.session.path.as_deref().filter(|p| p.is_absolute()).and_then(Path::parent).filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf)
}

/// Where the quick-save row saves when no folder was chosen: the document's folder, else the
/// most recent one, else Documents, else the home folder.
fn default_folder(app: &WordApp, st: &PageState) -> Option<PathBuf> {
    default_folder_in(app, &st.standard, st.home.as_deref())
}

/// [`default_folder`] given the standard folders and the home folder.
pub(crate) fn default_folder_in(app: &WordApp, standard: &[(&'static str, PathBuf)], home: Option<&Path>) -> Option<PathBuf> {
    document_folder(app)
        .or_else(|| app.ui.recent_folders.iter().max_by_key(|f| f.used).map(|f| PathBuf::from(&f.path)))
        .or_else(|| app.ui.recent.first().and_then(|d| Path::new(d).parent()).filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf))
        .or_else(|| standard.iter().find(|(n, _)| *n == "Documents").map(|(_, p)| p.clone()))
        .or_else(|| home.map(Path::to_path_buf))
}

/// Opens File › Save As on this name and folder (More options… in the compact Save dialog).
pub(crate) fn seed_page(app: &WordApp, ctx: &egui::Context, name: &str, folder: Option<PathBuf>) {
    let web = app.services.download.is_some();
    let mut st = page_state(app, ctx, web);
    st.name = name.to_string();
    st.ext = "docx";
    st.place = Place::ThisPc;
    st.folder = folder;
    st.replace = None;
    st.name_error = None;
    ctx.data_mut(|d| d.insert_temp(state_id(app), st));
}

/// The file name the quick-save row writes: the typed name with the type's extension (not
/// doubled when typed). `Err` names what's wrong with it.
pub(crate) fn file_name(typed: &str, ext: &str) -> Result<String, &'static str> {
    let name = typed.trim();
    if name.is_empty() {
        return Err("Type a file name.");
    }
    if name.contains(['/', '\\']) {
        return Err("A file name can't contain \\ or /.");
    }
    let has_ext = name.rsplit_once('.').is_some_and(|(stem, e)| !stem.is_empty() && e.eq_ignore_ascii_case(ext));
    Ok(if has_ext { name.to_string() } else { format!("{name}.{ext}") })
}

/// File › Save As.
pub(super) fn page(app: &mut WordApp, ui: &mut Ui) {
    super::heading(ui, "Save As");
    let error = ui.ctx().data(|d| d.get_temp::<String>(super::save_as_error_id())).unwrap_or_default();
    if !error.is_empty() {
        ui.label(egui::RichText::new(&error).color(ui.visuals().error_fg_color));
        ui.add_space(6.0);
    }
    let web = app.services.download.is_some();
    let mut st = page_state(app, ui.ctx(), web);
    if web {
        // No folders in the browser: name, type and Save, which downloads.
        quick_save_row(app, ui, &mut st, None);
    } else {
        crate::rtl::horizontal_top(ui, |ui| {
            crate::rtl::vertical(ui, |ui| {
                ui.set_width(210.0);
                places(app, ui, &mut st);
            });
            let (line, _) = ui.allocate_exact_size(vec2(25.0, 420.0), Sense::hover());
            let t = Tokens::get(ui.ctx());
            ui.painter().vline(line.center().x, line.y_range(), egui::Stroke::new(1.0, t.border));
            crate::rtl::vertical(ui, |ui| {
                ui.set_max_width(780.0);
                let target = st.folder.clone().or_else(|| default_folder(app, &st));
                quick_save_row(app, ui, &mut st, target);
                ui.add_space(18.0);
                match st.place {
                    Place::Recent => recent_list(app, ui, &st),
                    Place::ThisPc => this_pc_list(app, ui, &st),
                }
            });
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(state_id(app), st));
}

/// The places column: Recent, This PC, Browse.
fn places(app: &mut WordApp, ui: &mut Ui, st: &mut PageState) {
    let t = Tokens::get(ui.ctx());
    let rtl = crate::rtl::on();
    for (label, icon, place) in [("Recent", "clock", Some(Place::Recent)), ("This PC", "computer", Some(Place::ThisPc)), ("Browse", "open", None)] {
        let (r, resp) = ui.allocate_exact_size(vec2(210.0, 46.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tl!(label)));
        let active = place == Some(st.place);
        if active {
            ui.painter().rect_filled(r, 4.0, t.checked);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.hover);
        }
        icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(crate::rtl::lead_x(rtl, r, 24.0), r.center().y), vec2(22.0, 22.0)),
            icon,
            t.icon,
            t.accent,
        );
        let font = if active { semibold(14.0) } else { medium(14.0) };
        ui.painter().text(pos2(crate::rtl::lead_x(rtl, r, 50.0), r.center().y), crate::rtl::lead_center(rtl), tl!(label), font, t.text);
        if resp.clicked() {
            match place {
                Some(p) => st.place = p,
                None => {
                    let dir = st.folder.clone().or_else(|| default_folder(app, st));
                    browse(app, st, dir);
                }
            }
        }
        ui.add_space(2.0);
    }
}

/// The system Save As dialog in `dir`, with the quick-save row's name and type.
fn browse(app: &mut WordApp, st: &PageState, dir: Option<PathBuf>) {
    let name = file_name(&st.name, st.ext).unwrap_or_else(|_| app.default_save_name());
    app.save_as_in(dir.map(|d| d.to_string_lossy().into_owned()), Some(name));
}

/// File name, file type and Save; above them, where it saves.
fn quick_save_row(app: &mut WordApp, ui: &mut Ui, st: &mut PageState, target: Option<PathBuf>) {
    let t = Tokens::get(ui.ctx());
    if let Some(dir) = &target {
        let shown = breadcrumb(&dir.to_string_lossy(), st.home.as_deref().map(|h| h.to_string_lossy()).as_deref());
        crate::rtl::horizontal_top(ui, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            icons::paint(ui.painter(), r, "folder", t.icon, t.accent);
            ui.label(egui::RichText::new(crate::i18n::isolate(&shown)).font(regular(12.5)).color(t.text_dim))
                .on_hover_text(crate::i18n::isolate_path(&dir.to_string_lossy()));
        });
        ui.add_space(6.0);
    }
    let mut save = false;
    crate::rtl::horizontal_top(ui, |ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut st.name).hint_text(tl!("File name")).desired_width(300.0).font(regular(13.5)));
        if field.changed() {
            st.replace = None;
            st.name_error = None;
        }
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            save = true;
        }
        let before = st.ext;
        if file_type(ui, &mut st.ext) {
            st.replace = None;
            pick_type(app, st, before);
        }
        let button =
            egui::Button::new(egui::RichText::new(tl!("Save")).font(medium(13.5)).color(t.on_accent)).fill(t.accent).min_size(vec2(96.0, 0.0));
        if ui.add_enabled(!st.name.trim().is_empty(), button).clicked() {
            save = true;
        }
    });
    if let Some(e) = st.name_error {
        ui.label(egui::RichText::new(tl!(e)).color(ui.visuals().error_fg_color));
    }
    if let Some(path) = &st.replace {
        // The built-in file browser's wording: a second Save replaces it.
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let text = crate::i18n::fmt(tl!("{name} already exists. Choose Save again to replace it."), &[("name", &crate::i18n::isolate(&name))]);
        ui.label(egui::RichText::new(text).color(ui.visuals().warn_fg_color));
    }
    if save {
        quick_save(app, st, target);
    }
}

/// The file type list; true when another type was picked.
fn file_type(ui: &mut Ui, ext: &mut &'static str) -> bool {
    let shown = PAGE_FORMATS.iter().find(|(e, _)| *e == *ext).map_or("", |(_, l)| *l);
    let mut changed = false;
    let combo = craft_ui::ComboBox::from_id_salt("save_as_file_type").width(290.0).selected_text(tl!(shown)).show_ui(ui, |ui| {
        for (e, label) in PAGE_FORMATS {
            if ui.selectable_label(*ext == e, tl!(label)).clicked() && *ext != e {
                *ext = e;
                changed = true;
            }
        }
    });
    combo.response.accessible_name(tl!("File type"));
    changed
}

/// A new file type: a template type saves into the user templates folder (#2852), and leaving
/// the template types goes back to the usual folder.
fn pick_type(app: &WordApp, st: &mut PageState, before: &str) {
    if is_template(st.ext) && !is_template(before) {
        if let Some(dir) = app.user_templates_dir() {
            st.folder = Some(dir);
        }
    } else if !is_template(st.ext) && is_template(before) && st.folder.is_some() && st.folder == app.user_templates_dir() {
        st.folder = None;
    }
}

/// Save with the quick-save row's name and type into `dir` (the browser: download it).
fn quick_save(app: &mut WordApp, st: &mut PageState, dir: Option<PathBuf>) {
    let name = match file_name(&st.name, st.ext) {
        Ok(n) => n,
        Err(e) => {
            st.name_error = Some(e);
            return;
        }
    };
    st.name_error = None;
    if app.services.download.is_some() {
        write(app, st, &name);
        return;
    }
    // Without any folder (no home folder either) the system dialog asks where.
    let Some(dir) = dir else {
        browse(app, st, None);
        return;
    };
    // The app's own templates folder is made on first use.
    if !dir.is_dir()
        && Some(&dir) == app.user_templates_dir().as_ref()
        && let Err(e) = std::fs::create_dir_all(&dir)
    {
        log::warn!("couldn't make the templates folder {}: {e}", dir.display());
    }
    let path = dir.join(&name);
    if path.exists() && app.session.path.as_deref() != Some(path.as_path()) && st.replace.as_ref() != Some(&path) {
        st.replace = Some(path);
        return;
    }
    write(app, st, &path.to_string_lossy());
}

/// Save the document as `path`; once saved File closes, a failure says why on this page.
fn write(app: &mut WordApp, st: &mut PageState, path: &str) {
    st.replace = None;
    match app.run("file.saveAs", json!({"path": path})) {
        Ok(v) if v.get("saved").and_then(serde_json::Value::as_bool) == Some(true) || v.get("downloaded").is_some() => {
            app.ui.backstage = false;
            app.canvas.want_focus = true;
            if let Some(ctx) = app.ctx.clone() {
                super::forget_save_as_error(&ctx);
            }
        }
        Ok(_) => {}
        Err(e) => app.fail_save_as(&e),
    }
}

/// Recent: the pinned folders, then the folders used, by date.
fn recent_list(app: &mut WordApp, ui: &mut Ui, st: &PageState) {
    let t = Tokens::get(ui.ctx());
    let rows = folder_rows(&app.ui.recent_folders, &app.ui.recent, &app.ui.pinned_folders);
    if rows.is_empty() {
        ui.label(egui::RichText::new(tl!("Folders you open documents from or save them to show up here.")).color(t.text_dim));
        return;
    }
    let today = local_day(i64::try_from(wordcraft_engine::cmd::now_unix()).unwrap_or(i64::MAX));
    let mut last = None;
    for row in rows {
        let group = if row.pinned { DateGroup::Pinned } else { date_group(row.used.map(local_day), today) };
        if last != Some(group) {
            if last.is_some() {
                ui.add_space(10.0);
            }
            ui.label(egui::RichText::new(tl!(group.label())).font(semibold(13.5)).color(t.text));
            ui.add_space(4.0);
            last = Some(group);
        }
        let date = row.used.map(|u| shown_date(u, if row.pinned { date_group(Some(local_day(u)), today) } else { group }));
        folder_row(app, ui, st, &row.path, date.as_deref(), Some(row.pinned));
    }
}

/// This PC: the document's folder, then Documents, Desktop and Downloads.
fn this_pc_list(app: &mut WordApp, ui: &mut Ui, st: &PageState) {
    let t = Tokens::get(ui.ctx());
    let mut folders: Vec<String> = document_folder(app).map(|p| p.to_string_lossy().into_owned()).into_iter().collect();
    for (_, p) in &st.standard {
        let p = p.to_string_lossy().into_owned();
        if !folders.iter().any(|f| folder_key(f) == folder_key(&p)) {
            folders.push(p);
        }
    }
    if folders.is_empty() {
        ui.label(egui::RichText::new(tl!("Folders you open documents from or save them to show up here.")).color(t.text_dim));
        return;
    }
    ui.label(egui::RichText::new(tl!("Folders")).font(semibold(13.5)).color(t.text));
    ui.add_space(4.0);
    for f in folders {
        let pinned = app.ui.pinned_folders.iter().any(|p| folder_key(p) == folder_key(&f));
        folder_row(app, ui, st, &f, None, Some(pinned));
    }
}

/// One folder: icon, name, path and date; a click opens the system Save As dialog there. With
/// `pinned` the row has a pin to (un)pin it, shown on hover and while pinned.
fn folder_row(app: &mut WordApp, ui: &mut Ui, st: &PageState, path: &str, date: Option<&str>, pinned: Option<bool>) {
    let t = Tokens::get(ui.ctx());
    let rtl = crate::rtl::on();
    let home = st.home.as_deref().map(|h| h.to_string_lossy().into_owned());
    let name = folder_name(path, home.as_deref());
    let crumbs = breadcrumb(path, home.as_deref());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(780.0), 48.0), Sense::click());
    let resp = resp.on_hover_text(crate::i18n::isolate_path(path));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &name));
    let pin_rect = Rect::from_center_size(pos2(crate::rtl::trail_x(rtl, r, 22.0), r.center().y), vec2(26.0, 26.0));
    let pin = pinned.map(|on| {
        let id = ui.scope_id().with(("save_as_pin", path));
        let p = ui.interact(pin_rect, id, Sense::click());
        let label = if on { tl!("Unpin this folder") } else { tl!("Pin this folder") };
        p.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        (on, p.on_hover_text(label))
    });
    let hovered = ui.rect_contains_pointer(r);
    if hovered {
        ui.painter().rect_filled(r, 4.0, t.hover);
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(crate::rtl::lead_x(rtl, r, 20.0), r.center().y), vec2(24.0, 24.0)),
        "folder",
        t.icon,
        t.accent,
    );
    let side = crate::rtl::lead_center(rtl);
    let text_x = crate::rtl::lead_x(rtl, r, 46.0);
    let room = (r.width() - 46.0 - 170.0).max(80.0);
    let name_font = medium(13.5);
    let crumb_font = regular(11.5);
    let fitted_name = crate::theme::fit_text(ui.painter(), &crate::i18n::isolate(&name), &name_font, 0.0, t.text, room);
    let fitted_crumbs = crate::theme::fit_text(ui.painter(), &crate::i18n::isolate(&crumbs), &crumb_font, 0.0, t.text_dim, room);
    ui.painter().text(pos2(text_x, r.min.y + 15.0), side, fitted_name, name_font, t.text);
    ui.painter().text(pos2(text_x, r.min.y + 33.0), side, fitted_crumbs, crumb_font, t.text_dim);
    if let Some(date) = date {
        ui.painter().text(pos2(crate::rtl::trail_x(rtl, r, 48.0), r.center().y), crate::rtl::lead_center(!rtl), date, regular(12.0), t.text_dim);
    }
    if let Some((on, p)) = &pin {
        if *on || hovered {
            if p.hovered() {
                ui.painter().rect_filled(pin_rect, 3.0, t.pressed);
            }
            let ink = if *on { t.accent } else { t.icon };
            icons::paint(ui.painter(), pin_rect.shrink(4.0), "pin", ink, t.accent);
        }
        if p.clicked() {
            toggle_pin(&mut app.ui.pinned_folders, path);
            return;
        }
    }
    if resp.clicked() {
        browse(app, st, Some(PathBuf::from(path)));
    }
}

/// Pin `path` (last among the pinned) or unpin it.
pub(crate) fn toggle_pin(pinned: &mut Vec<String>, path: &str) {
    let key = folder_key(path);
    let before = pinned.len();
    pinned.retain(|p| folder_key(p) != key);
    if pinned.len() == before && pinned.len() < MAX_RECENT_FOLDERS {
        pinned.push(path.to_string());
    }
}
