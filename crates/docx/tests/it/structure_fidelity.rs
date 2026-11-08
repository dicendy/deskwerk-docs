//! Content that files from other tools place where the reader didn't look: list levels without a
//! start value, a table inside a paragraph, several bodies, range markers between table rows and
//! cells, and a symbol as a custom note mark. Each is read as LibreOffice and Word show it and
//! survives a save.

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
