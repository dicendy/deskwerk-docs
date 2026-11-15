//! Picture outlines (Picture Format › Picture Border: `pic:spPr/a:ln`) read from Word's markup,
//! written back the way Word writes them, and kept through save and reopen, for inline
//! (`wp:inline`) and floating (`wp:anchor`) pictures.

use wordcraft_doc::line_dash::LineDash;
use wordcraft_doc::para::{Float, PicOutline, Wrap};
use wordcraft_doc::props::{CharProps, Rgb};
use wordcraft_doc::{Document, InlineObject, Paragraph, para_block};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOC_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdP" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/p.png"/></Relationships>"#;

fn tiny_png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(3, 2, |x, y| image::Rgba([x as u8 * 80, y as u8 * 120, 200, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A 2 × 1 inch picture's `pic:pic` with `ln` (an `a:ln`, or nothing) in its `pic:spPr`, as Word
/// writes it.
fn pic(id: u32, ln: &str) -> String {
    format!(
        r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="p.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdP"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr bwMode="auto"><a:xfrm><a:off x="0" y="0"/><a:ext cx="1828800" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{ln}</pic:spPr></pic:pic></a:graphicData></a:graphic>"#
    )
}

fn inline(id: u32, ln: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1828800" cy="914400"/><wp:effectExtent l="6350" t="6350" r="6350" b="6350"/><wp:docPr id="{id}" name="Picture {id}"/><wp:cNvGraphicFramePr/>{}</wp:inline></w:drawing></w:r>"#,
        pic(id, ln)
    )
}

fn anchor(id: u32, ln: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="1828800" cy="914400"/><wp:effectExtent l="19050" t="19050" r="19050" b="19050"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="{id}" name="Picture {id}"/><wp:cNvGraphicFramePr/>{}</wp:anchor></w:drawing></w:r>"#,
        pic(id, ln)
    )
}

/// A package whose body is one paragraph of `runs`, with the picture they show.
fn docx(runs: &str) -> Vec<u8> {
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body><w:p>{runs}</w:p></w:body></w:document>"#);
    let png = tiny_png();
    let entries: [(&str, &[u8]); 4] = [
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", DOC_RELS.as_bytes()),
        ("word/media/p.png", &png),
    ];
    super::fixtures::zip(&entries)
}

/// The pictures of the document's first paragraph: their wrap and outline.
fn outlines(d: &Document) -> Vec<(Wrap, Option<PicOutline>)> {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Image { float, .. } => Some((float.wrap, float.outline.clone())),
            _ => None,
        })
        .collect()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
    xml
}

const RED_1PT_DASH: &str = r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:prstDash val="dash"/></a:ln>"#;
const GREEN_3PT_DOT: &str = r#"<a:ln w="38100"><a:solidFill><a:srgbClr val="00B050"/></a:solidFill><a:prstDash val="sysDot"/></a:ln>"#;
const BLUE_SOLID: &str = r#"<a:ln w="57150"><a:solidFill><a:srgbClr val="0070C0"/></a:solidFill></a:ln>"#;

fn word_document() -> Vec<u8> {
    let runs = [
        inline(1, RED_1PT_DASH),
        anchor(2, GREEN_3PT_DOT),
        inline(3, BLUE_SOLID),
        // No outline: Word's explicit one, and none at all.
        inline(4, "<a:ln><a:noFill/></a:ln>"),
        inline(5, ""),
        // A colour without a width is Word's ¾ pt.
        anchor(6, r#"<a:ln><a:solidFill><a:srgbClr val="7030A0"/></a:solidFill></a:ln>"#),
    ];
    docx(&runs.concat())
}

fn expected() -> Vec<(Wrap, Option<PicOutline>)> {
    vec![
        (Wrap::Inline, Some(PicOutline::new(Rgb(0xFF, 0, 0), 1.0, LineDash::Dash))),
        (Wrap::Square, Some(PicOutline::new(Rgb(0, 0xB0, 0x50), 3.0, LineDash::SystemDot))),
        (Wrap::Inline, Some(PicOutline::new(Rgb(0, 0x70, 0xC0), 4.5, LineDash::Solid))),
        (Wrap::Inline, None),
        (Wrap::Inline, None),
        (Wrap::Square, Some(PicOutline::new(Rgb(0x70, 0x30, 0xA0), 0.75, LineDash::Solid))),
    ]
}

/// Width (EMU to points), colour and dash of Word's picture outlines are read, inline and
/// floating; `a:noFill` and a missing `a:ln` are no outline.
#[test]
fn word_picture_outlines_are_read() {
    let d = wordcraft_docx::read(&word_document()).expect("read");
    assert_eq!(outlines(&d), expected());
}

/// Saved, each outline is written as Word writes it, in `pic:spPr` after the geometry; a picture
/// without one gets no `a:ln`; reopened, every outline is the same.
#[test]
fn picture_outlines_are_written_as_word_writes_them_and_round_trip() {
    let d = wordcraft_docx::read(&word_document()).expect("read");
    let bytes = wordcraft_docx::write(&d).expect("write");
    let xml = document_xml(&bytes);
    for ln in [RED_1PT_DASH, GREEN_3PT_DOT, BLUE_SOLID] {
        assert!(xml.contains(&format!(r#"<a:avLst/></a:prstGeom>{ln}</pic:spPr>"#)), "{ln} in {xml}");
    }
    assert!(xml.contains(r#"<a:ln w="9525"><a:solidFill><a:srgbClr val="7030A0"/></a:solidFill></a:ln>"#), "{xml}");
    assert_eq!(xml.matches("<a:ln").count(), 4, "no outline for the pictures without one: {xml}");
    let again = wordcraft_docx::read(&bytes).expect("read again");
    assert_eq!(outlines(&again), expected());
    // And a second save writes the same outlines.
    assert_eq!(document_xml(&wordcraft_docx::write(&again).expect("write again")).matches("<a:ln").count(), 4);
}

/// An outline set in the editor's model (any width, colour and dash, a custom dash too) survives
/// save and reopen, inline and floating, and a picture without one stays without.
#[test]
fn model_picture_outlines_round_trip() {
    let mut d = Document::new();
    let key = d.add_media(tiny_png(), "png");
    let picture = |float: Float| InlineObject::Image {
        media: key.clone(),
        w: 144.0,
        h: 72.0,
        alt: String::new(),
        float,
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    };
    let lines = [
        Some(PicOutline::new(Rgb(0x12, 0x34, 0x56), 0.25, LineDash::Solid)),
        Some(PicOutline::new(Rgb(0, 0, 0), 6.0, LineDash::LongDashDotDot)),
        Some(PicOutline::new(Rgb(0xC0, 0, 0), 2.25, LineDash::Custom(vec![(300_000, 100_000), (100_000, 100_000)]))),
        None,
    ];
    let mut p = Paragraph::with_text("pictures ", CharProps::default());
    for (i, outline) in lines.iter().enumerate() {
        let wrap = if i % 2 == 0 { Wrap::Inline } else { Wrap::Square };
        let end = p.len();
        p.insert_object(end, picture(Float { wrap, outline: outline.clone(), ..Default::default() }), &CharProps::default()).unwrap();
    }
    d.body = vec![para_block(p)];
    let bytes = wordcraft_docx::write(&d).expect("write");
    let xml = document_xml(&bytes);
    assert!(xml.contains(r#"<a:ln w="3175"><a:solidFill><a:srgbClr val="123456"/></a:solidFill></a:ln>"#), "{xml}");
    assert!(xml.contains(r#"<a:ln w="76200"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:prstDash val="lgDashDotDot"/></a:ln>"#), "{xml}");
    assert!(xml.contains(r#"<a:custDash><a:ds d="300000" sp="100000"/><a:ds d="100000" sp="100000"/></a:custDash>"#), "{xml}");
    assert_eq!(xml.matches("<a:ln").count(), 3, "{xml}");
    let r = wordcraft_docx::read(&bytes).expect("read");
    let got: Vec<Option<PicOutline>> = outlines(&r).into_iter().map(|(_, o)| o).collect();
    assert_eq!(got, lines);
}

/// The `wp:effectExtent` of each drawing in `xml`, in order: (l, t, r, b), EMU.
fn effect_extents(xml: &str) -> Vec<[i64; 4]> {
    xml.split("<wp:effectExtent ")
        .skip(1)
        .map(|rest| {
            let tag = &rest[..rest.find("/>").expect("an empty element")];
            ["l", "t", "r", "b"].map(|n| {
                let at = tag.find(&format!(r#"{n}=""#)).expect("the side") + n.len() + 2;
                tag[at..at + tag[at..].find('"').unwrap()].parse().unwrap()
            })
        })
        .collect()
}

/// Half an outline lies outside the picture's frame, so the effect extent written for it is at
/// least half its width (rounded up to whole EMU) on every side, or Word clips that half — the
/// top edge then looks thinner. A larger extent the file kept stays; a picture without an
/// outline keeps its own.
#[test]
fn effect_extent_makes_room_for_the_outline() {
    let mut d = Document::new();
    let key = d.add_media(tiny_png(), "png");
    let picture = |float: Float| InlineObject::Image {
        media: key.clone(),
        w: 144.0,
        h: 72.0,
        alt: String::new(),
        float,
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    };
    let lines = [
        Some(PicOutline::new(Rgb(0x12, 0x34, 0x56), 0.25, LineDash::Solid)),
        Some(PicOutline::new(Rgb(0, 0, 0), 6.0, LineDash::Solid)),
        Some(PicOutline::new(Rgb(0xC0, 0, 0), 2.25, LineDash::Dash)),
        None,
    ];
    let mut p = Paragraph::with_text("pictures ", CharProps::default());
    for (i, outline) in lines.iter().enumerate() {
        let wrap = if i % 2 == 0 { Wrap::Inline } else { Wrap::Square };
        let end = p.len();
        p.insert_object(end, picture(Float { wrap, outline: outline.clone(), ..Default::default() }), &CharProps::default()).unwrap();
    }
    d.body = vec![para_block(p)];
    let bytes = wordcraft_docx::write(&d).expect("write");
    // 3175, 76200 and 28575 EMU wide.
    assert_eq!(effect_extents(&document_xml(&bytes)), vec![[1588; 4], [38100; 4], [14288; 4], [0; 4]]);
    // Reopened and saved again, the same.
    let again = wordcraft_docx::read(&bytes).expect("read");
    let xml = document_xml(&wordcraft_docx::write(&again).expect("write again"));
    assert_eq!(effect_extents(&xml), vec![[1588; 4], [38100; 4], [14288; 4], [0; 4]]);

    // Word's file: 6350 recorded on the inline pictures, 19050 on the floating ones.
    let d = wordcraft_docx::read(&word_document()).expect("read");
    let xml = document_xml(&wordcraft_docx::write(&d).expect("write"));
    assert_eq!(
        effect_extents(&xml),
        vec![
            [6350; 4],  // 1 pt: room enough
            [19050; 4], // 3 pt: room enough
            [28575; 4], // 4.5 pt: widened
            [6350; 4],  // no outline
            [6350; 4], [19050; 4], // ¾ pt: the recorded room is larger
        ]
    );
}

/// A file that keeps no room for an outline (`wp:effectExtent` of zero, or none) still opens,
/// and the picture's line is given that room: half the outline's width on each side.
#[test]
fn outline_without_recorded_room_is_given_it() {
    let bare = |id: u32, ln: &str| inline(id, ln).replace(r#"<wp:effectExtent l="6350" t="6350" r="6350" b="6350"/>"#, "");
    let zero = |id: u32, ln: &str| inline(id, ln).replace(r#"l="6350" t="6350" r="6350" b="6350""#, r#"l="0" t="0" r="0" b="0""#);
    let d = wordcraft_docx::read(&docx(&[bare(1, BLUE_SOLID), zero(2, GREEN_3PT_DOT)].concat())).expect("read");
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    let extents: Vec<[f32; 4]> = p
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Image { float, .. } => Some(float.effect_extent()),
            _ => None,
        })
        .collect();
    assert_eq!(extents, vec![[2.25; 4], [1.5; 4]]);
    let xml = document_xml(&wordcraft_docx::write(&d).expect("write"));
    assert_eq!(effect_extents(&xml), vec![[28575; 4], [19050; 4]]);
}
