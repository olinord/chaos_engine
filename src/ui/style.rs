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
    Padding([Length; 4]),
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
            "max-width" => Ok(StyleValue::MaxWidth(Length::from_src(value)?)),
            "max-height" => Ok(StyleValue::MaxHeight(Length::from_src(value)?)),
            "margin" => {
                let mut lengths = value
                    .split(",")
                    .map(|s| Length::from_src(s.trim()))
                    .collect::<Result<Vec<_>, _>>()?;
                // fill lengths to 4 values
                while lengths.len() < 4 {
                    lengths.push(Length::Px(0.0));
                }
                Ok(StyleValue::Margin(lengths[0..4].try_into().unwrap()))
            }
            "padding" => {
                let mut lengths = value
                    .split(",")
                    .map(|s| Length::from_src(s.trim()))
                    .collect::<Result<Vec<_>, _>>()?;
                // fill lengths to 4 values
                while lengths.len() < 4 {
                    lengths.push(Length::Px(0.0));
                }
                Ok(StyleValue::Padding(lengths[0..4].try_into().unwrap()))
            }
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
            "border-width" => {
                Ok(StyleValue::BorderWidth(value.parse::<f32>().map_err(
                    |_| format!("Invalid border-width value: {}", value),
                )?))
            }
            "border-radius" => {
                Ok(StyleValue::BorderRadius(value.parse::<f32>().map_err(
                    |_| format!("Invalid border-radius value: {}", value),
                )?))
            }
            "opacity" => {
                Ok(StyleValue::Opacity(value.parse::<f32>().map_err(|_| {
                    format!("Invalid opacity value: {}", value)
                })?))
            }
            "font-family" => Ok(StyleValue::FontFamily(value.to_string())),
            "font-size" => {
                Ok(StyleValue::FontSize(value.parse::<f32>().map_err(
                    |_| format!("Invalid font-size value: {}", value),
                )?))
            }
            "color" => Ok(StyleValue::TextColor(Color::from_src(value)?)),
            "text-align" => Ok(StyleValue::TextAlign(TextAlign::from_src(value)?)),
            "z-index" => {
                Ok(StyleValue::ZIndex(value.parse::<i32>().map_err(|_| {
                    format!("Invalid z-index value: {}", value)
                })?))
            }
            "overflow" => Ok(StyleValue::Overflow(Overflow::from_src(value)?)),
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
            "start" => Ok(AlignItems::Start),
            "center" => Ok(AlignItems::Center),
            "end" => Ok(AlignItems::End),
            "stretch" => Ok(AlignItems::Stretch),
            _ => Err(format!("Unsupported align-items value: {}", src)),
        }
    }
}

impl JustifyContent {
    pub fn from_src(src: &str) -> Result<JustifyContent, String> {
        match src {
            "start" => Ok(JustifyContent::Start),
            "center" => Ok(JustifyContent::Center),
            "end" => Ok(JustifyContent::End),
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
}
