use super::toc::is_toc;
use super::{CharProps, W, Writer, rpr};

pub(super) struct FieldData<'a> {
    pub instr: &'a str,
    pub result: &'a str,
    pub locked: bool,
    pub dirty: bool,
    pub form: &'a str,
}

impl Writer<'_> {
    pub(super) fn field(&mut self, w: &mut W, field: FieldData<'_>, props: &CharProps) {
        let FieldData { instr, result, locked, dirty, form } = field;
        if self.toc_hold_end && is_toc(instr) {
            // A TOC heading's field opens in the first entry and closes after the last (see `toc_span`).
            self.toc_hold_end = false;
            self.toc_field = Some((instr.to_string(), locked, dirty, props.clone()));
            return;
        }
        if let Some(e) = wordcraft_doc::eq::Enclosure::parse(instr) {
            self.rev_open(w, props);
            self.enclosure(w, &e, locked, props);
            self.rev_close(w, props);
            return;
        }
        let del = self.is_del(props);
        let run = |w: &mut W, f: &dyn Fn(&mut W)| {
            w.open("w:r", &[]);
            rpr(w, props);
            f(w);
            w.close("w:r");
        };
        self.rev_open(w, props);
        self.field_start(w, instr, locked, dirty, form, props);
        if !result.is_empty() {
            run(w, &|w| {
                let mut buf = String::new();
                let flush = |w: &mut W, buf: &mut String| {
                    if !buf.is_empty() {
                        w.leaf(if del { "w:delText" } else { "w:t" }, &[("xml:space", "preserve")], buf);
                        buf.clear();
                    }
                };
                for c in result.chars() {
                    match c {
                        '\t' => {
                            flush(w, &mut buf);
                            w.empty("w:tab", &[]);
                        }
                        '\n' | '\r' => {
                            flush(w, &mut buf);
                            w.empty("w:br", &[]);
                        }
                        c if (c as u32) < 0x20 || c == wordcraft_doc::para::OBJ => {}
                        c => buf.push(c),
                    }
                }
                flush(w, &mut buf);
            });
        }
        run(w, &|w| w.empty("w:fldChar", &[("w:fldCharType", "end")]));
        self.rev_close(w, props);
    }

    /// Enclosed characters as Word stores them: an `eq \o\ac(○,字)` field with no result, whose
    /// code is split so the symbol and the text get their own sizes: "shrink text" makes the text
    /// smaller (raised to sit in the middle), "enlarge symbol" makes the symbol bigger (lowered).
    fn enclosure(&self, w: &mut W, e: &wordcraft_doc::eq::Enclosure, locked: bool, props: &CharProps) {
        use wordcraft_doc::eq::EncloseStyle;
        let del = self.is_del(props);
        let tag = if del { "w:delInstrText" } else { "w:instrText" };
        let base = props.size.unwrap_or_else(|| self.doc.styles.resolve_char(None, props).size).clamp(1.0, 1638.0);
        let half = |pt: f32| (pt * 2.0).round() / 2.0;
        let (mut sym, mut txt) = (props.clone(), props.clone());
        match e.style {
            EncloseStyle::Shrink => {
                txt.size = Some(half(base * 0.7).max(1.0));
                txt.position = Some(half(base * 0.1));
            }
            EncloseStyle::Enlarge => {
                sym.size = Some(half(base * 1.5));
                sym.position = Some(-half(base * 0.2));
            }
        }
        let run = |w: &mut W, p: &CharProps, f: &dyn Fn(&mut W)| {
            w.open("w:r", &[]);
            rpr(w, p);
            f(w);
            w.close("w:r");
        };
        run(w, props, &|w| {
            if locked {
                w.empty("w:fldChar", &[("w:fldCharType", "begin"), ("w:fldLock", "1")])
            } else {
                w.empty("w:fldChar", &[("w:fldCharType", "begin")])
            }
        });
        let code = e.word_instr();
        // `eq \o\ac(` + symbol, `,`, the text, `)`.
        let open = code.find('(').map(|i| i + 1).unwrap_or(0);
        let comma = open + e.shape.symbol().len_utf8();
        let (head, rest) = code.split_at(comma.min(code.len()));
        let text = rest.get(1..rest.len().saturating_sub(1)).unwrap_or("");
        run(w, &sym, &|w| w.leaf(tag, &[("xml:space", "preserve")], &format!(" {head}")));
        run(w, props, &|w| w.leaf(tag, &[("xml:space", "preserve")], ","));
        run(w, &txt, &|w| w.leaf(tag, &[("xml:space", "preserve")], text));
        run(w, props, &|w| w.leaf(tag, &[("xml:space", "preserve")], ")"));
        run(w, props, &|w| w.empty("w:fldChar", &[("w:fldCharType", "end")]));
    }

    /// A field's begin, instruction and separate runs.
    /// A field's begin, code and separator; `form` is a legacy form field's `w:ffData`, which
    /// belongs inside the begin.
    pub(super) fn field_start(&self, w: &mut W, instr: &str, locked: bool, dirty: bool, form: &str, props: &CharProps) {
        let del = self.is_del(props);
        let run = |w: &mut W, f: &dyn Fn(&mut W)| {
            w.open("w:r", &[]);
            rpr(w, props);
            f(w);
            w.close("w:r");
        };
        run(w, &|w| {
            let mut attrs: Vec<(&str, &str)> = vec![("w:fldCharType", "begin")];
            if locked {
                attrs.push(("w:fldLock", "1"));
            }
            if dirty {
                attrs.push(("w:dirty", "true"));
            }
            if form.is_empty() {
                w.empty("w:fldChar", &attrs);
            } else {
                w.open("w:fldChar", &attrs);
                w.raw(form);
                w.close("w:fldChar");
            }
        });
        // A citation manager's code (`ADDIN ZOTERO_ITEM CSL_CITATION {"title":"{DNA} …"}`) is
        // data whose braces are its own, never nested fields.
        let addin = instr.trim_start().get(..5).is_some_and(|k| k.eq_ignore_ascii_case("ADDIN"));
        instr_runs(w, &format!(" {} ", instr.trim()), del, props, if addin { MAX_NESTED_CODE } else { 0 });
        run(w, &|w| w.empty("w:fldChar", &[("w:fldCharType", "separate")]));
    }
}

/// One `w:r` with `props`, holding what `f` writes (a field character or code).
pub(super) fn field_run(w: &mut W, props: &CharProps, f: &dyn Fn(&mut W)) {
    w.open("w:r", &[]);
    rpr(w, props);
    f(w);
    w.close("w:r");
}

/// How deep nested fields in a field's code are written as fields; deeper ones stay code text.
const MAX_NESTED_CODE: usize = 8;

/// `code`, a field's code, as instruction runs (`w:delInstrText` when deleted). Each `{ CODE }`
/// in it — a nested field's code, as the reader keeps it (`IF { MERGEFIELD City } = "Oslo" …`) —
/// goes back as a nested field: a begin, its own code and an end. A brace that opens no field
/// code (`{` not followed by a space and then a field name or `=`, as in a URL's `{id}`, or never
/// closed) stays text.
fn instr_runs(w: &mut W, code: &str, del: bool, props: &CharProps, depth: usize) {
    let tag = if del { "w:delInstrText" } else { "w:instrText" };
    let text = |w: &mut W, t: &str| {
        if !t.is_empty() {
            field_run(w, props, &|w| w.leaf(tag, &[("xml:space", "preserve")], t));
        }
    };
    let (mut from, mut at) = (0, 0);
    while let Some(open) = code.get(at..).and_then(|s| s.find('{')).map(|i| i + at) {
        at = open + 1;
        let opens_code = code
            .get(at..)
            .is_some_and(|r| r.starts_with(char::is_whitespace) && r.trim_start().starts_with(|c: char| c.is_ascii_alphabetic() || c == '='));
        if depth >= MAX_NESTED_CODE || !opens_code {
            continue;
        }
        let Some(close) = code_end(code, open) else { continue };
        let inner = code.get(at..close).unwrap_or("");
        text(w, code.get(from..open).unwrap_or(""));
        field_run(w, props, &|w| w.empty("w:fldChar", &[("w:fldCharType", "begin")]));
        instr_runs(w, inner, del, props, depth + 1);
        field_run(w, props, &|w| w.empty("w:fldChar", &[("w:fldCharType", "end")]));
        (from, at) = (close + 1, close + 1);
    }
    text(w, code.get(from..).unwrap_or(""));
}

/// The byte offset of the `}` closing the `{` at `open` in `code`, nested braces counted.
fn code_end(code: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in code.get(open..)?.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}
