use cssparser::ParserInput;
use cssparser::{Parser, Token};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn from_src(src: &str) -> Result<Color, String> {
        let mut input = ParserInput::new(src);
        let mut parser = Parser::new(&mut input);
        match parser.next() {
            Ok(Token::Function(name)) => {
                if !src.trim_end().ends_with(')') {
                    return Err(format!("Unclosed color function: {}", src));
                }

                let function_name = name.to_string();

                let values =
                    parser
                        .parse_nested_block(|nested_parser| {
                            let mut values = Vec::<f32>::new();

                            while let Ok(token) = nested_parser.next() {
                                match token {
                                    Token::Number { value, .. } => values.push(*value),
                                    Token::Percentage { unit_value, .. } => {
                                        values.push(*unit_value)
                                    }
                                    Token::Comma => continue,
                                    _ => break,
                                }
                            }

                            Ok::<
                                Vec<f32>,
                                cssparser::ParseError<'_, cssparser::BasicParseErrorKind<'_>>,
                            >(values)
                        })
                        .map_err(|_| format!("Unexpected token in color function: {}", src))?;

                match function_name.as_str() {
                    "rgb" => Color::from_rgb(&values),
                    "rgba" => Color::from_rgba(&values),
                    "hsl" => Color::from_hsl(&values),
                    "hsla" => Color::from_hsla(&values),
                    _ => Err(format!("Unsupported color function: {}", function_name)),
                }
            }
            Ok(Token::Hash(hash)) | Ok(Token::IDHash(hash)) => {
                let hex = hash.to_string();
                if hex.len() == 6 || hex.len() == 8 {
                    let r = u8::from_str_radix(&hex[0..2], 16)
                        .map_err(|_| format!("Invalid hex color value: {}", &hex[0..2]))?;
                    let g = u8::from_str_radix(&hex[2..4], 16)
                        .map_err(|_| format!("Invalid hex color value: {}", &hex[2..4]))?;
                    let b = u8::from_str_radix(&hex[4..6], 16)
                        .map_err(|_| format!("Invalid hex color value: {}", &hex[4..6]))?;

                    let a = if hex.len() == 8 {
                        u8::from_str_radix(&hex[6..8], 16)
                            .map_err(|_| format!("Invalid hex color value: {}", &hex[6..8]))?
                    } else {
                        255
                    };
                    Ok(Color {
                        r: r as f32 / 255.0,
                        g: g as f32 / 255.0,
                        b: b as f32 / 255.0,
                        a: a as f32 / 255.0,
                    })
                } else {
                    Err(format!("Invalid hex color value: {}", hex))
                }
            }
            Ok(Token::Ident(name)) => {
                let color_name = name.to_string().to_lowercase();
                let found_color = colors::color_from_name(color_name.as_str());
                match found_color {
                    Some(color) => Ok(color),
                    None => Err(format!("Unknown color name: {}", color_name)),
                }
            }
            _ => Err(format!("Invalid color value: {}", src)),
        }
    }

    fn hsl_to_rgb(h: f32, s: f32, l: f32, a: f32) -> Color {
        let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let h_prime = h / 60.0;
        let x = chroma * (1.0 - (h_prime % 2.0 - 1.0).abs());
        let (r1, g1, b1) = match h_prime as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            5 => (chroma, 0.0, x),
            _ => (0.0, 0.0, 0.0),
        };
        let m = l - 0.5 * chroma;
        let r = r1 + m;
        let g = g1 + m;
        let b = b1 + m;
        Color { r, g, b, a }
    }

    fn from_rgb(values: &Vec<f32>) -> Result<Color, String> {
        if values.len() != 3 {
            return Err(format!("RGB color requires 3 values, got {}", values.len()));
        }
        for &v in values {
            if v > 255.0 || v < 0.0 {
                return Err(format!(
                    "RGB color values must be in the range [0, 255], got {:?}",
                    values
                ));
            }
        }
        Ok(Color {
            r: values[0] as f32 / 255.0,
            g: values[1] as f32 / 255.0,
            b: values[2] as f32 / 255.0,
            a: 1.0,
        })
    }

    fn from_rgba(values: &Vec<f32>) -> Result<Color, String> {
        if values.len() != 4 {
            return Err(format!(
                "RGBA color requires 4 values, got {}",
                values.len()
            ));
        }
        for &v in values {
            if v > 255.0 || v < 0.0 {
                return Err(format!(
                    "RGBA color values must be in the range [0, 255], got {:?}",
                    values
                ));
            }
        }
        Ok(Color {
            r: values[0] as f32 / 255.0,
            g: values[1] as f32 / 255.0,
            b: values[2] as f32 / 255.0,
            a: values[3],
        })
    }

    fn from_hsl(values: &Vec<f32>) -> Result<Color, String> {
        if values.len() != 3 {
            return Err(format!("HSL color requires 3 values, got {:?}", values));
        }
        let h = values[0];
        let s = values[1];
        let l = values[2];
        if h < 0.0 || h > 360.0 {
            return Err(format!(
                "HSL hue value must be in the range [0, 360], got {}",
                h
            ));
        }
        if s < 0.0 || s > 1.0 {
            return Err(format!(
                "HSL saturation value must be in the range [0, 1], got {}",
                s
            ));
        }
        if l < 0.0 || l > 1.0 {
            return Err(format!(
                "HSL lightness value must be in the range [0, 1], got {}",
                l
            ));
        }
        Ok(Color::hsl_to_rgb(h, s, l, 1.0))
    }

    fn from_hsla(values: &Vec<f32>) -> Result<Color, String> {
        if values.len() != 4 {
            return Err(format!("HSLA color requires 4 values, got {:?}", values));
        }
        let h = values[0];
        let s = values[1];
        let l = values[2];
        let a = values[3];
        if h < 0.0 || h > 360.0 {
            return Err(format!(
                "HSLA hue value must be in the range [0, 360], got {}",
                h
            ));
        }
        if s < 0.0 || s > 1.0 {
            return Err(format!(
                "HSLA saturation value must be in the range [0, 1], got {}",
                s
            ));
        }
        if l < 0.0 || l > 1.0 {
            return Err(format!(
                "HSLA lightness value must be in the range [0, 1], got {}",
                l
            ));
        }
        if a < 0.0 || a > 1.0 {
            return Err(format!(
                "HSLA alpha value must be in the range [0, 1], got {}",
                a
            ));
        }
        let mut color = Color::hsl_to_rgb(h, s, l, 1.0);
        color.a = a;
        Ok(color)
    }
}

pub mod colors {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use super::Color;

    // Red colors
    pub const INDIAN_RED: Color = Color::rgb(205, 92, 92);
    pub const LIGHT_CORAL: Color = Color::rgb(240, 128, 128);
    pub const SALMON: Color = Color::rgb(250, 128, 114);
    pub const DARK_SALMON: Color = Color::rgb(233, 150, 122);
    pub const LIGHT_SALMON: Color = Color::rgb(255, 160, 122);
    pub const CRIMSON: Color = Color::rgb(220, 20, 60);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const FIRE_BRICK: Color = Color::rgb(178, 34, 34);
    pub const DARK_RED: Color = Color::rgb(139, 0, 0);

    // Pink colors
    pub const PINK: Color = Color::rgb(255, 192, 203);
    pub const LIGHT_PINK: Color = Color::rgb(255, 182, 193);
    pub const HOT_PINK: Color = Color::rgb(255, 105, 180);
    pub const DEEP_PINK: Color = Color::rgb(255, 20, 147);
    pub const MEDIUM_VIOLET_RED: Color = Color::rgb(199, 21, 133);
    pub const PALE_VIOLET_RED: Color = Color::rgb(219, 112, 147);

    // Orange colors
    pub const CORAL: Color = Color::rgb(255, 127, 80);
    pub const TOMATO: Color = Color::rgb(255, 99, 71);
    pub const ORANGE_RED: Color = Color::rgb(255, 69, 0);
    pub const DARK_ORANGE: Color = Color::rgb(255, 140, 0);
    pub const ORANGE: Color = Color::rgb(255, 165, 0);

    // Yellow colors
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const LIGHT_YELLOW: Color = Color::rgb(255, 255, 224);
    pub const LIGHT_GOLDENROD_YELLOW: Color = Color::rgb(250, 250, 210);
    pub const LEMON_CHIFFON: Color = Color::rgb(255, 250, 205);
    pub const PAPAYA_WHIP: Color = Color::rgb(255, 239, 213);
    pub const MOCCASIN: Color = Color::rgb(255, 228, 181);
    pub const PEACH_PUFF: Color = Color::rgb(255, 218, 185);
    pub const PALE_GOLDENROD: Color = Color::rgb(238, 232, 170);
    pub const KHAKI: Color = Color::rgb(240, 230, 140);
    pub const DARK_KHAKI: Color = Color::rgb(189, 183, 107);
    pub const GOLD: Color = Color::rgb(255, 215, 0);

    // Brown colors
    pub const CORNSILK: Color = Color::rgb(255, 248, 220);
    pub const BLANCHED_ALMOND: Color = Color::rgb(255, 235, 205);
    pub const BISQUE: Color = Color::rgb(255, 228, 196);
    pub const NAVAJO_WHITE: Color = Color::rgb(255, 222, 173);
    pub const WHEAT: Color = Color::rgb(245, 222, 179);
    pub const BURLYWOOD: Color = Color::rgb(222, 184, 135);
    pub const TAN: Color = Color::rgb(210, 180, 140);
    pub const ROSY_BROWN: Color = Color::rgb(188, 143, 143);
    pub const SANDY_BROWN: Color = Color::rgb(244, 164, 96);
    pub const GOLDENROD: Color = Color::rgb(218, 165, 32);
    pub const DARK_GOLDENROD: Color = Color::rgb(184, 134, 11);
    pub const PERU: Color = Color::rgb(205, 133, 63);
    pub const CHOCOLATE: Color = Color::rgb(210, 105, 30);
    pub const SADDLE_BROWN: Color = Color::rgb(139, 69, 19);
    pub const SIENNA: Color = Color::rgb(160, 82, 45);
    pub const BROWN: Color = Color::rgb(165, 42, 42);
    pub const MAROON: Color = Color::rgb(128, 0, 0);

    // Green colors
    pub const DARK_OLIVE_GREEN: Color = Color::rgb(85, 107, 47);
    pub const OLIVE: Color = Color::rgb(128, 128, 0);
    pub const OLIVE_DRAB: Color = Color::rgb(107, 142, 35);
    pub const YELLOW_GREEN: Color = Color::rgb(154, 205, 50);
    pub const LIME_GREEN: Color = Color::rgb(50, 205, 50);
    pub const LIME: Color = Color::rgb(0, 255, 0);
    pub const LAWN_GREEN: Color = Color::rgb(124, 252, 0);
    pub const CHARTREUSE: Color = Color::rgb(127, 255, 0);
    pub const GREEN_YELLOW: Color = Color::rgb(173, 255, 47);
    pub const SPRING_GREEN: Color = Color::rgb(0, 255, 127);
    pub const MEDIUM_SPRING_GREEN: Color = Color::rgb(0, 250, 154);
    pub const LIGHT_GREEN: Color = Color::rgb(144, 238, 144);
    pub const PALE_GREEN: Color = Color::rgb(152, 251, 152);
    pub const DARK_SEA_GREEN: Color = Color::rgb(143, 188, 143);
    pub const MEDIUM_SEA_GREEN: Color = Color::rgb(60, 179, 113);
    pub const SEA_GREEN: Color = Color::rgb(46, 139, 87);
    pub const FOREST_GREEN: Color = Color::rgb(34, 139, 34);
    pub const GREEN: Color = Color::rgb(0, 128, 0);
    pub const DARK_GREEN: Color = Color::rgb(0, 100, 0);

    // Cyan colors
    pub const MEDIUM_AQUAMARINE: Color = Color::rgb(102, 205, 170);
    pub const AQUA: Color = Color::rgb(0, 255, 255);
    pub const CYAN: Color = Color::rgb(0, 255, 255);
    pub const LIGHT_CYAN: Color = Color::rgb(224, 255, 255);
    pub const PALE_TURQUOISE: Color = Color::rgb(175, 238, 238);
    pub const AQUAMARINE: Color = Color::rgb(127, 255, 212);
    pub const TURQUOISE: Color = Color::rgb(64, 224, 208);
    pub const MEDIUM_TURQUOISE: Color = Color::rgb(72, 209, 204);
    pub const DARK_TURQUOISE: Color = Color::rgb(0, 206, 209);
    pub const LIGHT_SEA_GREEN: Color = Color::rgb(32, 178, 170);
    pub const CADET_BLUE: Color = Color::rgb(95, 158, 160);
    pub const DARK_CYAN: Color = Color::rgb(0, 139, 139);
    pub const TEAL: Color = Color::rgb(0, 128, 128);

    // Blue colors
    pub const LIGHT_STEEL_BLUE: Color = Color::rgb(176, 196, 222);
    pub const POWDER_BLUE: Color = Color::rgb(176, 224, 230);
    pub const LIGHT_BLUE: Color = Color::rgb(173, 216, 230);
    pub const SKY_BLUE: Color = Color::rgb(135, 206, 235);
    pub const LIGHT_SKY_BLUE: Color = Color::rgb(135, 206, 250);
    pub const DEEP_SKY_BLUE: Color = Color::rgb(0, 191, 255);
    pub const DODGER_BLUE: Color = Color::rgb(30, 144, 255);
    pub const CORNFLOWER_BLUE: Color = Color::rgb(100, 149, 237);
    pub const STEEL_BLUE: Color = Color::rgb(70, 130, 180);
    pub const ROYAL_BLUE: Color = Color::rgb(65, 105, 225);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const MEDIUM_BLUE: Color = Color::rgb(0, 0, 205);
    pub const DARK_BLUE: Color = Color::rgb(0, 0, 139);
    pub const NAVY: Color = Color::rgb(0, 0, 128);
    pub const MIDNIGHT_BLUE: Color = Color::rgb(25, 25, 112);

    // Purple colors
    pub const LAVENDER: Color = Color::rgb(230, 230, 250);
    pub const THISTLE: Color = Color::rgb(216, 191, 216);
    pub const PLUM: Color = Color::rgb(221, 160, 221);
    pub const VIOLET: Color = Color::rgb(238, 130, 238);
    pub const ORCHID: Color = Color::rgb(218, 112, 214);
    pub const FUCHSIA: Color = Color::rgb(255, 0, 255);
    pub const MAGENTA: Color = Color::rgb(255, 0, 255);
    pub const MEDIUM_ORCHID: Color = Color::rgb(186, 85, 211);
    pub const MEDIUM_PURPLE: Color = Color::rgb(147, 112, 219);
    pub const BLUE_VIOLET: Color = Color::rgb(138, 43, 226);
    pub const DARK_VIOLET: Color = Color::rgb(148, 0, 211);
    pub const DARK_ORCHID: Color = Color::rgb(153, 50, 204);
    pub const DARK_MAGENTA: Color = Color::rgb(139, 0, 139);
    pub const PURPLE: Color = Color::rgb(128, 0, 128);
    pub const REBECCA_PURPLE: Color = Color::rgb(102, 51, 153);
    pub const INDIGO: Color = Color::rgb(75, 0, 130);
    pub const MEDIUM_SLATE_BLUE: Color = Color::rgb(123, 104, 238);
    pub const SLATE_BLUE: Color = Color::rgb(106, 90, 205);
    pub const DARK_SLATE_BLUE: Color = Color::rgb(72, 61, 139);

    // White colors
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const SNOW: Color = Color::rgb(255, 250, 250);
    pub const HONEYDEW: Color = Color::rgb(240, 255, 240);
    pub const AZURE: Color = Color::rgb(240, 255, 255);
    pub const MINT_CREAM: Color = Color::rgb(245, 255, 250);
    pub const ALICE_BLUE: Color = Color::rgb(240, 248, 255);
    pub const GHOST_WHITE: Color = Color::rgb(248, 248, 255);
    pub const WHITE_SMOKE: Color = Color::rgb(245, 245, 245);
    pub const SEASHELL: Color = Color::rgb(255, 245, 238);
    pub const BEIGE: Color = Color::rgb(245, 245, 220);
    pub const OLD_LACE: Color = Color::rgb(253, 245, 230);
    pub const FLORAL_WHITE: Color = Color::rgb(255, 250, 240);
    pub const IVORY: Color = Color::rgb(255, 255, 240);
    pub const ANTIQUE_WHITE: Color = Color::rgb(250, 235, 215);
    pub const LINEN: Color = Color::rgb(250, 240, 230);
    pub const LAVENDER_BLUSH: Color = Color::rgb(255, 240, 245);
    pub const MISTY_ROSE: Color = Color::rgb(255, 228, 225);

    // Gray colors
    pub const GAINSBORO: Color = Color::rgb(220, 220, 220);
    pub const LIGHT_GRAY: Color = Color::rgb(211, 211, 211);
    pub const LIGHT_GREY: Color = Color::rgb(211, 211, 211);
    pub const SILVER: Color = Color::rgb(192, 192, 192);
    pub const DARK_GRAY: Color = Color::rgb(169, 169, 169);
    pub const DARK_GREY: Color = Color::rgb(169, 169, 169);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const GREY: Color = Color::rgb(128, 128, 128);
    pub const DIM_GRAY: Color = Color::rgb(105, 105, 105);
    pub const DIM_GREY: Color = Color::rgb(105, 105, 105);
    pub const LIGHT_SLATE_GRAY: Color = Color::rgb(119, 136, 153);
    pub const LIGHT_SLATE_GREY: Color = Color::rgb(119, 136, 153);
    pub const SLATE_GRAY: Color = Color::rgb(112, 128, 144);
    pub const SLATE_GREY: Color = Color::rgb(112, 128, 144);
    pub const DARK_SLATE_GRAY: Color = Color::rgb(47, 79, 79);
    pub const DARK_SLATE_GREY: Color = Color::rgb(47, 79, 79);
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub static COLOR_MAP: LazyLock<HashMap<&'static str, Color>> = LazyLock::new(|| {
        HashMap::from([
            ("alice_blue", ALICE_BLUE),
            ("antique_white", ANTIQUE_WHITE),
            ("aqua", AQUA),
            ("aquamarine", AQUAMARINE),
            ("azure", AZURE),
            ("beige", BEIGE),
            ("bisque", BISQUE),
            ("black", BLACK),
            ("blanched_almond", BLANCHED_ALMOND),
            ("blue", BLUE),
            ("blue_violet", BLUE_VIOLET),
            ("brown", BROWN),
            ("burlywood", BURLYWOOD),
            ("cadet_blue", CADET_BLUE),
            ("chartreuse", CHARTREUSE),
            ("chocolate", CHOCOLATE),
            ("coral", CORAL),
            ("cornflower_blue", CORNFLOWER_BLUE),
            ("cornsilk", CORNSILK),
            ("crimson", CRIMSON),
            ("cyan", CYAN),
            ("dark_blue", DARK_BLUE),
            ("dark_cyan", DARK_CYAN),
            ("dark_goldenrod", DARK_GOLDENROD),
            ("dark_gray", DARK_GRAY),
            ("dark_grey", DARK_GREY),
            ("dark_green", DARK_GREEN),
            ("dark_khaki", DARK_KHAKI),
            ("dark_magenta", DARK_MAGENTA),
            ("dark_olive_green", DARK_OLIVE_GREEN),
            ("dark_orange", DARK_ORANGE),
            ("dark_orchid", DARK_ORCHID),
            ("dark_red", DARK_RED),
            ("dark_salmon", DARK_SALMON),
            ("dark_sea_green", DARK_SEA_GREEN),
            ("dark_slate_blue", DARK_SLATE_BLUE),
            ("dark_slate_gray", DARK_SLATE_GRAY),
            ("dark_slate_grey", DARK_SLATE_GREY),
            ("dark_turquoise", DARK_TURQUOISE),
            ("dark_violet", DARK_VIOLET),
            ("deep_pink", DEEP_PINK),
            ("deep_sky_blue", DEEP_SKY_BLUE),
            ("dim_gray", DIM_GRAY),
            ("dim_grey", DIM_GREY),
            ("dodger_blue", DODGER_BLUE),
            ("fire_brick", FIRE_BRICK),
            ("floral_white", FLORAL_WHITE),
            ("forest_green", FOREST_GREEN),
            ("fuchsia", FUCHSIA),
            ("gainsboro", GAINSBORO),
            ("ghost_white", GHOST_WHITE),
            ("gold", GOLD),
            ("goldenrod", GOLDENROD),
            ("gray", GRAY),
            ("grey", GREY),
            ("green", GREEN),
            ("green_yellow", GREEN_YELLOW),
            ("honeydew", HONEYDEW),
            ("hot_pink", HOT_PINK),
            ("indian_red", INDIAN_RED),
            ("indigo", INDIGO),
            ("ivory", IVORY),
            ("khaki", KHAKI),
            ("lavender", LAVENDER),
            ("lavender_blush", LAVENDER_BLUSH),
            ("lawn_green", LAWN_GREEN),
            ("lemon_chiffon", LEMON_CHIFFON),
            ("light_blue", LIGHT_BLUE),
            ("light_coral", LIGHT_CORAL),
            ("light_cyan", LIGHT_CYAN),
            ("light_goldenrod_yellow", LIGHT_GOLDENROD_YELLOW),
            ("light_gray", LIGHT_GRAY),
            ("light_grey", LIGHT_GREY),
            ("light_green", LIGHT_GREEN),
            ("light_pink", LIGHT_PINK),
            ("light_salmon", LIGHT_SALMON),
            ("light_sea_green", LIGHT_SEA_GREEN),
            ("light_sky_blue", LIGHT_SKY_BLUE),
            ("light_slate_gray", LIGHT_SLATE_GRAY),
            ("light_slate_grey", LIGHT_SLATE_GREY),
            ("light_steel_blue", LIGHT_STEEL_BLUE),
            ("light_yellow", LIGHT_YELLOW),
            ("lime", LIME),
            ("lime_green", LIME_GREEN),
            ("linen", LINEN),
            ("magenta", MAGENTA),
            ("maroon", MAROON),
            ("medium_aquamarine", MEDIUM_AQUAMARINE),
            ("medium_blue", MEDIUM_BLUE),
            ("medium_orchid", MEDIUM_ORCHID),
            ("medium_purple", MEDIUM_PURPLE),
            ("medium_sea_green", MEDIUM_SEA_GREEN),
            ("medium_slate_blue", MEDIUM_SLATE_BLUE),
            ("medium_spring_green", MEDIUM_SPRING_GREEN),
            ("medium_turquoise", MEDIUM_TURQUOISE),
            ("medium_violet_red", MEDIUM_VIOLET_RED),
            ("midnight_blue", MIDNIGHT_BLUE),
            ("mint_cream", MINT_CREAM),
            ("misty_rose", MISTY_ROSE),
            ("moccasin", MOCCASIN),
            ("navajo_white", NAVAJO_WHITE),
            ("navy", NAVY),
            ("old_lace", OLD_LACE),
            ("olive", OLIVE),
            ("olive_drab", OLIVE_DRAB),
            ("orange", ORANGE),
            ("orange_red", ORANGE_RED),
            ("orchid", ORCHID),
            ("pale_goldenrod", PALE_GOLDENROD),
            ("pale_green", PALE_GREEN),
            ("pale_turquoise", PALE_TURQUOISE),
            ("pale_violet_red", PALE_VIOLET_RED),
            ("papaya_whip", PAPAYA_WHIP),
            ("peach_puff", PEACH_PUFF),
            ("peru", PERU),
            ("pink", PINK),
            ("plum", PLUM),
            ("powder_blue", POWDER_BLUE),
            ("purple", PURPLE),
            ("rebecca_purple", REBECCA_PURPLE),
            ("red", RED),
            ("rosy_brown", ROSY_BROWN),
            ("royal_blue", ROYAL_BLUE),
            ("saddle_brown", SADDLE_BROWN),
            ("salmon", SALMON),
            ("sandy_brown", SANDY_BROWN),
            ("sea_green", SEA_GREEN),
            ("seashell", SEASHELL),
            ("sienna", SIENNA),
            ("silver", SILVER),
            ("sky_blue", SKY_BLUE),
            ("slate_blue", SLATE_BLUE),
            ("slate_gray", SLATE_GRAY),
            ("slate_grey", SLATE_GREY),
            ("snow", SNOW),
            ("spring_green", SPRING_GREEN),
            ("steel_blue", STEEL_BLUE),
            ("tan", TAN),
            ("teal", TEAL),
            ("thistle", THISTLE),
            ("tomato", TOMATO),
            ("turquoise", TURQUOISE),
            ("violet", VIOLET),
            ("wheat", WHEAT),
            ("white", WHITE),
            ("white_smoke", WHITE_SMOKE),
            ("yellow", YELLOW),
            ("yellow_green", YELLOW_GREEN),
        ])
    });

    pub fn color_from_name(name: &str) -> Option<Color> {
        COLOR_MAP.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parsing() {
        let color = Color::from_src("rgb(255, 0, 0)").unwrap();
        assert_eq!(
            color,
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0
            }
        );

        let color = Color::from_src("rgba(0, 255, 0, 0.5)").unwrap();
        assert_eq!(
            color,
            Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 0.5
            }
        );

        let color = Color::from_src("hsl(240, 1, 0.5)").unwrap();
        assert_eq!(
            color,
            Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0
            }
        );

        let color = Color::from_src("hsla(60, 1, 0.5, 0.25)").unwrap();
        assert_eq!(
            color,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 0.25
            }
        );

        let color = Color::from_src("#FF00FF").unwrap();
        assert_eq!(
            color,
            Color {
                r: 1.0,
                g: 0.0,
                b: 1.0,
                a: 1.0
            }
        );

        let color = Color::from_src("#00FFFFAA").unwrap();
        assert_eq!(
            color,
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 0.6666667
            }
        );

        assert!(Color::from_src("rgb()").is_err());
        assert!(Color::from_src("rgb(1)").is_err());
        assert!(Color::from_src("rgb(1, 2)").is_err());
        assert!(Color::from_src("rgb(1, 2, 3, 4)").is_err());
        assert!(Color::from_src("rgb(-1, 2, 3)").is_err());
        assert!(Color::from_src("rgb(1, 2, 3").is_err());
    }
}
