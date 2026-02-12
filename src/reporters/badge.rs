use anyhow::Result;

use crate::core::score::{Grade, HealthScore};

pub struct BadgeGenerator;

impl BadgeGenerator {
    pub fn generate(score: &HealthScore) -> Result<String> {
        Ok(render_badge(score))
    }
}

fn grade_color(grade: Grade) -> &'static str {
    match grade {
        Grade::A => "#4c1",
        Grade::B => "#97CA00",
        Grade::C => "#dfb317",
        Grade::D => "#fe7d37",
        Grade::F => "#e05d44",
    }
}

const SVG_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="__TOTAL__" height="20" role="img" aria-label="health: __VALUE__">
  <title>health: __VALUE__</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="__TOTAL__" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="__LW__" height="20" fill="#555"/>
    <rect x="__LW__" width="__VW__" height="20" fill="__COLOR__"/>
    <rect width="__TOTAL__" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">
    <text aria-hidden="true" x="__LX__" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="__LT__">health</text>
    <text x="__LX__" y="140" transform="scale(.1)" fill="#fff" textLength="__LT__">health</text>
    <text aria-hidden="true" x="__VX__" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="__VT__">__VALUE__</text>
    <text x="__VX__" y="140" transform="scale(.1)" fill="#fff" textLength="__VT__">__VALUE__</text>
  </g>
</svg>"##;

fn render_badge(score: &HealthScore) -> String {
    let color = grade_color(score.grade);
    let value = format!("{}/100 {}", score.total, score.grade);

    let label_width: u32 = 50;
    let value_width: u32 = 70;
    let total_width = label_width + value_width;
    let label_x = label_width * 10 / 2;
    let value_x = label_width * 10 + value_width * 10 / 2;
    let label_text_len = (label_width - 10) * 10;
    let value_text_len = (value_width - 10) * 10;

    SVG_TEMPLATE
        .replace("__TOTAL__", &total_width.to_string())
        .replace("__LW__", &label_width.to_string())
        .replace("__VW__", &value_width.to_string())
        .replace("__COLOR__", color)
        .replace("__LX__", &label_x.to_string())
        .replace("__VX__", &value_x.to_string())
        .replace("__LT__", &label_text_len.to_string())
        .replace("__VT__", &value_text_len.to_string())
        .replace("__VALUE__", &value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_score(total: u8, grade: Grade) -> HealthScore {
        HealthScore {
            total,
            grade,
            breakdown: vec![],
        }
    }

    #[test]
    fn test_badge_contains_svg() {
        let score = make_score(95, Grade::A);
        let svg = BadgeGenerator::generate(&score).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_badge_contains_score() {
        let score = make_score(85, Grade::B);
        let svg = BadgeGenerator::generate(&score).unwrap();
        assert!(svg.contains("85/100 B"));
    }

    #[test]
    fn test_badge_grade_a_green() {
        let score = make_score(95, Grade::A);
        let svg = BadgeGenerator::generate(&score).unwrap();
        assert!(svg.contains("#4c1"));
    }

    #[test]
    fn test_badge_grade_f_red() {
        let score = make_score(30, Grade::F);
        let svg = BadgeGenerator::generate(&score).unwrap();
        assert!(svg.contains("#e05d44"));
    }
}
