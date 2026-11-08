//! Content that files from other tools place where the reader didn't look: list levels without a
//! start value, a table inside a paragraph, several bodies, range markers between table rows and
//! cells, and a symbol as a custom note mark. Each is read as LibreOffice and Word show it and
//! survives a save. So do drawings the model can't show (a locked canvas, a picture whose image
//! is linked or missing), a shapetype inside a VML group, fields inside fields, a bibliography's
//! result of several paragraphs, and a list's own level definition without a restart.

use std::io::Read;

use wordcraft_doc::numbering::Counters;
use wordcraft_doc::{Block, Document, InlineObject, Paragraph};

use crate::fixtures::zip;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A package whose main part is `document` (a whole `w:document`), with `parts` (path in `word/`,
/// relationship type, content) related to it.
fn package(document: &str, parts: &[(&str, &str, &str)]) -> Vec<u8> {
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let mut rels =
        String::from(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for (i, (path, ty, _)) in parts.iter().enumerate() {
        rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="{REL}/{ty}" Target="{path}"/>"#, i + 1));
    }
    rels.push_str("</Relationships>");
    let names: Vec<String> = parts.iter().map(|(path, _, _)| format!("word/{path}")).collect();
    let mut entries: Vec<(&str, &[u8])> =
        vec![("_rels/.rels", root.as_bytes()), ("word/document.xml", document.as_bytes()), ("word/_rels/document.xml.rels", rels.as_bytes())];
    for (name, (_, _, content)) in names.iter().zip(parts) {
        entries.push((name.as_str(), content.as_bytes()));
    }
    zip(&entries)
}

fn body_package(body: &str, parts: &[(&str, &str, &str)]) -> Vec<u8> {
    package(&format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W_NS}><w:body>{body}</w:body></w:document>"#), parts)
}

fn saved_and_read(d: &Document) -> Document {
    wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap()
}

fn para(b: &Block) -> &Paragraph {
    b.as_para().expect("a paragraph")
}

fn table(b: &Block) -> &wordcraft_doc::Table {
    match b {
        Block::Table(t) => t,
        Block::Para(p) => panic!("a table, not the paragraph {:?}", p.plain_text()),
    }
}

/// The text of each body paragraph and, for a table, of each paragraph in its first cell.
fn outline(blocks: &[std::sync::Arc<Block>]) -> Vec<String> {
    let mut out = Vec::new();
    for b in blocks {
        match &**b {
            Block::Para(p) => out.push(p.plain_text()),
            Block::Table(t) => {
                out.push("[".into());
                out.extend(outline(&t.rows[0].cells[0].blocks));
                out.push("]".into());
            }
        }
    }
    out
}

#[test]
fn a_list_level_without_a_start_counts_from_zero() {
    // ECMA-376 §17.9.25: `w:start` left out means 0; Word and LibreOffice number such a list
    // 0., 1., 2. (an absent level keeps the usual 1).
    let numbering = format!(
        r#"<w:numbering {W_NS}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let item = r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>"#;
    let d = wordcraft_docx::read(&body_package(&item.repeat(2), &[("numbering.xml", "numbering", &numbering)])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        let starts: Vec<u32> = (0..3).map(|l| d.numbering.level(1, l).unwrap().start).collect();
        assert_eq!(starts, [0, 3, 1]);
        let mut c = Counters::default();
        let labels: Vec<String> = (0..2).map(|_| c.next_label(&d.numbering, 1, 0).unwrap().0).collect();
        assert_eq!(labels, ["0.", "1."]);
    }
}

#[test]
fn a_table_inside_a_paragraph_follows_the_paragraph() {
    // Not valid (CT_P has no w:tbl), but written by some tools; LibreOffice shows the table
    // after the paragraph's text (tdf#111550), in a cell and in the body alike.
    let inner = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let body = format!(
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r>{inner}</w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>before</w:t></w:r>{inner}</w:p><w:p><w:r><w:t>after</w:t></w:r></w:p>"#
    );
    let d = wordcraft_docx::read(&body_package(&body, &[])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        assert_eq!(&outline(&d.body)[..4], ["[", "A2", "[", "inner"], "{:?}", outline(&d.body));
        let cell = &table(&d.body[0]).rows[0].cells[0];
        assert_eq!(table(&cell.blocks[1]).rows[0].cells[0].blocks.len(), 1);
        let texts: Vec<String> = d.body[1..].iter().map(|b| outline(std::slice::from_ref(b)).join("")).collect();
        assert_eq!(texts, ["before", "[inner]", "after"]);
    }
}

#[test]
fn every_body_of_a_main_part_with_several_is_read() {
    // Some tools write one `w:body` per page; LibreOffice shows them all. The last section
    // properties are the document's.
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W_NS}><w:body><w:p><w:r><w:t>one</w:t></w:r></w:p></w:body><w:body><w:p><w:r><w:t>two</w:t></w:r></w:p></w:body><w:body><w:p><w:r><w:t>three</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"#
    );
    let d = wordcraft_docx::read(&package(&doc, &[])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        assert_eq!(outline(&d.body), ["one", "two", "three"]);
        assert!((d.last_section.page_w - 595.3).abs() < 0.1, "{}", d.last_section.page_w);
    }
}

/// The bookmark starts (`+name`) and ends (`-name`) in a paragraph, in order.
fn bookmarks(p: &Paragraph) -> Vec<String> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::BookmarkStart { name } => Some(format!("+{name}")),
            InlineObject::BookmarkEnd { name } => Some(format!("-{name}")),
            _ => None,
        })
        .collect()
}

#[test]
fn bookmarks_between_rows_and_cells_are_kept() {
    // CT_Tbl and CT_Row take range markers between rows and cells (Word's `_GoBack`, `_Hlk…`):
    // one before a row or cell starts that cell; one after the last cell ends the last cell.
    let cell = |t: &str| format!(r#"<w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>"#);
    let body = format!(
        r#"<w:tbl><w:bookmarkStart w:id="1" w:name="table"/><w:tr><w:bookmarkStart w:id="2" w:name="row"/>{a}<w:bookmarkEnd w:id="2"/><w:sdt><w:sdtContent>{b}</w:sdtContent></w:sdt><w:bookmarkStart w:id="3" w:name="back"/><w:bookmarkEnd w:id="3"/></w:tr><w:tr>{c}</w:tr><w:bookmarkEnd w:id="1"/></w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
        a = cell("a"),
        b = cell("b"),
        c = cell("c"),
    );
    let d = wordcraft_docx::read(&body_package(&body, &[])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        let t = table(&d.body[0]);
        let marks: Vec<Vec<String>> = t.rows.iter().flat_map(|r| &r.cells).map(|c| bookmarks(para(&c.blocks[0]))).collect();
        assert_eq!(marks, [vec!["+table", "+row"], vec!["-row", "+back", "-back"], vec!["-table"]]);
        let texts: Vec<String> = t.rows.iter().flat_map(|r| &r.cells).map(|c| para(&c.blocks[0]).shown_text()).collect();
        assert_eq!(texts, ["a", "b", "c"]);
        assert!(bookmarks(para(&d.body[1])).is_empty());
    }
}

#[test]
fn a_symbol_as_a_custom_note_mark_is_the_mark() {
    // `w:customMarkFollows` then a `w:sym` (Insert › Symbol as the mark): the symbol, as its
    // Unicode character, is the mark; no automatic number appears (tdf#129912).
    let notes = format!(r#"<w:footnotes {W_NS}><w:footnote w:id="1"><w:p><w:r><w:t>Note</w:t></w:r></w:p></w:footnote></w:footnotes>"#);
    let body = r#"<w:p><w:r><w:t>ab</w:t></w:r><w:r><w:footnoteReference w:customMarkFollows="1" w:id="1"/><w:sym w:font="Symbol" w:char="F0A7"/></w:r></w:p>"#;
    let d = wordcraft_docx::read(&body_package(body, &[("footnotes.xml", "footnotes", &notes)])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        let p = para(&d.body[0]);
        let customs: Vec<&str> = p
            .objects
            .iter()
            .filter_map(|o| match o {
                InlineObject::NoteRef { custom, .. } => Some(custom.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(customs, ["\u{2663}"]);
        assert!(!p.text.contains('\u{F0A7}'), "the symbol is the mark, not text after it");
    }
}

const DRAWING_NS: &str = r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:lc="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas""#;
const PICTURE_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// A package whose main part has namespaces `ns` and body `body`, with the relationships `rels`
/// (`Relationship` elements) and the files `files`.
fn rels_package(ns: &str, body: &str, rels: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {ns}><w:body>{body}</w:body></w:document>"#);
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    );
    let mut entries: Vec<(&str, &[u8])> =
        vec![("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes()), ("word/_rels/document.xml.rels", rels.as_bytes())];
    entries.extend_from_slice(files);
    zip(&entries)
}

/// Part `name` of the package `bytes`.
fn part_xml(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// A paragraph holding one inline drawing 72 × 36 pt whose `a:graphicData` (of `uri`) holds
/// `payload`.
fn drawing_para(uri: &str, payload: &str) -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="1" name="Drawing 1"/><a:graphic><a:graphicData uri="{uri}">{payload}</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

/// A picture whose `a:blip` has the attributes `blip`, in a frame of 1 × ½ in.
fn picture(blip: &str) -> String {
    format!(
        r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="p.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip {blip}/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#
    )
}

/// The size and kept `a:graphic` of the drawing in the body's first paragraph, which the model
/// holds as a picture without an image (shown as the missing-picture box).
fn kept_drawing(d: &Document) -> (f32, f32, String) {
    match para(&d.body[0]).objects.as_slice() {
        [InlineObject::Image { media, w, h, ole: Some(kept), .. }] if media.is_empty() => {
            assert!(kept.xml.starts_with("<a:graphic "), "{}", kept.xml);
            (*w, *h, kept.xml.clone())
        }
        other => panic!("one kept drawing, not {other:?}"),
    }
}

#[test]
fn a_linked_picture_is_kept_with_its_link() {
    // `r:link` only (Insert › Picture › Link to File): the image is outside the package, which
    // is never read; the picture is kept and saved with its external relationship.
    let rels = format!(r#"<Relationship Id="rIdL" Type="{REL}/image" Target="file:///C:/pictures/p.png" TargetMode="External"/>"#);
    let body = drawing_para(PICTURE_URI, &picture(r#"r:link="rIdL""#));
    let d = wordcraft_docx::read(&rels_package(&format!("{W_NS} {DRAWING_NS}"), &body, &rels, &[])).unwrap();
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = part_xml(&bytes, "word/document.xml");
    assert!(xml.contains("<pic:pic>") && xml.contains("r:link=\""), "{xml}");
    let doc_rels = part_xml(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="file:///C:/pictures/p.png" TargetMode="External""#), "{doc_rels}");
    for d in [&d, &wordcraft_docx::read(&bytes).unwrap()] {
        let (w, h, kept) = kept_drawing(d);
        assert_eq!((w, h), (72.0, 36.0));
        assert!(kept.contains("r:link="), "{kept}");
    }
}

#[test]
fn a_picture_whose_image_is_missing_is_kept_without_it() {
    // The relationship names a part the package lacks: the picture stays, without the image,
    // as LibreOffice and Word show it (an empty frame), rather than leaving the document.
    let rels = format!(r#"<Relationship Id="rIdGone" Type="{REL}/image" Target="media/gone.png"/>"#);
    let body = drawing_para(PICTURE_URI, &picture(r#"r:embed="rIdGone""#));
    let d = wordcraft_docx::read(&rels_package(&format!("{W_NS} {DRAWING_NS}"), &body, &rels, &[])).unwrap();
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = part_xml(&bytes, "word/document.xml");
    assert!(xml.contains("<a:blip") && !xml.contains("r:embed="), "{xml}");
    assert!(xml.contains(r#"<a:ext cx="914400" cy="457200"/>"#), "the picture's frame follows the object: {xml}");
    for d in [&d, &wordcraft_docx::read(&bytes).unwrap()] {
        assert_eq!(kept_drawing(d).0, 72.0);
    }
}

/// A locked canvas (Word 2003's drawing canvas) holding one rectangle, its members in their own
/// space (12345 × 6789 EMU).
const LOCKED_CANVAS: &str = r#"<lc:lockedCanvas><a:nvGrpSpPr><a:cNvPr id="2" name="Canvas"/><a:cNvGrpSpPr/></a:nvGrpSpPr><a:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12345" cy="6789"/><a:chOff x="0" y="0"/><a:chExt cx="12345" cy="6789"/></a:xfrm></a:grpSpPr><a:sp><a:nvSpPr><a:cNvPr id="3" name="Box"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12345" cy="6789"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr></a:sp></lc:lockedCanvas>"#;

#[test]
fn a_locked_canvas_is_kept_whole() {
    // Transitional and Strict: the canvas is saved back as it was, in the transitional
    // namespace and graphic URI, its members' frames untouched by the drawing's own size.
    let strict = r#"xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main" xmlns:r="http://purl.oclc.org/ooxml/officeDocument/relationships" xmlns:wp="http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing" xmlns:a="http://purl.oclc.org/ooxml/drawingml/main" xmlns:lc="http://purl.oclc.org/ooxml/drawingml/lockedCanvas" w:conformance="strict""#;
    let transitional = format!("{W_NS} {DRAWING_NS}");
    for (ns, uri) in [
        (transitional.as_str(), "http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"),
        (strict, "http://purl.oclc.org/ooxml/drawingml/lockedCanvas"),
    ] {
        let d = wordcraft_docx::read(&rels_package(ns, &drawing_para(uri, LOCKED_CANVAS), "", &[])).unwrap();
        let bytes = wordcraft_docx::write(&d).unwrap();
        let xml = part_xml(&bytes, "word/document.xml");
        assert!(xml.contains(r#"xmlns:lc="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas""#), "{xml}");
        assert!(xml.contains(r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><lc:lockedCanvas>"#), "{xml}");
        assert_eq!(xml.matches(r#"<a:ext cx="12345" cy="6789"/>"#).count(), 2, "{xml}");
        assert!(xml.contains(r#"cx="914400" cy="457200""#), "{xml}");
        for d in [&d, &wordcraft_docx::read(&bytes).unwrap()] {
            let (w, h, kept) = kept_drawing(d);
            assert_eq!((w, h), (72.0, 36.0));
            assert!(kept.contains("<lc:lockedCanvas>") && kept.contains(r#"name="Box""#), "{kept}");
        }
    }
}

#[test]
fn a_shapetype_inside_a_vml_group_is_found() {
    // Some producers write the `v:shapetype` inside the group whose member uses it.
    let ns = format!(r#"{W_NS} xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office""#);
    let body = concat!(
        r#"<w:p><w:r><w:pict><v:group style="width:100pt;height:50pt" coordsize="1000,500">"#,
        r#"<v:shapetype id="tri" o:spt="5" coordsize="21600,21600"/>"#,
        r##"<v:shape type="#tri" style="left:0;top:0;width:400;height:500"/>"##,
        r#"<v:rect style="left:600;top:0;width:400;height:500"/>"#,
        r#"</v:group></w:pict></w:r></w:p>"#,
    );
    let d = wordcraft_docx::read(&rels_package(&ns, body, "", &[])).unwrap();
    let InlineObject::Group { children, .. } = &para(&d.body[0]).objects[0] else { panic!("{:?}", para(&d.body[0]).objects) };
    let kinds: Vec<_> = children
        .iter()
        .map(|c| match &c.obj {
            InlineObject::Shape { kind, .. } => *kind,
            o => panic!("{o:?}"),
        })
        .collect();
    assert_eq!(kinds, [wordcraft_doc::para::ShapeKind::Triangle, wordcraft_doc::para::ShapeKind::Rectangle]);
}

fn fld(kind: &str) -> String {
    format!(r#"<w:r><w:fldChar w:fldCharType="{kind}"/></w:r>"#)
}

fn instr(code: &str) -> String {
    format!(r#"<w:r><w:instrText xml:space="preserve">{code}</w:instrText></w:r>"#)
}

fn run(text: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{text}</w:t></w:r>"#)
}

/// The fields in a paragraph: `<code` for a field start, `code=result` for a field, `>` for a
/// field end.
fn fields(p: &Paragraph) -> Vec<String> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::FieldStart { instr, .. } => Some(format!("<{instr}")),
            InlineObject::Field { instr, result, .. } => Some(format!("{instr}={result}")),
            InlineObject::FieldEnd => Some(">".into()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_field_in_another_fields_code_is_saved_as_a_field() {
    // `IF { MERGEFIELD City } = "Oslo" …`: the inner field is part of the code, and goes back as
    // a field nested in it, not as its result or as braces in the code.
    let body = [
        fld("begin"),
        instr(" IF "),
        fld("begin"),
        instr(" MERGEFIELD City "),
        fld("separate"),
        run("Oslo"),
        fld("end"),
        instr(r#" = "Oslo" "yes" "no" "#),
        fld("separate"),
        run("yes"),
        fld("end"),
    ]
    .concat();
    let d = wordcraft_docx::read(&body_package(&format!("<w:p>{body}</w:p>"), &[])).unwrap();
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = part_xml(&bytes, "word/document.xml");
    assert_eq!(xml.matches(r#"w:fldCharType="begin""#).count(), 2, "{xml}");
    assert!(xml.contains("> MERGEFIELD City <") && !xml.contains("{ MERGEFIELD"), "{xml}");
    for d in [&d, &wordcraft_docx::read(&bytes).unwrap()] {
        assert_eq!(fields(para(&d.body[0])), [r#"IF { MERGEFIELD City } = "Oslo" "yes" "no"=yes"#]);
    }
}

#[test]
fn a_field_in_another_fields_result_stays_a_field() {
    // `IF 1 = 1 "by { AUTHOR }"` shows "by Ada": the result keeps its own field.
    let body = [
        fld("begin"),
        instr(r#" IF 1 = 1 "by " "#),
        fld("separate"),
        run("by "),
        fld("begin"),
        instr(" AUTHOR "),
        fld("separate"),
        run("Ada"),
        fld("end"),
        fld("end"),
    ]
    .concat();
    let d = wordcraft_docx::read(&body_package(&format!("<w:p>{body}</w:p>"), &[])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        let p = para(&d.body[0]);
        assert_eq!(fields(p), [r#"<IF 1 = 1 "by ""#, "AUTHOR=Ada", ">"]);
        assert!(p.plain_text().contains("by "), "{:?}", p.plain_text());
    }
}

#[test]
fn a_bibliography_of_several_paragraphs_is_a_range() {
    // Its result is the list of sources, a paragraph each: the field spans them all.
    let body = format!(
        "<w:p>{}{}{}{}</w:p><w:p>{}{}</w:p>",
        fld("begin"),
        instr(" BIBLIOGRAPHY "),
        fld("separate"),
        run("Source one"),
        run("Source two"),
        fld("end"),
    );
    let d = wordcraft_docx::read(&body_package(&body, &[])).unwrap();
    for d in [&d, &saved_and_read(&d)] {
        let ps: Vec<&Paragraph> = d.body.iter().filter_map(|b| b.as_para()).collect();
        assert_eq!(fields(ps[0]), ["<BIBLIOGRAPHY"]);
        assert_eq!(fields(ps[1]), [">"]);
        assert!(ps[0].plain_text().contains("Source one") && ps[1].plain_text().contains("Source two"));
    }
    // One that ends in its paragraph (the engine writes its list's field empty, after a heading)
    // stays a field.
    let body = format!("<w:p>{}{}{}{}{}</w:p>", run("References"), fld("begin"), instr(" BIBLIOGRAPHY "), fld("separate"), fld("end"));
    let d = wordcraft_docx::read(&body_package(&body, &[])).unwrap();
    let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
    assert_eq!(fields(p), ["BIBLIOGRAPHY="]);
}

#[test]
fn a_lists_own_level_without_a_start_override_goes_on_counting() {
    // `w:lvlOverride` holding a `w:lvl` (with its own `w:start`) but no `w:startOverride`: Word
    // goes on from the abstract list's count, and saving adds no restart.
    let level = |text: &str| format!(r#"<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="{text}"/></w:lvl>"#);
    let numbering = format!(
        r#"<w:numbering {W_NS}><w:abstractNum w:abstractNumId="0">{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0">{}</w:lvlOverride></w:num></w:numbering>"#,
        level("%1."),
        level("%1)"),
    );
    let item =
        |num: u32| format!(r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="{num}"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>"#);
    let body = [item(1), item(1), item(2)].concat();
    let d = wordcraft_docx::read(&body_package(&body, &[("numbering.xml", "numbering", &numbering)])).unwrap();
    let bytes = wordcraft_docx::write(&d).unwrap();
    assert!(!part_xml(&bytes, "word/numbering.xml").contains("w:startOverride"));
    for d in [&d, &wordcraft_docx::read(&bytes).unwrap()] {
        let mut c = Counters::default();
        let labels: Vec<String> = [1, 1, 2].iter().map(|&n| c.next_label(&d.numbering, n, 0).unwrap().0).collect();
        assert_eq!(labels, ["1.", "2.", "3)"]);
    }
}
