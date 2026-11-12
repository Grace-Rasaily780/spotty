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

pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, path: &str, fam: FontFamily, first: bool| {
        if let Ok(b) = std::fs::read(path) {
            fonts.font_data.insert(
                name.into(),
                std::sync::Arc::new(egui::FontData::from_owned(b)),
            );
            let v = fonts.families.entry(fam).or_default();
            if first {
                v.insert(0, name.into())
            } else {
                v.push(name.into())
            }
            true
        } else {
            false
        }
    };
    let has_bold = add(
        "noto-bold",
        "/usr/share/fonts/noto/NotoSans-Bold.ttf",
        FontFamily::Name("bold".into()),
        true,
    );
    add(
        "noto-reg",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        FontFamily::Proportional,
        true,
    );
    if !has_bold {
        let v = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts.families.insert(FontFamily::Name("bold".into()), v);
    } else {
        // glyph fallbacks for the bold family
        let fb = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        let v = fonts
            .families
            .get_mut(&FontFamily::Name("bold".into()))
            .unwrap();
        for f in fb {
            if !v.contains(&f) {
                v.push(f);
            }
        }
    }
    ctx.set_fonts(fonts);
}
