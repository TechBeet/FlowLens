use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

pub const CJK_FAMILY: &str = "flowlens-cjk";

const CJK_FONT_CANDIDATES: &[&str] = &[
    r"C:\\Windows\\Fonts\\msyh.ttc",
    r"C:\\Windows\\Fonts\\msyhl.ttc",
    r"C:\\Windows\\Fonts\\Deng.ttf",
    r"C:\\Windows\\Fonts\\simhei.ttf",
    r"C:\\Windows\\Fonts\\msjh.ttc",
    r"C:\\Windows\\Fonts\\simsun.ttc",
    r"C:\\Windows\\Fonts\\arialuni.ttf",
];

pub fn find_cjk_font() -> Option<PathBuf> {
    for candidate in CJK_FONT_CANDIDATES {
        let path = Path::new(candidate);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    None
}

pub fn build_font_definitions() -> (FontDefinitions, Option<String>) {
    let mut definitions = FontDefinitions::default();
    let Some(path) = find_cjk_font() else { return (definitions, None); };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return (definitions, None),
    };
    definitions
        .font_data
        .insert(CJK_FAMILY.to_owned(), Arc::new(FontData::from_owned(bytes)));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = definitions.families.get_mut(&family) {
            list.insert(0, CJK_FAMILY.to_owned());
        }
    }
    (definitions, Some(path.display().to_string()))
}

pub fn install(ctx: &egui::Context) -> Option<String> {
    let (definitions, label) = build_font_definitions();
    ctx.set_fonts(definitions);
    label
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_cjk_font_returns_existing_file() {
        if let Some(path) = find_cjk_font() {
            assert!(path.is_file(), "font not found: {}", path.display());
        }
    }

    #[test]
    fn test_cjk_font_supports_latin_and_digits() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut ok = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                ok = fonts.has_glyphs(&egui::FontId::proportional(16.0), "HTTP 200 1.2 MB");
            });
        });
        assert!(ok, "latin/digit glyphs missing");
    }

    #[test]
    fn test_cjk_font_covers_chinese_glyphs() {
        let ctx = egui::Context::default();
        let label = install(&ctx);
        let Some(label) = label else { panic!("no CJK font found"); };
        let mut has = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                has = fonts.has_glyphs(&egui::FontId::proportional(16.0), "流光网络流量分析器");
            });
        });
        assert!(has, "CJK glyphs missing in {}", label);
    }
}