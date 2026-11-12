use eframe::egui::{self, Color32, FontFamily, FontId};

pub(crate) const GREEN: Color32 = Color32::from_rgb(30, 215, 96);

pub(crate) const BASE: Color32 = Color32::BLACK;

pub(crate) const BG: Color32 = Color32::from_rgb(18, 18, 18);

pub(crate) const ELEV: Color32 = Color32::from_rgb(26, 26, 26);

pub(crate) const HOVER: Color32 = Color32::from_rgb(42, 42, 42);

pub(crate) const PILL: Color32 = Color32::from_rgb(36, 36, 36);

pub(crate) const DIM: Color32 = Color32::from_rgb(179, 179, 179);

pub(crate) const MUTED: Color32 = Color32::from_rgb(130, 130, 130);

pub(crate) const WHITE: Color32 = Color32::WHITE;

pub(crate) fn reg(s: f32) -> FontId {
    FontId::proportional(s)
}

pub(crate) fn bold(s: f32) -> FontId {
    FontId::new(s, FontFamily::Name("bold".into()))
}

pub(crate) fn tint(seed: &str, v: f32) -> Color32 {
    let h = seed
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32))
        % 360;
    egui::ecolor::Hsva::new(h as f32 / 360.0, 0.55, v, 1.0).into()
}

const NOTO_REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const NOTO_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");

/// Noto Sans is embedded (Apache-2.0, see assets/fonts) so the app looks the same on every OS.
pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [("noto-regular", NOTO_REGULAR), ("noto-bold", NOTO_BOLD)] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto-regular".into());
    // bold family = the bold face, then the built-in fonts as glyph fallbacks
    let mut bold = vec!["noto-bold".to_string()];
    bold.extend(defaults);
    fonts.families.insert(FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);
}
