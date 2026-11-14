use super::*;
use wordcraft_doc::connector::ShapeExtra;
use wordcraft_doc::freeform::{FreePath, Freeform, InkTool};

#[test]
fn freeform_fill_has_own_alpha() {
    let rect = Rect::new(4.0, 4.0, 24.0, 24.0);
    for transparency in [0.0, 50.0, 100.0] {
        for closed in [false, true] {
            let freeform = Freeform {
                w: 24.0,
                h: 24.0,
                paths: vec![FreePath { pts: vec![[0.0, 0.0], [24.0, 0.0], [24.0, 24.0]], closed }],
                alpha: 0.25,
                ..Default::default()
            };
            let mut out = Vec::new();
            shape_draws(
                rect,
                &ShapeLook {
                    kind: ShapeKind::Freeform,
                    fill: Some(&wordcraft_doc::fill::Fill::Solid { color: Rgb::BLACK, transparency }),
                    stroke: Some(Rgb::WHITE),
                    stroke_width: 2.0,
                    effects: Default::default(),
                    freeform: Some(&freeform),
                    extra: &ShapeExtra::default(),
                },
                &DisplayOptions::default(),
                &mut out,
            );
            let Draw::Path { fill, stroke, stroke_width, .. } = &out[0] else { panic!("expected freeform path") };
            match fill {
                Some(Paint::Solid(color, alpha)) if closed => {
                    assert_eq!(*color, Rgb::BLACK);
                    assert_eq!(*alpha, 1.0 - transparency / 100.0);
                }
                None if !closed => {}
                _ => panic!("unexpected freeform paint: {fill:?}"),
            }
            assert_eq!((*stroke, *stroke_width), (Some(Rgb::WHITE), 2.0));
        }
    }
    let ink = Freeform::ink(InkTool::Highlighter, 24.0, 24.0, vec![[0.0, 0.0], [24.0, 24.0]]);
    let mut out = Vec::new();
    shape_draws(
        rect,
        &ShapeLook {
            kind: ShapeKind::Freeform,
            fill: Some(&wordcraft_doc::fill::Fill::Solid { color: Rgb::BLACK, transparency: 100.0 }),
            stroke: Some(Rgb::WHITE),
            stroke_width: 2.0,
            effects: Default::default(),
            freeform: Some(&ink),
            extra: &ShapeExtra::default(),
        },
        &DisplayOptions::default(),
        &mut out,
    );
    assert!(matches!(out.as_slice(), [Draw::Ink { alpha, .. }] if *alpha == 0.5));
}

#[test]
fn other_path_alpha_defaults() {
    let rect = Rect::new(4.0, 4.0, 24.0, 24.0);
    let graphic = Graphic {
        items: vec![GraphicItem::Path {
            segs: vec![PathSeg::Move(0.0, 0.0), PathSeg::Line(24.0, 24.0)],
            fill: Some(Rgb::BLACK),
            stroke: None,
            stroke_width: 0.0,
        }],
        ..Default::default()
    };
    let mut out = graphic_draws(&Document::new(), &graphic, rect, 1.0);
    out.push(path_arrowhead(&[(4.0, 4.0), (28.0, 28.0)], Rgb::BLACK, 2.0, ArrowStyle::default()).unwrap());
    let extra = ShapeExtra { arrow_start: true, arrow_end: true, ..Default::default() };
    line_draws(rect, ShapeKind::ElbowConnector, Some(Rgb::BLACK), 2.0, Default::default(), &extra, &mut out);
    assert_eq!(out.len(), 5);
    assert!(out.iter().all(
        |item| matches!(item, Draw::Path { fill, .. } if fill.as_ref().is_none_or(|paint| matches!(paint, Paint::Solid(_, alpha) if *alpha == 1.0)))
    ));
}

/// A picture's outline (Picture Border) is drawn over it, centred on its frame, in its colour,
/// width and dashes, inside the picture's turn; a picture without one is the image alone.
#[test]
fn picture_outline_is_drawn_on_the_frame() {
    use wordcraft_doc::line_dash::LineDash;
    use wordcraft_doc::para::{Float, PicOutline};
    let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
    let picture = |outline: Option<PicOutline>, rot: f32| InlineObject::Image {
        media: "m".into(),
        w: 100.0,
        h: 50.0,
        alt: String::new(),
        float: Float { outline, rot, ..Default::default() },
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    };
    let draws = |o: &InlineObject| {
        let mut out = Vec::new();
        object_draws(o, rect, Spin::default(), 1.0, &DisplayOptions::default(), &mut out);
        out
    };
    let plain = draws(&picture(None, 0.0));
    assert!(matches!(plain.as_slice(), [Draw::Image { .. }]), "{plain:?}");
    let red = PicOutline::new(Rgb(0xC0, 0, 0), 3.0, LineDash::Dash);
    let out = draws(&picture(Some(red.clone()), 0.0));
    let [Draw::Image { .. }, Draw::Shape { rect: r, kind, fill, stroke, stroke_width, dash, .. }] = out.as_slice() else { panic!("{out:?}") };
    assert_eq!((*r, *kind, fill.is_none(), *stroke, *stroke_width), (rect, ShapeKind::Rectangle, true, Some(Rgb(0xC0, 0, 0)), 3.0));
    assert_eq!(*dash, LineDash::Dash.pattern(3.0));
    let turned = draws(&picture(Some(red), 30.0));
    let [Draw::Rotated { items, .. }] = turned.as_slice() else { panic!("{turned:?}") };
    assert!(matches!(items.as_slice(), [Draw::Image { .. }, Draw::Shape { .. }]), "the outline turns with the picture: {items:?}");
}
