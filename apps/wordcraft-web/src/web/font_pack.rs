//! The web font pack (`wordcraft_fonts::pack`), fetched from the editor's own origin: the files
//! sit in `webfonts/` next to `index.html` (Trunk copies `assets/webfonts` there), so no font is
//! ever fetched from a third-party server. A few Regular faces are fetched at start; every other
//! family only when a document's text or the user's pick asks for it.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use wasm_bindgen::JsCast as _;
use wasm_bindgen_futures::JsFuture;
use wordcraft_fonts::pack::{self, PackFile};

/// Fetches pack files, each at most once per visit.
#[derive(Clone, Default)]
pub(crate) struct Fetcher {
    started: Rc<RefCell<HashSet<&'static str>>>,
}

impl Fetcher {
    /// Turn the pack on and fetch its eager files ([`PackFile::eager`]) in the background.
    pub(crate) fn start(&self, ctx: &egui::Context) {
        pack::enable();
        self.fetch(pack::eager_files().collect(), ctx);
    }

    /// Fetch the families the layout asked for since the last frame ([`pack::take_wanted`]).
    pub(crate) fn poll(&self, ctx: &egui::Context) {
        for family in pack::take_wanted() {
            self.fetch(pack::files_of(family).collect(), ctx);
        }
    }

    /// Fetch `files` not fetched yet and add them in one change, so the document is laid out
    /// again once.
    fn fetch(&self, files: Vec<&'static PackFile>, ctx: &egui::Context) {
        let files: Vec<&'static PackFile> = files.into_iter().filter(|f| self.started.borrow_mut().insert(f.file)).collect();
        if files.is_empty() {
            return;
        }
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut got = Vec::new();
            for f in files {
                let url = format!("{}/{}", pack::WEB_DIR, f.file);
                match fetch_bytes(&url).await {
                    Ok(bytes) if bytes.len() as u64 == f.bytes => got.push((f, bytes)),
                    Ok(bytes) => log::warn!("font pack: {url} has {} bytes, not {}", bytes.len(), f.bytes),
                    Err(e) => log::warn!("font pack: {url}: {e}"),
                }
            }
            if !got.is_empty() {
                pack::add_files(got);
                ctx.request_repaint();
            }
        });
    }
}

/// GET `url` (relative to the page) and return its body.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let resp = JsFuture::from(window.fetch_with_str(url)).await.map_err(|e| format!("{e:?}"))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buffer = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
