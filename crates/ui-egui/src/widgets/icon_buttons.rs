use super::buttons::DROPDOWN;
use super::combo::drawn_state;
use super::{Color32, CornerRadius, Rect, Response, Sense, Stroke, Tokens, Ui, icons, pos2, regular, vec2};

/// Height of a dialog push button, in points (the shared Docs/Sheets button spec).
pub const PUSH_H: f32 = 32.0;
/// Narrowest a push button gets, whatever its label.
pub const PUSH_MIN_W: f32 = 96.0;
/// Space between the label and each side of the button.
pub const PUSH_PAD_X: f32 = 16.0;
/// Corner radius of a push button.
pub const PUSH_RADIUS: u8 = 4;
/// Label size: the normal interface font at 14 pt.
pub const PUSH_FONT: f32 = 14.0;
/// Space between neighbouring push buttons in a row.
pub const PUSH_GAP: f32 = 8.0;

/// The three push button looks of a dialog's button row, as in Word's prompts: Save filled in
/// the app colour, Don't Save outlined in it, Cancel neutral grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushKind {
    /// The default answer: accent fill, `on_accent` text.
    Primary,
    /// A secondary answer: no fill, a 1 px border and text in the accent colour.
    Outline,
    /// Cancel, Close and the like: no fill, a 1 px grey border and normal text.
    Neutral,
}

/// A dialog or backstage push button drawn to the shared spec: 32 pt high, at least 96 pt wide,
/// 16 pt padding, 4 pt corners, 14 pt label. Hover and press darken it (the fill of a primary
/// button, a tint of an outline or neutral one); keyboard focus draws a 2 pt accent ring around
/// it, pointer focus doesn't; disabled, it fades to 40 % and doesn't react. Add with `ui.add`.
#[must_use = "add it with `ui.add`"]
pub struct PushButton {
    text: String,
    kind: PushKind,
    min_w: f32,
}

impl PushButton {
    pub fn new(kind: PushKind, text: impl Into<String>) -> Self {
        Self { text: text.into(), kind, min_w: PUSH_MIN_W }
    }

    /// At least `w` points wide: the buttons of one row share the widest one's width
    /// ([`push_width`]).
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_w = self.min_w.max(w);
        self
    }
}

/// A dialog's primary button (OK, Save, Close).
pub fn primary_button(text: impl Into<String>) -> PushButton {
    PushButton::new(PushKind::Primary, text)
}

/// A dialog's secondary button outlined in the accent colour (Don't Save).
pub fn outline_button(text: impl Into<String>) -> PushButton {
    PushButton::new(PushKind::Outline, text)
}

/// A dialog's neutral button (Cancel, Close, Apply).
pub fn neutral_button(text: impl Into<String>) -> PushButton {
    PushButton::new(PushKind::Neutral, text)
}

fn push_label_width(ui: &Ui, text: &str) -> f32 {
    ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), regular(PUSH_FONT), Color32::WHITE).size().x)
}

/// The width every button of a row takes so they all match: the widest label plus padding,
/// never under [`PUSH_MIN_W`].
pub fn push_width(ui: &Ui, labels: &[&str]) -> f32 {
    labels.iter().map(|l| push_label_width(ui, l) + 2.0 * PUSH_PAD_X).fold(PUSH_MIN_W, f32::max)
}

/// Whether focus came from the keyboard (Tab or the arrow keys) since the last pointer press:
/// only then does a push button show its focus ring.
fn keyboard_focus_visible(ui: &Ui) -> bool {
    let id = egui::Id::unique("deskwerk_push_keyboard_focus");
    let (pressed, keys) = ui.input(|i| {
        let keys =
            [egui::Key::Tab, egui::Key::ArrowLeft, egui::Key::ArrowRight, egui::Key::ArrowUp, egui::Key::ArrowDown].iter().any(|k| i.key_pressed(*k));
        (i.pointer.any_pressed(), keys)
    });
    ui.ctx().data_mut(|d| {
        let v = d.get_temp_mut_or_insert_with(id, || false);
        if pressed {
            *v = false;
        } else if keys {
            *v = true;
        }
        *v
    })
}

/// `c` darkened by `f` (0..1) toward black.
fn darken(c: Color32, f: f32) -> Color32 {
    c.lerp_to_gamma(Color32::BLACK, f)
}

impl egui::Widget for PushButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = Tokens::get(ui.ctx());
        let w = (push_label_width(ui, &self.text) + 2.0 * PUSH_PAD_X).max(self.min_w);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, PUSH_H), Sense::click());
        let enabled = ui.is_enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &self.text));
        let ring = keyboard_focus_visible(ui) && resp.has_focus();
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let pressed = enabled && resp.is_pointer_button_down_on();
        let hovered = enabled && (resp.hovered() || pressed);
        let (fill, border, ink) = match self.kind {
            PushKind::Primary => {
                let fill = if pressed {
                    darken(t.accent, 0.20)
                } else if hovered {
                    darken(t.accent, 0.10)
                } else {
                    t.accent
                };
                (fill, Color32::TRANSPARENT, t.on_accent)
            }
            PushKind::Outline => {
                let a = if t.dark { t.accent_text } else { t.accent };
                let fill = if pressed {
                    a.gamma_multiply(0.16)
                } else if hovered {
                    a.gamma_multiply(0.08)
                } else {
                    Color32::TRANSPARENT
                };
                (fill, if pressed { darken(a, 0.10) } else { a }, a)
            }
            PushKind::Neutral => {
                let fill = if pressed {
                    t.text.gamma_multiply(0.12)
                } else if hovered {
                    t.text.gamma_multiply(0.06)
                } else {
                    Color32::TRANSPARENT
                };
                (fill, t.input_border, t.text)
            }
        };
        // Disabled: 40 % of the enabled look. A fresh painter, so egui's own fading of a
        // disabled `Ui` doesn't stack on top.
        let (painter, fade) =
            if enabled { (ui.painter().clone(), 1.0) } else { (ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(ui.clip_rect()), 0.4) };
        let cr = CornerRadius::same(PUSH_RADIUS);
        let stroke = if border == Color32::TRANSPARENT { Stroke::NONE } else { Stroke::new(1.0, border.gamma_multiply(fade)) };
        painter.rect(rect, cr, fill.gamma_multiply(fade), stroke, egui::StrokeKind::Inside);
        let galley = ui.ctx().fonts_mut(|f| f.layout_no_wrap(self.text.clone(), regular(PUSH_FONT), ink.gamma_multiply(fade)));
        let pos = rect.center() - galley.size() / 2.0;
        painter.galley(pos, galley, ink.gamma_multiply(fade));
        if ring {
            let a = if t.dark { t.accent_text } else { t.accent };
            painter.rect_stroke(rect.expand(1.0), CornerRadius::same(PUSH_RADIUS + 2), Stroke::new(2.0, a), egui::StrokeKind::Outside);
        }
        resp
    }
}

/// A frameless 22 px icon button (pane close buttons): ribbon-style hover and press feedback.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    let st = drawn_state(ui, &resp, false);
    ui.painter().rect_filled(r, CornerRadius::same(4), st.fill);
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(14.0, 14.0)), icon, t.icon, t.accent);
    resp.on_hover_text(tip)
}

/// A text button with a drawn icon in front. Symbols such as ✎ or 💬 aren't in the interface
/// fonts and would draw as empty boxes, so icons are painted (`icons.rs`) rather than typed.
/// It looks and reacts like `ui.button` (see [`drawn_state`]).
pub fn icon_text_button(ui: &mut Ui, icon: &str, text: &str, font: egui::FontId) -> Response {
    icon_text_button_with(ui, icon, text, font, false, true)
}

/// [`icon_text_button`] that opens a menu: the drop-down marker follows the text.
pub fn icon_text_menu_button(ui: &mut Ui, icon: &str, text: &str, font: egui::FontId) -> Response {
    icon_text_button_with(ui, icon, text, font, true, true)
}

/// The icon-only form of [`icon_text_button`] / [`icon_text_menu_button`] for a crowded strip
/// (#2026): `label` is the accessible name and the tooltip, not drawn.
pub fn icon_only_button(ui: &mut Ui, icon: &str, label: &str, font: egui::FontId, marker: bool) -> Response {
    icon_text_button_with(ui, icon, label, font, marker, false).on_hover_text(label)
}

/// Width [`icon_text_button`] (or its menu / icon-only forms) takes.
pub fn icon_text_button_width(ui: &Ui, text: &str, font: egui::FontId, marker: bool, show_text: bool) -> f32 {
    let text_w = if show_text { ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER).size().x) } else { 0.0 };
    icon_text_button_size(ui, text_w, marker, show_text)
}

fn icon_text_button_size(ui: &Ui, text_w: f32, marker: bool, show_text: bool) -> f32 {
    let pad = ui.spacing().button_padding;
    let gap = if show_text { 4.0 } else { 0.0 };
    let marker_w = if marker { DROPDOWN + 4.0 } else { 0.0 };
    pad.x + 16.0 + gap + text_w + marker_w + pad.x
}

/// Allocates and paints a themed icon button with optional text, marker and accessible label, and
/// returns its response.
fn icon_text_button_with(ui: &mut Ui, icon: &str, text: &str, font: egui::FontId, marker: bool, show_text: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let pad = ui.spacing().button_padding;
    let icon_w = 16.0;
    let text_w = if show_text { galley.size().x } else { 0.0 };
    let size = vec2(icon_text_button_size(ui, text_w, marker, show_text), galley.size().y.max(icon_w) + 2.0 * pad.y);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text));
    let st = drawn_state(ui, &resp, true);
    let r = r.expand(st.expansion);
    ui.painter().rect(r, ui.visuals().widgets.inactive.corner_radius, st.fill, st.stroke, egui::StrokeKind::Inside);
    // By language, not by layout: these buttons also sit in far-side clusters, which fill from
    // the other end.
    let rtl = crate::rtl::on();
    icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(crate::rtl::lead_x(rtl, r, pad.x + icon_w / 2.0), r.center().y), vec2(icon_w, icon_w)),
        icon,
        t.icon,
        t.accent,
    );
    let text_x = crate::rtl::lead_left(rtl, r, pad.x + icon_w + if show_text { 4.0 } else { 0.0 }, text_w);
    if show_text {
        ui.painter().galley(pos2(text_x, r.center().y - galley.size().y / 2.0), galley, st.text);
    }
    if marker {
        let c = pos2(if rtl { text_x - 4.0 - DROPDOWN / 2.0 } else { text_x + text_w + 4.0 + DROPDOWN / 2.0 }, r.center().y);
        icons::paint(ui.painter(), Rect::from_center_size(c, vec2(DROPDOWN, DROPDOWN)), "dropdown", st.text, st.text);
    }
    resp
}

/// A drop-down menu item with `icon` before `text`, at least `min_w` wide.
pub fn icon_menu_item(ui: &mut Ui, icon: &str, text: &str, shortcut: &str, min_w: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), Color32::PLACEHOLDER);
    let sc = (!shortcut.is_empty()).then(|| ui.painter().layout_no_wrap(shortcut.to_string(), font, Color32::PLACEHOLDER));
    let pad = ui.spacing().button_padding;
    let icon_w = 16.0;
    let sc_w = sc.as_ref().map_or(0.0, |g| g.size().x + 3.0 * pad.x);
    let w = (pad.x + icon_w + 6.0 + galley.size().x + sc_w + pad.x).max(min_w).max(ui.available_width());
    let (r, resp) = ui.allocate_exact_size(vec2(w, galley.size().y.max(icon_w) + 2.0 * pad.y), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text));
    let st = drawn_state(ui, &resp, false);
    ui.painter().rect_filled(r, ui.visuals().widgets.inactive.corner_radius, st.fill);
    let (line, accent) = if ui.is_enabled() { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    let text_c = if ui.is_enabled() { st.text } else { t.text_disabled };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + pad.x + icon_w / 2.0, r.center().y), vec2(icon_w, icon_w)), icon, line, accent);
    ui.painter().galley(pos2(r.min.x + pad.x + icon_w + 6.0, r.center().y - galley.size().y / 2.0), galley, text_c);
    if let Some(g) = sc {
        ui.painter().galley(pos2(r.max.x - pad.x - g.size().x, r.center().y - g.size().y / 2.0), g, t.text_disabled);
    }
    resp
}

/// A strip of dialog tabs. The active tab joins the page below it.
/// Returns the index of the tab the user clicked.
pub fn dialog_tabs(ui: &mut Ui, labels: &[String], selected: usize) -> Option<usize> {
    const H: f32 = 26.0;
    let t = Tokens::get(ui.ctx());
    let page = ui.visuals().window_fill;
    let need: f32 = labels.iter().map(|l| ui.painter().layout_no_wrap(l.clone(), regular(12.5), t.text).size().x + 27.0).sum::<f32>() + 1.0;
    ui.set_min_width(need);
    let origin = ui.cursor().min;
    let width = ui.available_width();
    let base_y = origin.y + H;
    let base = ui.painter().add(egui::Shape::Noop);
    let top = CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 };
    // Right to left, the tabs start at the right edge and run leftwards.
    let rtl = crate::rtl::flows(ui);
    let mut x = if rtl { origin.x + width } else { origin.x };
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let on = i == selected;
        let galley = ui.painter().layout_no_wrap(label.clone(), regular(12.5), t.text);
        let w = galley.size().x + 28.0;
        let left = if rtl { x - w } else { x };
        let r = Rect::from_min_max(pos2(left, origin.y + 2.0), pos2(left + w, base_y));
        let resp = ui.interact(r, ui.scope_id().with(("dialog_tab", i)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
        let hover = ui.ctx().animate_bool_responsive(resp.id.with("hover"), resp.hovered());
        let fill = if on { page } else { page.lerp_to_gamma(t.border, 0.14 + 0.1 * hover) };
        let p = ui.painter();
        let shown = if on { r } else { Rect::from_min_max(pos2(r.min.x, r.min.y + 2.0), r.max) };
        p.rect(shown, top, fill, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        if on {
            p.rect_filled(Rect::from_min_max(pos2(r.min.x + 1.0, base_y - 1.0), pos2(r.max.x - 1.0, base_y + 1.0)), 0.0, page);
        }
        let color = if on { t.text } else { t.text.gamma_multiply(0.8) };
        p.galley(pos2(r.center().x - galley.size().x / 2.0, r.center().y - galley.size().y / 2.0), galley, color);
        if resp.clicked() && !on {
            clicked = Some(i);
        }
        x += if rtl { 1.0 - w } else { w - 1.0 };
    }
    ui.allocate_space(vec2(width, H + 1.0));
    ui.painter().set(base, egui::Shape::line_segment([pos2(origin.x, base_y), pos2(origin.x + width, base_y)], Stroke::new(1.0, t.border)));
    clicked
}
