/// Embedded VT323 Regular font (OFL 1.1).
/// Source: <https://github.com/phoikoi/VT323>
pub const VT323_REGULAR: &[u8] = include_bytes!("../../res/internal/fonts/VT323-Regular.ttf");

#[cfg(test)]
mod tests {
    use super::VT323_REGULAR;

    #[test]
    fn bundled_default_font_is_valid() {
        let parsed = fontdue::Font::from_bytes(VT323_REGULAR, fontdue::FontSettings::default());
        assert!(parsed.is_ok(), "bundled VT323 font must be parseable");
    }
}
