//! Remote documents: a server (Deskwerk Server) opens a document of its own in Deskwerk Docs and
//! stores what Save writes, over plain HTTP. Unlike the host modes (`host`), the page that frames
//! the editor may live on another origin.
//!
//! - `?src=<url>` turns it on: the document is fetched with `GET src`, sent with
//!   `Authorization: Bearer <token>` (`&token=`), and opens as `&name=` (else the last part of
//!   `src`'s path) like a dropped file. No sample and no start screen meanwhile. Only `https:`
//!   URLs are taken, and `http:` to this machine (`localhost`) for development.
//! - `&save=<url>`: Save writes the document back with `PUT save` and `If-Match:` the `ETag` of
//!   the last load or save, so a file changed on the server since isn't overwritten. The save
//!   finishes later (an [`AsyncSaves`] ticket): until the server answers, the document's save isn't
//!   confirmed, and a failure (412: changed on the server, 423: the edit lock is gone) leaves it
//!   unsaved with the reason. Without `save` the document is read-only on the server: saves
//!   download as usual. Only a Save under the opened file's own name goes to the server; Save As
//!   another name, Export as PDF and Export Page as PNG download, so they never replace the
//!   document.
//! - `&origin=<origin>`: the framing page, told what happens with `postMessage` to that origin
//!   alone: `deskwerk:ready {app, version}`, `deskwerk:loaded`, `deskwerk:load-failed {status,
//!   error}`, `deskwerk:dirty {dirty}`, `deskwerk:saved {version, etag}` and `deskwerk:save-failed
//!   {status, error}`. A `deskwerk:save` message from exactly that origin runs File › Save.
//!
//! The token is kept in memory and taken out of the address bar. `?src=` wins over the host modes.
//!
//! The parsing and the messages are plain functions, tested natively; the browser side is
//! [`Remote`] (wasm only).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use serde_json::{Value, json};

/// The app id in `deskwerk:ready`.
const APP: &str = "docs";

/// The name a document gets when neither `name` nor `src` gives one.
const DEFAULT_NAME: &str = "Document.docx";

/// The longest file name taken from `name` or `src`.
const MAX_NAME_CHARS: usize = 255;

/// What the page's address asks for: the document's URL, where to save it, the server's token,
/// the document's name and the framing page's origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub src: String,
    pub save: Option<String>,
    pub token: Option<String>,
    pub name: String,
    pub origin: Option<String>,
}

impl Params {
    /// The parameters from the query string's values (`get("src")`…). `Ok(None)` without `src`
    /// (the normal app); an error for a URL or origin that isn't taken.
    pub fn parse(get: impl Fn(&str) -> Option<String>) -> Result<Option<Self>, String> {
        let value = |key: &str| get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let Some(src) = value("src") else { return Ok(None) };
        if !allowed_url(&src) {
            return Err(format!("src is not an https URL: {src}"));
        }
        let save = value("save");
        if let Some(save) = save.as_deref().filter(|s| !allowed_url(s)) {
            return Err(format!("save is not an https URL: {save}"));
        }
        let origin = value("origin").map(|o| o.trim_end_matches('/').to_string());
        if let Some(origin) = origin.as_deref().filter(|o| !is_origin(o)) {
            return Err(format!("origin is not an origin: {origin}"));
        }
        let name = value("name").and_then(|n| file_name(&n)).or_else(|| file_name(url_path(&src))).unwrap_or_else(|| DEFAULT_NAME.into());
        Ok(Some(Self { src, save, token: value("token"), name, origin }))
    }

    /// Whether a save named `path` writes the opened document back (its own name), rather than a
    /// copy (another name, a PDF or PNG export), which downloads.
    pub fn saves_back(&self, path: &str) -> bool {
        self.save.is_some() && file_name(path).as_deref() == Some(self.name.as_str())
    }
}

/// `scheme://authority` and the rest of `url`, when the scheme is one taken: `https`, or `http`
/// to this machine.
fn split_url(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains(['@', ' ', '\\']) {
        return None;
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().map(|h| format!("[{h}]"))?,
        None => authority.split(':').next().unwrap_or_default().to_string(),
    };
    let local = matches!(host.to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1" | "[::1]");
    match scheme.to_ascii_lowercase().as_str() {
        "https" => Some((authority, &rest[end..])),
        "http" if local => Some((authority, &rest[end..])),
        _ => None,
    }
}

fn allowed_url(url: &str) -> bool {
    split_url(url).is_some()
}

/// An origin as `postMessage` takes it: a scheme and host (and port), nothing after.
fn is_origin(origin: &str) -> bool {
    split_url(origin).is_some_and(|(_, rest)| rest.is_empty())
}

/// The path part of a URL (no query or fragment).
fn url_path(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    rest.find('/').map_or("", |i| &rest[i..])
}

/// The last part of a path, without the characters a file name can't hold, cut to
/// [`MAX_NAME_CHARS`]; `None` when nothing is left.
fn file_name(path: &str) -> Option<String> {
    let last = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let name: String =
        last.chars().filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')).take(MAX_NAME_CHARS).collect();
    let name = name.trim();
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_string())
}

/// Why the server refused a save, as the status bar says it.
pub fn save_error(status: u16) -> String {
    match status {
        412 => "the file was changed on the server since it was opened. Save a copy with File › Save As to keep your changes.".into(),
        423 => "the file is no longer locked for you on the server (the lock was released, expired or taken over). Save a copy with File › Save As to keep your changes.".into(),
        401 | 403 => "the server no longer accepts this editing session. Save a copy with File › Save As to keep your changes.".into(),
        413 => "the file is too large for the server.".into(),
        415 => "the server doesn't take this file format.".into(),
        507 => "there is no storage space left on the server.".into(),
        0 => "the server couldn't be reached.".into(),
        s => format!("the server refused it (HTTP {s})."),
    }
}

/// Why the document couldn't be loaded.
pub fn load_error(status: u16) -> String {
    match status {
        401 | 403 => "the server didn't accept this editing session. Open the file from the server again.".into(),
        404 => "the file isn't on the server.".into(),
        0 => "the server couldn't be reached.".into(),
        s => format!("the server refused it (HTTP {s})."),
    }
}

/// The version number in the server's `ETag` (`"<file>.<version>"`, maybe weak).
pub fn version_from_etag(etag: &str) -> Option<u64> {
    let tag = etag.trim().trim_start_matches("W/").trim_matches('"');
    tag.rsplit_once('.')?.1.parse().ok()
}

/// A `deskwerk:*` message: `{type, ...fields}`.
pub fn message(ty: &str, fields: Value) -> Value {
    let mut msg = json!({ "type": ty });
    if let (Some(msg), Value::Object(fields)) = (msg.as_object_mut(), fields) {
        msg.extend(fields);
    }
    msg
}

/// Whether a message is the framing page asking for File › Save: `deskwerk:save` from exactly
/// the expected origin.
pub fn is_save_request(expected: &str, origin: &str, ty: Option<&str>) -> bool {
    origin == expected && ty == Some("deskwerk:save")
}

#[cfg(target_arch = "wasm32")]
pub use browser::Remote;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::mpsc::Sender;

    use serde_json::{Value, json};
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use wordcraft_ui_egui::async_saves::{AsyncSaves, SaveOutcome};
    use wordcraft_ui_egui::{ControlRequest, Inbox, Services};

    use super::{APP, Params, load_error, message, save_error, version_from_etag};

    /// The largest document loaded, as for one a host page sends.
    const MAX_LOAD_BYTES: u64 = crate::host::MAX_OPEN_BYTES;

    /// How often a save waiting for the one before it looks again, in milliseconds.
    const WAIT_MS: i32 = 50;

    /// A failed request: the HTTP status (0 when there was no answer) and the reason.
    type Failure = (u16, String);

    pub struct Remote {
        params: Params,
        /// The `ETag` of the last load or save, sent as `If-Match`.
        etag: RefCell<Option<String>>,
        /// A save is being sent: the next one waits for its new `ETag`.
        busy: Cell<bool>,
    }

    impl Remote {
        /// The remote document this page's address asks for, if any. The token leaves the
        /// address bar (and so the history and a copied link) at once.
        pub fn from_query() -> Option<Rc<Self>> {
            let search = web_sys::window()?.location().search().ok()?;
            let query = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
            match Params::parse(|key| query.get(key)) {
                Ok(params) => {
                    let params = params?;
                    if params.token.is_some() {
                        hide_token();
                    }
                    Some(Rc::new(Self { params, etag: RefCell::new(None), busy: Cell::new(false) }))
                }
                Err(e) => {
                    log::error!("remote document not opened: {e}");
                    None
                }
            }
        }

        /// Save writes back to the server, finishing later; copies download. Changes in unsaved
        /// state reach the framing page, and its `deskwerk:save` runs File › Save.
        pub fn connect(self: &Rc<Self>, services: &mut Services, saves: AsyncSaves, ctx: egui::Context, control: Sender<ControlRequest>) {
            if self.params.save.is_some() {
                let remote = self.clone();
                // The saves the server finishes are tickets like the save picker's: keep them on.
                services.async_saves = Some(saves.clone());
                services.host_save = Some(Box::new(move |path: &str, bytes: &[u8], _save_as: bool| {
                    if !remote.params.saves_back(path) {
                        return crate::web::download(&saves, &ctx, path, bytes);
                    }
                    let ticket = saves.start();
                    let (remote, saves, ctx) = (remote.clone(), saves.clone(), ctx.clone());
                    let bytes = bytes.to_vec();
                    wasm_bindgen_futures::spawn_local(async move {
                        let outcome = match remote.put(bytes).await {
                            Ok(()) => SaveOutcome::Saved,
                            Err(e) => SaveOutcome::Failed(e),
                        };
                        saves.finish(ticket, outcome);
                        ctx.request_repaint();
                    });
                    Ok(())
                }));
            }
            if let Some(inner) = services.on_dirty.take() {
                let remote = self.clone();
                let last = Cell::new(None);
                services.on_dirty = Some(Box::new(move |dirty: bool| {
                    inner(dirty);
                    if last.replace(Some(dirty)) != Some(dirty) {
                        remote.post("deskwerk:dirty", json!({ "dirty": dirty }));
                    }
                }));
            }
            self.listen(control);
        }

        /// Fetch the document and queue it for the app, like a dropped file. A failure shows in
        /// the status bar (`messages`) and is posted.
        pub fn load(self: &Rc<Self>, inbox: Inbox, ctx: egui::Context, messages: Rc<RefCell<Vec<String>>>) {
            let remote = self.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let name = remote.params.name.clone();
                match remote.get().await {
                    Ok(bytes) => {
                        inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                        remote.post("deskwerk:loaded", json!({}));
                    }
                    Err((status, error)) => {
                        log::error!("couldn't load {name}: {error}");
                        messages.borrow_mut().push(format!("Couldn't open {name}: {error}"));
                        remote.post("deskwerk:load-failed", json!({ "status": status, "error": error }));
                    }
                }
                ctx.request_repaint();
            });
        }

        pub fn post_ready(&self) {
            self.post("deskwerk:ready", json!({ "app": APP, "version": env!("CARGO_PKG_VERSION") }));
        }

        async fn get(&self) -> Result<Vec<u8>, Failure> {
            let resp = fetch("GET", &self.params.src, self.params.token.as_deref(), None, None).await?;
            if !resp.ok() {
                return Err((resp.status(), load_error(resp.status())));
            }
            let too_big = || (413, format!("the file is larger than {} MiB.", MAX_LOAD_BYTES >> 20));
            let length = resp.headers().get("Content-Length").ok().flatten().and_then(|l| l.trim().parse::<u64>().ok());
            if length.is_some_and(|l| l > MAX_LOAD_BYTES) {
                return Err(too_big());
            }
            let etag = resp.headers().get("ETag").ok().flatten();
            let buffer = resp.array_buffer().map_err(no_answer)?;
            let buffer = JsFuture::from(buffer).await.map_err(no_answer)?;
            let array = js_sys::Uint8Array::new(&buffer);
            if u64::from(array.length()) > MAX_LOAD_BYTES {
                return Err(too_big());
            }
            *self.etag.borrow_mut() = etag;
            Ok(array.to_vec())
        }

        /// `PUT save`, one at a time, posting how it went. The error is the status bar's reason.
        async fn put(&self, bytes: Vec<u8>) -> Result<(), String> {
            while self.busy.replace(true) {
                crate::api::sleep(WAIT_MS).await;
            }
            let result = self.put_now(&bytes).await;
            self.busy.set(false);
            match result {
                Ok(etag) => {
                    let version = etag.as_deref().and_then(version_from_etag);
                    self.post("deskwerk:saved", json!({ "version": version, "etag": etag }));
                    Ok(())
                }
                Err((status, error)) => {
                    log::error!("saving {} on the server failed: {error}", self.params.name);
                    self.post("deskwerk:save-failed", json!({ "status": status, "error": error }));
                    Err(error)
                }
            }
        }

        /// The new `ETag` on success (the server's next `If-Match`).
        async fn put_now(&self, bytes: &[u8]) -> Result<Option<String>, Failure> {
            let Some(url) = &self.params.save else { return Err((0, "there is nowhere to save it.".into())) };
            let etag = self.etag.borrow().clone();
            let body = (crate::web::mime_for(&self.params.name), bytes);
            let resp = fetch("PUT", url, self.params.token.as_deref(), etag.as_deref(), Some(body)).await?;
            if !resp.ok() {
                return Err((resp.status(), save_error(resp.status())));
            }
            let etag = resp.headers().get("ETag").ok().flatten();
            *self.etag.borrow_mut() = etag.clone();
            Ok(etag)
        }

        /// `postMessage` to the framing page (or the page that opened this tab), on its origin
        /// only. Nothing without `origin`.
        fn post(&self, ty: &str, fields: Value) {
            let Some(origin) = &self.params.origin else { return };
            let Some(target) = framing_page() else { return };
            let Ok(data) = js_sys::JSON::parse(&message(ty, fields).to_string()) else { return };
            if let Err(e) = target.post_message(&data, origin) {
                log::warn!("couldn't post {ty}: {e:?}");
            }
        }

        /// Hear `deskwerk:save` from the framing page and queue File › Save for the app.
        fn listen(&self, control: Sender<ControlRequest>) {
            let Some(origin) = self.params.origin.clone() else { return };
            let Some(window) = web_sys::window() else { return };
            let on_message = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                let ty = js_sys::Reflect::get(&e.data(), &JsValue::from_str("type")).ok().and_then(|v| v.as_string());
                if super::is_save_request(&origin, &e.origin(), ty.as_deref()) {
                    // Nobody waits for the reply: the save reports itself (`deskwerk:saved`).
                    let (request, _reply) = ControlRequest::new("file.save", json!({}));
                    if control.send(request).is_err() {
                        log::warn!("deskwerk:save: the app is not running");
                    }
                }
            });
            if let Err(e) = window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()) {
                log::error!("deskwerk:save is unavailable: {e:?}");
            }
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// The parent window when framed, else the window that opened this tab.
    fn framing_page() -> Option<web_sys::Window> {
        let window = web_sys::window()?;
        if let Ok(Some(parent)) = window.parent()
            && !js_sys::Object::is(&parent, &window)
        {
            return Some(parent);
        }
        let opener = window.opener().ok().filter(|o| !o.is_null() && !o.is_undefined())?;
        Some(opener.unchecked_into())
    }

    fn no_answer(e: JsValue) -> Failure {
        log::warn!("request failed: {e:?}");
        (0, save_error(0))
    }

    /// A cross-origin request with the token and no cookies (the server takes the token alone).
    async fn fetch(
        method: &str,
        url: &str,
        token: Option<&str>,
        if_match: Option<&str>,
        body: Option<(&str, &[u8])>,
    ) -> Result<web_sys::Response, Failure> {
        let window = web_sys::window().ok_or((0, "no window".to_string()))?;
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        init.set_mode(web_sys::RequestMode::Cors);
        init.set_credentials(web_sys::RequestCredentials::Omit);
        let headers = web_sys::Headers::new().map_err(no_answer)?;
        if let Some(token) = token {
            headers.set("Authorization", &format!("Bearer {token}")).map_err(no_answer)?;
        }
        if let Some(etag) = if_match {
            headers.set("If-Match", etag).map_err(no_answer)?;
        }
        if let Some((mime, bytes)) = body {
            headers.set("Content-Type", mime).map_err(no_answer)?;
            init.set_body(&js_sys::Uint8Array::from(bytes));
        }
        init.set_headers(&headers);
        let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(no_answer)?;
        let resp = JsFuture::from(window.fetch_with_request(&request)).await.map_err(no_answer)?;
        resp.dyn_into().map_err(no_answer)
    }

    /// Take `token` out of the address bar, keeping the rest.
    fn hide_token() {
        let Some(window) = web_sys::window() else { return };
        let Ok(href) = window.location().href() else { return };
        let Ok(url) = web_sys::Url::new(&href) else { return };
        url.search_params().delete("token");
        if let Ok(history) = window.history()
            && let Err(e) = history.replace_state_with_url(&JsValue::NULL, "", Some(&url.href()))
        {
            log::warn!("couldn't take the token out of the address: {e:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(query: &[(&str, &str)]) -> Result<Option<Params>, String> {
        Params::parse(|key| query.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string()))
    }

    const SRC: &str = "https://files.example.com/editor/v1/files/f1/content";

    #[test]
    fn no_src_is_the_normal_app() {
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(parse(&[("src", "  "), ("token", "t")]), Ok(None));
    }

    #[test]
    fn the_server_link_is_read_whole() {
        let p = parse(&[("src", SRC), ("save", SRC), ("token", "abc.def"), ("name", "Report.docx"), ("origin", "https://files.example.com")])
            .unwrap()
            .unwrap();
        assert_eq!(
            p,
            Params {
                src: SRC.into(),
                save: Some(SRC.into()),
                token: Some("abc.def".into()),
                name: "Report.docx".into(),
                origin: Some("https://files.example.com".into()),
            }
        );
    }

    #[test]
    fn the_name_falls_back_to_the_path_and_is_cleaned() {
        let p = parse(&[("src", "https://h/files/Notes.odt?x=1#y")]).unwrap().unwrap();
        assert_eq!((p.name.as_str(), p.save, p.token, p.origin), ("Notes.odt", None, None, None));
        assert_eq!(parse(&[("src", "https://h/")]).unwrap().unwrap().name, DEFAULT_NAME);
        assert_eq!(parse(&[("src", SRC), ("name", "../a/b\\Q3: plan?.docx")]).unwrap().unwrap().name, "Q3 plan.docx");
        assert_eq!(parse(&[("src", SRC), ("name", "..")]).unwrap().unwrap().name, "content");
    }

    #[test]
    fn only_https_or_this_machine_is_taken() {
        for bad in ["http://files.example.com/a", "javascript:alert(1)", "file:///etc/passwd", "https://user@evil/a", "//h/a", "https:///a"] {
            assert!(parse(&[("src", bad)]).is_err(), "{bad}");
            assert!(parse(&[("src", SRC), ("save", bad)]).is_err(), "{bad}");
        }
        for good in ["http://localhost:8000/a", "http://127.0.0.1/a", "http://[::1]:8000/a", "HTTPS://Files.Example.com/a"] {
            assert!(parse(&[("src", good)]).unwrap().is_some(), "{good}");
        }
    }

    #[test]
    fn the_origin_is_an_origin() {
        let origin = |o: &str| parse(&[("src", SRC), ("origin", o)]).map(|p| p.and_then(|p| p.origin));
        assert_eq!(origin("https://files.example.com/"), Ok(Some("https://files.example.com".into())));
        assert_eq!(origin("http://localhost:8000"), Ok(Some("http://localhost:8000".into())));
        for bad in ["*", "https://files.example.com/edit", "http://files.example.com", "null"] {
            assert!(origin(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn only_the_documents_own_name_saves_back() {
        let p = parse(&[("src", SRC), ("save", SRC), ("name", "Report.docx")]).unwrap().unwrap();
        assert!(p.saves_back("Report.docx"));
        assert!(p.saves_back("some/dir/Report.docx"));
        for copy in ["Report.pdf", "Report.png", "Report.odt", "Other.docx", "report.docx"] {
            assert!(!p.saves_back(copy), "{copy}");
        }
        let read_only = parse(&[("src", SRC), ("name", "Report.docx")]).unwrap().unwrap();
        assert!(!read_only.saves_back("Report.docx"));
    }

    #[test]
    fn statuses_say_what_to_do() {
        assert!(save_error(412).contains("changed on the server"));
        assert!(save_error(423).contains("no longer locked"));
        for status in [412, 423, 401, 403] {
            assert!(save_error(status).contains("Save a copy"), "{status}");
        }
        assert!(save_error(413).contains("too large"));
        assert!(save_error(415).contains("format"));
        assert!(save_error(507).contains("storage"));
        assert!(save_error(0).contains("couldn't be reached"));
        assert!(save_error(500).contains("HTTP 500"));
        assert!(load_error(401).contains("again"));
        assert!(load_error(404).contains("isn't on the server"));
        assert!(load_error(502).contains("HTTP 502"));
    }

    #[test]
    fn the_version_comes_from_the_etag() {
        assert_eq!(version_from_etag("\"9f2c.7\""), Some(7));
        assert_eq!(version_from_etag("W/\"9f2c.12\""), Some(12));
        assert_eq!(version_from_etag("\"9f2c\""), None);
        assert_eq!(version_from_etag(""), None);
    }

    #[test]
    fn messages_carry_their_type_and_fields() {
        assert_eq!(message("deskwerk:dirty", json!({"dirty": true})), json!({"type": "deskwerk:dirty", "dirty": true}));
        assert_eq!(message("deskwerk:loaded", json!({})), json!({"type": "deskwerk:loaded"}));
        assert_eq!(message("deskwerk:ready", Value::Null), json!({"type": "deskwerk:ready"}));
    }

    #[test]
    fn only_the_framing_origin_asks_for_a_save() {
        let page = "https://files.example.com";
        assert!(is_save_request(page, page, Some("deskwerk:save")));
        assert!(!is_save_request(page, "https://files.example.com.evil.com", Some("deskwerk:save")));
        assert!(!is_save_request(page, "null", Some("deskwerk:save")));
        assert!(!is_save_request(page, page, Some("deskwerk:open")));
        assert!(!is_save_request(page, page, None));
    }
}
