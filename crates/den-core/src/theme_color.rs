use crate::ThemeColors;
// OKLab matrices from https://bottosson.github.io/posts/oklab/.
// Holding a/b while replacing L holds OKLCH chroma and hue.
fn rgb(hex: &str) -> [f64; 3] {
    std::array::from_fn(|i| {
        let v = u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16).unwrap() as f64 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}
fn lab(hex: &str) -> [f64; 3] {
    let [r, g, b] = rgb(hex);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    ]
}
fn hex([l, a, b]: [f64; 3]) -> String {
    let ll = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (l - 0.0894841775 * a - 1.291485548 * b).powi(3);
    let channels = [
        4.0767416621 * ll - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * ll + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * ll - 0.7034186147 * m + 1.707614701 * s,
    ];
    let bytes = channels.map(|v| {
        let v = v.clamp(0., 1.);
        ((if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1. / 2.4) - 0.055
        }) * 255.)
            .round() as u8
    });
    format!("#{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2])
}
pub fn contrast(a: &str, b: &str) -> f64 {
    let luminance = |h: &str| {
        let [r, g, b] = rgb(h);
        r * 0.2126 + g * 0.7152 + b * 0.0722
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn derive_half(source: &ThemeColors) -> ThemeColors {
    let replace_l = |color: &str, l: f64| {
        let mut c = lab(color);
        c[0] = l.clamp(0., 1.);
        hex(c)
    };
    let mut out = source.clone();
    out.bg = replace_l(&source.bg, lab(&source.ink)[0]);
    out.ink = replace_l(&source.ink, lab(&source.bg)[0]);
    out.bg2 = replace_l(&source.bg2, lab(&source.ink2)[0]);
    out.ink2 = replace_l(&source.ink2, lab(&source.bg2)[0]);
    out.bg3 = replace_l(&source.bg3, lab(&source.ink3)[0]);
    out.ink3 = replace_l(&source.ink3, lab(&source.bg3)[0]);
    let direction = if lab(&out.bg)[0] > 0.5 { -1. } else { 1. };
    out.line = replace_l(&source.line, lab(&out.bg)[0] + direction * 0.12);
    let readable = |color: &str| {
        let start = lab(color)[0];
        for step in 0..=1000 {
            let candidate = replace_l(color, start + direction * step as f64 / 1000.);
            if contrast(&candidate, &out.bg) >= 4.5 {
                return candidate;
            }
        }
        if contrast("#000000", &out.bg) >= 4.5 {
            "#000000".into()
        } else {
            "#ffffff".into()
        }
    };
    out.accent = readable(&source.accent);
    out.success = readable(&source.success);
    out.danger = readable(&source.danger);
    out.generated = true;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oklab_roundtrip_and_generated_halves_are_readable() {
        assert!((lab("#ff0000")[0] - 0.6279553606).abs() < 1e-8);
        for color in ["#000000", "#ffffff", "#ff0000", "#12af74", "#e8a44a"] {
            assert_eq!(hex(lab(color)), color);
        }
        for theme in crate::builtin_themes() {
            for source in [&theme.light, &theme.dark] {
                let derived = derive_half(source);
                assert!(derived.generated);
                for color in [&derived.accent, &derived.success, &derived.danger] {
                    assert!(contrast(color, &derived.bg) >= 4.5, "{} {color}", theme.id);
                }
                assert!(contrast(&derived.ink, &derived.bg) >= 4.5);
                assert!(contrast(&derived.ink2, &derived.bg2) >= 4.5);
            }
        }
    }
}
