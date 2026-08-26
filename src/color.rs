use std::{collections::HashSet, fmt, str::FromStr};

/// An opaque 24-bit RGB color.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub const fn from_u32(value: u32) -> Self {
        Self {
            red: ((value >> 16) & 0xff) as u8,
            green: ((value >> 8) & 0xff) as u8,
            blue: (value & 0xff) as u8,
        }
    }

    pub const fn to_u32(self) -> u32 {
        ((self.red as u32) << 16) | ((self.green as u32) << 8) | self.blue as u32
    }

    pub fn hex(self) -> String {
        format!("#{:06X}", self.to_u32())
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:06X}", self.to_u32())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParseColorError(String);

impl fmt::Display for ParseColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ParseColorError {}

impl FromStr for Rgb {
    type Err = ParseColorError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let hex = input.trim().strip_prefix('#').unwrap_or(input.trim());
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ParseColorError(format!(
                "invalid color {input:?}; expected exactly six hexadecimal digits, optionally prefixed by #"
            )));
        }

        let value = u32::from_str_radix(hex, 16)
            .map_err(|_| ParseColorError(format!("invalid hexadecimal color {input:?}")))?;
        Ok(Self::from_u32(value))
    }
}

/// Reproduces Minecraft Java's `ARGB.setBrightness(color, 0.9F)` path.
///
/// Intermediate values deliberately remain `f32`, matching Java `float`.
pub fn set_brightness(color: Rgb, brightness: f32) -> Rgb {
    let red = i32::from(color.red);
    let green = i32::from(color.green);
    let blue = i32::from(color.blue);

    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let chroma = (maximum - minimum) as f32;
    let saturation = if maximum == 0 {
        0.0_f32
    } else {
        chroma / maximum as f32
    };

    let mut hue = if saturation == 0.0_f32 {
        0.0_f32
    } else {
        let red_distance = (maximum - red) as f32 / chroma;
        let green_distance = (maximum - green) as f32 / chroma;
        let blue_distance = (maximum - blue) as f32 / chroma;

        if red == maximum {
            blue_distance - green_distance
        } else if green == maximum {
            2.0_f32 + red_distance - blue_distance
        } else {
            4.0_f32 + green_distance - red_distance
        }
    };

    hue /= 6.0_f32;
    if hue < 0.0_f32 {
        hue += 1.0_f32;
    }

    if saturation == 0.0_f32 {
        let channel = java_round_channel(brightness * 255.0_f32);
        return Rgb::new(channel, channel, channel);
    }

    let segment = (hue - hue.floor()) * 6.0_f32;
    let offset = segment - segment.floor();
    let primary = brightness * (1.0_f32 - saturation);
    let secondary = brightness * (1.0_f32 - saturation * offset);
    let tertiary = brightness * (1.0_f32 - saturation * (1.0_f32 - offset));

    let value = java_round_channel(brightness * 255.0_f32);
    let primary = java_round_channel(primary * 255.0_f32);
    let secondary = java_round_channel(secondary * 255.0_f32);
    let tertiary = java_round_channel(tertiary * 255.0_f32);

    match segment as i32 {
        0 => Rgb::new(value, tertiary, primary),
        1 => Rgb::new(secondary, value, primary),
        2 => Rgb::new(primary, value, tertiary),
        3 => Rgb::new(primary, secondary, value),
        4 => Rgb::new(tertiary, primary, value),
        5 => Rgb::new(value, primary, secondary),
        _ => unreachable!("normalized hue must select one of six color wheel segments"),
    }
}

fn java_round_channel(value: f32) -> u8 {
    // Java Math.round(float) is floor(value + 0.5). Channels here are non-negative.
    ((value + 0.5_f32).floor() as i32).clamp(0, 255) as u8
}

/// Finds every internal 24-bit RGB value that Minecraft renders as `target`.
pub fn rendered_preimages(target: Rgb) -> HashSet<u32> {
    (0_u32..=0x00ff_ffff)
        .filter(|raw| set_brightness(Rgb::from_u32(*raw), 0.9_f32) == target)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_hex_forms() {
        assert_eq!("#39E6E5".parse::<Rgb>().unwrap(), Rgb::new(57, 230, 229));
        assert_eq!("39e6e5".parse::<Rgb>().unwrap(), Rgb::new(57, 230, 229));
        assert!("#39E6E".parse::<Rgb>().is_err());
        assert!("#39E6EZ".parse::<Rgb>().is_err());
    }

    #[test]
    fn matches_known_minecraft_brightness_vectors() {
        assert_eq!(
            set_brightness(Rgb::from_u32(0x000000), 0.9),
            Rgb::from_u32(0xE6E6E6)
        );
        assert_eq!(
            set_brightness(Rgb::from_u32(0xFFFFFF), 0.9),
            Rgb::from_u32(0xE6E6E6)
        );
        assert_eq!(
            set_brightness(Rgb::from_u32(0xF5688E), 0.9),
            Rgb::from_u32(0xE66185)
        );
        assert_eq!(
            set_brightness(Rgb::from_u32(0x3AEBEA), 0.9),
            Rgb::from_u32(0x39E6E5)
        );
    }

    #[test]
    fn target_has_exact_expected_preimages() {
        let target = Rgb::from_u32(0x39E6E5);
        let values = rendered_preimages(target);
        assert_eq!(values.len(), 26);
        assert!(values.contains(&0x39E6E5));
        assert!(values.contains(&0x39E7E6));
        assert!(values.contains(&0x3FFFFE));
    }
}
