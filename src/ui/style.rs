use crate::ui::colors::Color;
use cssparser::{Parser, ParserInput, Token};

use crate::ui::selector::Selector;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overflow {
    Visible,
    Hidden,
}

/// Result of diffing a node's computed style against its previous value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleChange {
    NoChange,
    PaintOnly,
    LayoutAndPaint,
    LayoutAndPaintAndInherited,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PseudoState {
    Hover,
    Active,
    Focus,
    Disabled,
}

/// Which side of a box a directional property (e.g. `margin-top`) targets.
/// Indexes align with the `[top, right, bottom, left]` array layout used by
/// `ComputedStyle::margin` and `ComputedStyle::padding`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top = 0,
    Right = 1,
    Bottom = 2,
    Left = 3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Flex,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignItems {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JustifyContent {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

#[derive(Clone, Debug)]
pub enum StyleValue {
    Display(Display),
    Width(Length),
    Height(Length),
    MinWidth(Length),
    MinHeight(Length),
    MaxWidth(Length),
    MaxHeight(Length),
    Margin([Length; 4]),
    MarginSide(Side, Length),
    Padding([Length; 4]),
    PaddingSide(Side, Length),
    Gap(Length),
    FlexDirection(FlexDirection),
    FlexGrow(f32),
    FlexShrink(f32),
    AlignItems(AlignItems),
    JustifyContent(JustifyContent),
    BackgroundColor(Color),
    BorderColor(Color),
    BorderWidth(f32),
    BorderRadius(f32),
    Opacity(f32),
    FontFamily(String),
    FontSize(f32),
    TextColor(Color),
    TextAlign(TextAlign),
    ZIndex(i32),
    PointerEvents(bool),
    Visibility(bool),
    Overflow(Overflow),
}

#[derive(Clone, Debug)]
pub struct Declaration {
    pub property: String,
    pub value: StyleValue,
}

#[derive(Clone, Debug)]
pub struct StyleRule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
    pub specificity: u32,
    pub source_order: u32,
}

#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub display: Display,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub margin: [Length; 4],
    pub padding: [Length; 4],
    pub gap: Length,
    pub flex_direction: FlexDirection,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,
    pub opacity: f32,
    pub font_family: String,
    pub font_size: f32,
    pub text_color: Color,
    pub text_align: TextAlign,
    pub z_index: i32,
    pub pointer_events: bool,
    pub visibility: bool,
    pub overflow: Overflow,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            margin: [Length::Px(0.0); 4],
            padding: [Length::Px(0.0); 4],
            gap: Length::Px(0.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Start,
            background_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            opacity: 1.0,
            font_family: "UI".to_string(),
            font_size: 16.0,
            text_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            text_align: TextAlign::Left,
            z_index: 0,
            pointer_events: true,
            visibility: true,
            overflow: Overflow::Visible,
        }
    }
}

impl Declaration {
    pub fn from_src(src: &str) -> Result<Vec<Declaration>, String> {
        // split the src by the semicolons and parse each declaration
        let src_declarations = src.split(';').map(|s| s.trim()).filter(|s| !s.is_empty());

        let mut declarations = Vec::new();
        for src_declaration in src_declarations {
            if let Some((property, value)) = src_declaration.split_once(':') {
                let property = &property.trim().to_lowercase();
                let value = &value.trim().to_lowercase();
                let style_value = StyleValue::from_src(property, value)?;
                declarations.push(Declaration {
                    property: property.to_string(),
                    value: style_value,
                });
            } else {
                return Err(format!("Invalid declaration: {}", src_declaration));
            }
        }
        Ok(declarations)
    }
}

impl StyleValue {
    pub fn from_src(property: &str, value: &str) -> Result<StyleValue, String> {
        match property {
            "display" => Ok(StyleValue::Display(Display::from_src(value)?)),
            "width" => Ok(StyleValue::Width(Length::from_src(value)?)),
            "height" => Ok(StyleValue::Height(Length::from_src(value)?)),
            "min-width" => Ok(StyleValue::MinWidth(Length::from_src(value)?)),
            "min-height" => Ok(StyleValue::MinHeight(Length::from_src(value)?)),
            "max-width" => Ok(StyleValue::MaxWidth(parse_max_length(value)?)),
            "max-height" => Ok(StyleValue::MaxHeight(parse_max_length(value)?)),
            "margin" => Ok(StyleValue::Margin(parse_box_shorthand(value)?)),
            "margin-top" => Ok(StyleValue::MarginSide(Side::Top, Length::from_src(value)?)),
            "margin-right" => Ok(StyleValue::MarginSide(
                Side::Right,
                Length::from_src(value)?,
            )),
            "margin-bottom" => Ok(StyleValue::MarginSide(
                Side::Bottom,
                Length::from_src(value)?,
            )),
            "margin-left" => Ok(StyleValue::MarginSide(Side::Left, Length::from_src(value)?)),
            "padding" => Ok(StyleValue::Padding(parse_box_shorthand(value)?)),
            "padding-top" => Ok(StyleValue::PaddingSide(Side::Top, Length::from_src(value)?)),
            "padding-right" => Ok(StyleValue::PaddingSide(
                Side::Right,
                Length::from_src(value)?,
            )),
            "padding-bottom" => Ok(StyleValue::PaddingSide(
                Side::Bottom,
                Length::from_src(value)?,
            )),
            "padding-left" => Ok(StyleValue::PaddingSide(
                Side::Left,
                Length::from_src(value)?,
            )),
            "gap" => Ok(StyleValue::Gap(Length::from_src(value)?)),
            "flex-direction" => Ok(StyleValue::FlexDirection(FlexDirection::from_src(value)?)),
            "flex-grow" => {
                Ok(StyleValue::FlexGrow(value.parse::<f32>().map_err(
                    |_| format!("Invalid flex-grow value: {}", value),
                )?))
            }
            "flex-shrink" => {
                Ok(StyleValue::FlexShrink(value.parse::<f32>().map_err(
                    |_| format!("Invalid flex-shrink value: {}", value),
                )?))
            }
            "align-items" => Ok(StyleValue::AlignItems(AlignItems::from_src(value)?)),
            "justify-content" => Ok(StyleValue::JustifyContent(JustifyContent::from_src(value)?)),
            "background-color" => Ok(StyleValue::BackgroundColor(Color::from_src(value)?)),
            "border-color" => Ok(StyleValue::BorderColor(Color::from_src(value)?)),
            "border-width" => Ok(StyleValue::BorderWidth(parse_px_length(
                value,
                "border-width",
            )?)),
            "border-radius" => Ok(StyleValue::BorderRadius(parse_px_length(
                value,
                "border-radius",
            )?)),
            "opacity" => {
                Ok(StyleValue::Opacity(value.parse::<f32>().map_err(|_| {
                    format!("Invalid opacity value: {}", value)
                })?))
            }
            "font-family" => Ok(StyleValue::FontFamily(parse_font_family(value)?)),
            "font-size" => Ok(StyleValue::FontSize(parse_px_length(value, "font-size")?)),
            "color" => Ok(StyleValue::TextColor(Color::from_src(value)?)),
            "text-align" => Ok(StyleValue::TextAlign(TextAlign::from_src(value)?)),
            "z-index" => {
                Ok(StyleValue::ZIndex(value.parse::<i32>().map_err(|_| {
                    format!("Invalid z-index value: {}", value)
                })?))
            }
            "overflow" => Ok(StyleValue::Overflow(Overflow::from_src(value)?)),
            "visibility" => Ok(StyleValue::Visibility(parse_visibility(value)?)),
            "pointer-events" => Ok(StyleValue::PointerEvents(parse_pointer_events(value)?)),
            _ => Err(format!("Unsupported style property: {}", property)),
        }
    }
}

impl Overflow {
    pub fn from_src(src: &str) -> Result<Overflow, String> {
        match src {
            "visible" => Ok(Overflow::Visible),
            "hidden" => Ok(Overflow::Hidden),
            _ => Err(format!("Unsupported overflow value: {}", src)),
        }
    }
}

/// Parse a CSS box shorthand (margin/padding) accepting 1-4 space-separated
/// length values. Returns `[top, right, bottom, left]` per standard CSS rules.
fn parse_box_shorthand(value: &str) -> Result<[Length; 4], String> {
    if value.contains(',') {
        return Err(format!(
            "Box shorthand values must be space-separated, not comma-separated: '{}'",
            value
        ));
    }
    let parts: Vec<&str> = value.split_whitespace().collect();
    let lens: Vec<Length> = parts
        .iter()
        .map(|s| Length::from_src(s))
        .collect::<Result<_, _>>()?;
    match lens.as_slice() {
        [a] => Ok([*a, *a, *a, *a]),
        [a, b] => Ok([*a, *b, *a, *b]),
        [a, b, c] => Ok([*a, *b, *c, *b]),
        [a, b, c, d] => Ok([*a, *b, *c, *d]),
        _ => Err(format!(
            "Box shorthand expects 1 to 4 values, got {}: '{}'",
            lens.len(),
            value
        )),
    }
}

/// Parse a CSS length that resolves to a pixel `f32`. Accepts `<number>` and
/// `<number>px`; rejects percentages and `auto`. Used for scalar properties
/// like `border-width`, `border-radius`, and `font-size`.
fn parse_px_length(value: &str, property: &str) -> Result<f32, String> {
    match Length::from_src(value) {
        Ok(Length::Px(v)) => Ok(v),
        Ok(other) => Err(format!(
            "Invalid {property} value: '{value}' (expected a pixel length, got {other:?})",
        )),
        Err(e) => Err(format!("Invalid {property} value: '{value}' ({e})")),
    }
}

/// Extract the first font family from a CSS font-family value, stripping
/// surrounding quotes. Fallbacks (e.g. `"Inter", Arial, sans-serif`) are
/// tolerated for authoring but only the first entry is retained since the
/// renderer resolves against a single font asset.
fn parse_font_family(value: &str) -> Result<String, String> {
    let first = value.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        return Err(format!("Invalid font-family value: '{}'", value));
    }
    let unquoted = first
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| first.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(first);
    Ok(unquoted.to_string())
}

fn parse_visibility(value: &str) -> Result<bool, String> {
    match value {
        "visible" => Ok(true),
        "hidden" => Ok(false),
        _ => Err(format!("Unsupported visibility value: '{}'", value)),
    }
}

fn parse_pointer_events(value: &str) -> Result<bool, String> {
    match value {
        "auto" => Ok(true),
        "none" => Ok(false),
        _ => Err(format!("Unsupported pointer-events value: '{}'", value)),
    }
}

/// `max-width` / `max-height` uniquely accept `none` in CSS as "no upper
/// bound". Internally we fold it into `Length::Auto` since Taffy treats
/// `Auto` on `max_size` as "no constraint".
fn parse_max_length(value: &str) -> Result<Length, String> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(Length::Auto);
    }
    Length::from_src(value)
}

impl Display {
    pub fn from_src(src: &str) -> Result<Display, String> {
        match src {
            "flex" => Ok(Display::Flex),
            "none" => Ok(Display::None),
            _ => Err(format!("Unsupported display value: {}", src)),
        }
    }
}

impl Length {
    pub fn from_src(src: &str) -> Result<Length, String> {
        let mut input = ParserInput::new(src);
        let mut parser = Parser::new(&mut input);
        match parser.next() {
            Ok(Token::Percentage {
                has_sign,
                unit_value,
                ..
            }) => {
                if *has_sign {
                    return Err("Percentage values with sign are not supported".to_string());
                }
                Ok(Length::Percent(*unit_value))
            }
            Ok(Token::Dimension { value, unit, .. }) => {
                if unit.as_ref() != "px" {
                    return Err(format!("Unsupported unit: {}", unit));
                }
                Ok(Length::Px(*value))
            }
            Ok(Token::Number { value, .. }) => Ok(Length::Px(*value)),
            Ok(Token::Ident(ident)) if ident.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
            _ => Err(format!("Invalid length value: {}", src)),
        }
    }
}

impl FlexDirection {
    pub fn from_src(src: &str) -> Result<FlexDirection, String> {
        match src {
            "row" => Ok(FlexDirection::Row),
            "column" => Ok(FlexDirection::Column),
            _ => Err(format!("Unsupported flex-direction value: {}", src)),
        }
    }
}

impl AlignItems {
    pub fn from_src(src: &str) -> Result<AlignItems, String> {
        match src {
            "start" | "flex-start" => Ok(AlignItems::Start),
            "center" => Ok(AlignItems::Center),
            "end" | "flex-end" => Ok(AlignItems::End),
            "stretch" => Ok(AlignItems::Stretch),
            _ => Err(format!("Unsupported align-items value: {}", src)),
        }
    }
}

impl JustifyContent {
    pub fn from_src(src: &str) -> Result<JustifyContent, String> {
        match src {
            "start" | "flex-start" => Ok(JustifyContent::Start),
            "center" => Ok(JustifyContent::Center),
            "end" | "flex-end" => Ok(JustifyContent::End),
            "space-between" => Ok(JustifyContent::SpaceBetween),
            "space-around" => Ok(JustifyContent::SpaceAround),
            _ => Err(format!("Unsupported justify-content value: {}", src)),
        }
    }
}

impl TextAlign {
    pub fn from_src(src: &str) -> Result<TextAlign, String> {
        match src {
            "left" => Ok(TextAlign::Left),
            "center" => Ok(TextAlign::Center),
            "right" => Ok(TextAlign::Right),
            _ => Err(format!("Unsupported text-align value: {}", src)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_parsing() {
        assert_eq!(Length::from_src("10px").unwrap(), Length::Px(10.0));
        assert_eq!(Length::from_src("25%").unwrap(), Length::Percent(0.25));
        assert_eq!(Length::from_src("auto").unwrap(), Length::Auto);
    }

    #[test]
    fn test_length_parsing_invalid() {
        assert!(Length::from_src("10em").is_err());
        assert!(Length::from_src("-10%").is_err());
        assert!(Length::from_src("abc").is_err());
    }

    #[test]
    fn test_style_value_from_src() {
        match StyleValue::from_src("display", "none").unwrap() {
            StyleValue::Display(Display::None) => {}
            other => panic!("unexpected style value: {:?}", other),
        }

        match StyleValue::from_src("background-color", "red").unwrap() {
            StyleValue::BackgroundColor(color) => {
                assert_eq!(color, Color::rgb(255, 0, 0));
            }
            other => panic!("unexpected style value: {:?}", other),
        }

        match StyleValue::from_src("color", "#00ff00").unwrap() {
            StyleValue::TextColor(color) => {
                assert_eq!(color, Color::rgb(0, 255, 0));
            }
            other => panic!("unexpected style value: {:?}", other),
        }

        match StyleValue::from_src("opacity", "0.5").unwrap() {
            StyleValue::Opacity(value) => assert_eq!(value, 0.5),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn test_declaration_from_src() {
        let declarations = Declaration::from_src("width: 10px; opacity: 0.75;").unwrap();
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].property, "width");
        assert_eq!(declarations[1].property, "opacity");

        match declarations[0].value {
            StyleValue::Width(Length::Px(v)) => assert_eq!(v, 10.0),
            ref other => panic!("unexpected declaration value: {:?}", other),
        }

        match declarations[1].value {
            StyleValue::Opacity(v) => assert_eq!(v, 0.75),
            ref other => panic!("unexpected declaration value: {:?}", other),
        }
    }

    #[test]
    fn test_declaration_from_src_invalid() {
        assert!(Declaration::from_src("width 10px;").is_err());
        assert!(StyleValue::from_src("not-a-property", "1").is_err());
        assert!(StyleValue::from_src("font-size", "large").is_err());
    }

    fn assert_lengths_eq(actual: [Length; 4], expected: [Length; 4]) {
        assert_eq!(
            actual, expected,
            "\nleft:  {:?}\nright: {:?}",
            actual, expected
        );
    }

    #[test]
    fn margin_shorthand_one_value_applies_to_all_sides() {
        match StyleValue::from_src("margin", "10px").unwrap() {
            StyleValue::Margin(v) => assert_lengths_eq(v, [Length::Px(10.0); 4]),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn margin_shorthand_two_values_map_to_vertical_horizontal() {
        // "10px 20px" -> top=10, right=20, bottom=10, left=20
        match StyleValue::from_src("margin", "10px 20px").unwrap() {
            StyleValue::Margin(v) => assert_lengths_eq(
                v,
                [
                    Length::Px(10.0),
                    Length::Px(20.0),
                    Length::Px(10.0),
                    Length::Px(20.0),
                ],
            ),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn margin_shorthand_three_values_map_to_top_horizontal_bottom() {
        // "10px 20px 30px" -> top=10, right=20, bottom=30, left=20
        match StyleValue::from_src("margin", "10px 20px 30px").unwrap() {
            StyleValue::Margin(v) => assert_lengths_eq(
                v,
                [
                    Length::Px(10.0),
                    Length::Px(20.0),
                    Length::Px(30.0),
                    Length::Px(20.0),
                ],
            ),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn margin_shorthand_four_values_map_top_right_bottom_left() {
        match StyleValue::from_src("margin", "1px 2px 3px 4px").unwrap() {
            StyleValue::Margin(v) => assert_lengths_eq(
                v,
                [
                    Length::Px(1.0),
                    Length::Px(2.0),
                    Length::Px(3.0),
                    Length::Px(4.0),
                ],
            ),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn padding_shorthand_matches_margin_rules() {
        match StyleValue::from_src("padding", "5px 6px 7px").unwrap() {
            StyleValue::Padding(v) => assert_lengths_eq(
                v,
                [
                    Length::Px(5.0),
                    Length::Px(6.0),
                    Length::Px(7.0),
                    Length::Px(6.0),
                ],
            ),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn box_shorthand_accepts_mixed_units_and_extra_whitespace() {
        match StyleValue::from_src("margin", "  10px   50%  auto  0  ").unwrap() {
            StyleValue::Margin(v) => assert_lengths_eq(
                v,
                [
                    Length::Px(10.0),
                    Length::Percent(0.5),
                    Length::Auto,
                    Length::Px(0.0),
                ],
            ),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn box_shorthand_rejects_zero_or_five_values() {
        assert!(StyleValue::from_src("margin", "").is_err());
        assert!(StyleValue::from_src("padding", "1px 2px 3px 4px 5px").is_err());
    }

    #[test]
    fn box_shorthand_rejects_commas() {
        // Reject old non-standard comma-separated syntax so callers migrate.
        assert!(StyleValue::from_src("margin", "10px, 20px").is_err());
        assert!(StyleValue::from_src("padding", "10px,20px,30px,40px").is_err());
    }

    #[test]
    fn directional_margin_properties_target_expected_side() {
        for (prop, expected_side) in [
            ("margin-top", Side::Top),
            ("margin-right", Side::Right),
            ("margin-bottom", Side::Bottom),
            ("margin-left", Side::Left),
        ] {
            match StyleValue::from_src(prop, "12px").unwrap() {
                StyleValue::MarginSide(side, Length::Px(v)) => {
                    assert_eq!(side, expected_side, "{prop}");
                    assert_eq!(v, 12.0);
                }
                other => panic!("{prop}: unexpected style value: {:?}", other),
            }
        }
    }

    #[test]
    fn directional_padding_properties_target_expected_side() {
        for (prop, expected_side) in [
            ("padding-top", Side::Top),
            ("padding-right", Side::Right),
            ("padding-bottom", Side::Bottom),
            ("padding-left", Side::Left),
        ] {
            match StyleValue::from_src(prop, "8px").unwrap() {
                StyleValue::PaddingSide(side, Length::Px(v)) => {
                    assert_eq!(side, expected_side, "{prop}");
                    assert_eq!(v, 8.0);
                }
                other => panic!("{prop}: unexpected style value: {:?}", other),
            }
        }
    }

    // ---------- scalar `px` properties accept the px unit ----------

    /// Regression: `border-width: 2px` used to fail because the parser called
    /// `value.parse::<f32>()` which rejects the `px` suffix.
    #[test]
    fn border_width_accepts_px_and_unitless() {
        match StyleValue::from_src("border-width", "2px").unwrap() {
            StyleValue::BorderWidth(v) => assert_eq!(v, 2.0),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("border-width", "3").unwrap() {
            StyleValue::BorderWidth(v) => assert_eq!(v, 3.0),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn border_radius_accepts_px_and_unitless() {
        match StyleValue::from_src("border-radius", "16px").unwrap() {
            StyleValue::BorderRadius(v) => assert_eq!(v, 16.0),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("border-radius", "8").unwrap() {
            StyleValue::BorderRadius(v) => assert_eq!(v, 8.0),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn font_size_accepts_px_and_unitless() {
        match StyleValue::from_src("font-size", "32px").unwrap() {
            StyleValue::FontSize(v) => assert_eq!(v, 32.0),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("font-size", "40").unwrap() {
            StyleValue::FontSize(v) => assert_eq!(v, 40.0),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    /// Percentages and `auto` are not valid pixel lengths and must be rejected
    /// with a helpful error mentioning the offending value.
    #[test]
    fn px_length_properties_reject_percent_and_auto() {
        assert!(StyleValue::from_src("border-width", "50%").is_err());
        assert!(StyleValue::from_src("border-radius", "auto").is_err());
        assert!(StyleValue::from_src("font-size", "10em").is_err());
    }

    // ---------- visibility / pointer-events (previously unparseable) ----------

    #[test]
    fn visibility_accepts_visible_and_hidden() {
        match StyleValue::from_src("visibility", "visible").unwrap() {
            StyleValue::Visibility(v) => assert!(v),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("visibility", "hidden").unwrap() {
            StyleValue::Visibility(v) => assert!(!v),
            other => panic!("unexpected style value: {:?}", other),
        }
        assert!(StyleValue::from_src("visibility", "collapse").is_err());
    }

    #[test]
    fn pointer_events_accepts_auto_and_none() {
        match StyleValue::from_src("pointer-events", "auto").unwrap() {
            StyleValue::PointerEvents(v) => assert!(v),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("pointer-events", "none").unwrap() {
            StyleValue::PointerEvents(v) => assert!(!v),
            other => panic!("unexpected style value: {:?}", other),
        }
        assert!(StyleValue::from_src("pointer-events", "all").is_err());
    }

    // ---------- CSS keyword aliases ----------

    /// Both `start` and `flex-start` (and both `end` and `flex-end`) are valid
    /// per the modern CSS spec; the parser must accept both spellings.
    #[test]
    fn align_items_accepts_flex_start_and_flex_end_aliases() {
        for (input, expected) in [
            ("start", AlignItems::Start),
            ("flex-start", AlignItems::Start),
            ("end", AlignItems::End),
            ("flex-end", AlignItems::End),
            ("center", AlignItems::Center),
            ("stretch", AlignItems::Stretch),
        ] {
            match StyleValue::from_src("align-items", input).unwrap() {
                StyleValue::AlignItems(v) => assert_eq!(v, expected, "{input}"),
                other => panic!("{input}: unexpected style value: {:?}", other),
            }
        }
    }

    #[test]
    fn justify_content_accepts_flex_start_and_flex_end_aliases() {
        for (input, expected) in [
            ("start", JustifyContent::Start),
            ("flex-start", JustifyContent::Start),
            ("end", JustifyContent::End),
            ("flex-end", JustifyContent::End),
            ("center", JustifyContent::Center),
            ("space-between", JustifyContent::SpaceBetween),
            ("space-around", JustifyContent::SpaceAround),
        ] {
            match StyleValue::from_src("justify-content", input).unwrap() {
                StyleValue::JustifyContent(v) => assert_eq!(v, expected, "{input}"),
                other => panic!("{input}: unexpected style value: {:?}", other),
            }
        }
    }

    // ---------- `none` as an alias for `Auto` on max-* only ----------

    #[test]
    fn max_dimensions_accept_none_keyword() {
        match StyleValue::from_src("max-width", "none").unwrap() {
            StyleValue::MaxWidth(Length::Auto) => {}
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("max-height", "none").unwrap() {
            StyleValue::MaxHeight(Length::Auto) => {}
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    /// `none` is only valid on max-width/max-height in CSS; width/height must
    /// still reject it so authoring typos surface as errors.
    #[test]
    fn width_and_height_reject_none_keyword() {
        assert!(StyleValue::from_src("width", "none").is_err());
        assert!(StyleValue::from_src("height", "none").is_err());
    }

    // ---------- font-family strips quotes and takes first fallback ----------

    #[test]
    fn font_family_strips_double_and_single_quotes() {
        match StyleValue::from_src("font-family", "\"helvetica neue\"").unwrap() {
            StyleValue::FontFamily(name) => assert_eq!(name, "helvetica neue"),
            other => panic!("unexpected style value: {:?}", other),
        }
        match StyleValue::from_src("font-family", "'inter'").unwrap() {
            StyleValue::FontFamily(name) => assert_eq!(name, "inter"),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn font_family_takes_first_of_comma_separated_fallback_list() {
        match StyleValue::from_src("font-family", "\"inter\", arial, sans-serif").unwrap() {
            StyleValue::FontFamily(name) => assert_eq!(name, "inter"),
            other => panic!("unexpected style value: {:?}", other),
        }
    }

    #[test]
    fn font_family_rejects_empty_value() {
        assert!(StyleValue::from_src("font-family", "").is_err());
    }
}
