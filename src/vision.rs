use anyhow::{Result, ensure};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, TextItem};
use rten::Model;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glyph {
    pub text: String,
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ui {
    pub mode: String,
    #[serde(default)]
    pub page: String,
    pub lines: Vec<Line>,
    pub words: Vec<Line>,
    pub width: u32,
    pub height: u32,
    pub processing_ms: u64,
}
pub struct Vision {
    engine: OcrEngine,
}
impl Vision {
    pub fn load(models: &Path) -> Result<Self> {
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(Model::load_file(models.join("text-detection.onnx"))?),
            recognition_model: Some(Model::load_file(models.join("text-recognition.onnx"))?),
            ..Default::default()
        })?;
        Ok(Self { engine })
    }
    pub fn read(&self, path: &Path) -> Result<Ui> {
        let start = Instant::now();
        let image = image::open(path)?.into_rgb8();
        let (width, height) = image.dimensions();
        ensure!(
            width <= 7680 && height <= 4320,
            "Unexpected frame dimensions"
        );
        let resized = image::imageops::resize(
            &image,
            width.min(1920),
            ((height as f32) * (width.min(1920) as f32) / (width as f32)) as u32,
            image::imageops::FilterType::Triangle,
        );
        let scale = width as f64 / resized.width() as f64;
        let source = ImageSource::from_bytes(resized.as_raw(), resized.dimensions())?;
        let input = self.engine.prepare_input(source)?;
        let words = self.engine.detect_words(&input)?;
        let groups = self.engine.find_text_lines(&input, &words);
        let texts = self.engine.recognize_text(&input, &groups)?;
        let mut lines = vec![];
        let mut words = vec![];
        for line in texts.into_iter().flatten() {
            let text = line.to_string();
            if text.len() < 2 {
                continue;
            }
            let rect = line.bounding_rect();
            for word in line.words() {
                let rect = word.bounding_rect();
                words.push(Line {
                    text: word.to_string(),
                    left: rect.left() as f64 * scale,
                    top: rect.top() as f64 * scale,
                    right: rect.right() as f64 * scale,
                    bottom: rect.bottom() as f64 * scale,
                    glyphs: word
                        .chars()
                        .iter()
                        .map(|c| Glyph {
                            text: c.char.to_string(),
                            left: c.rect.left() as f64 * scale,
                            top: c.rect.top() as f64 * scale,
                            right: c.rect.right() as f64 * scale,
                            bottom: c.rect.bottom() as f64 * scale,
                        })
                        .collect(),
                });
            }
            lines.push(Line {
                text,
                left: rect.left() as f64 * scale,
                top: rect.top() as f64 * scale,
                right: rect.right() as f64 * scale,
                bottom: rect.bottom() as f64 * scale,
                glyphs: vec![],
            });
        }
        let mode = classify(&lines);
        let text = lines
            .iter()
            .map(|line| normalize(&line.text))
            .collect::<Vec<_>>()
            .join(" ");
        let page = if mode == "journal"
            && ["serkis", "roberts", "alfonso"]
                .iter()
                .all(|name| text.contains(name))
        {
            "faction_index"
        } else if mode == "journal" && text.contains("en cours") && text.contains("termine") {
            "story_index"
        } else {
            "unknown"
        }
        .to_owned();
        Ok(Ui {
            mode,
            page,
            lines,
            words,
            width,
            height,
            processing_ms: start.elapsed().as_millis() as u64,
        })
    }
}
fn classify(lines: &[Line]) -> String {
    let text = lines
        .iter()
        .map(|v| normalize(&v.text))
        .collect::<Vec<_>>()
        .join("\n");
    if text.contains("charger") && text.contains("sauvegarder") {
        return "pause".into();
    }
    if text.contains("audio") && text.contains("graphismes") && text.contains("accessibilite") {
        return "settings".into();
    }
    if text.contains("charger")
        && (text.contains("continuer") || text.contains("reprendre") || text.contains("jouer"))
        && text.contains("nouvelle")
    {
        return "title".into();
    }
    if (text.contains("journal") || text.contains("lournal"))
        && text.contains("histoire")
        && text.contains("factions")
    {
        return "journal".into();
    }
    // Overlays such as the recent-menu wheel keep the gameplay clock visible.
    // Require their absence before treating the clock as a gameplay signal.
    if text.contains("menu-rece") || text.contains("menu rece") {
        return "unknown".into();
    }
    let headers = lines
        .iter()
        .filter(|line| line.top < 300.0)
        .map(|line| normalize(&line.text))
        .collect::<Vec<_>>()
        .join(" ");
    if ["inventaire", "journal", "tournal", "competenc", "divers"]
        .iter()
        .filter(|word| headers.contains(**word))
        .count()
        >= 3
    {
        return "unknown".into();
    }
    let clock = lines
        .iter()
        .any(|line| line.left < 500.0 && hud_clock(&line.text));
    if clock {
        return "world".into();
    }
    "unknown".into()
}
fn hud_clock(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let digit = |value: u8| match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'i' | b'l' => Some(1),
        b'o' => Some(0),
        _ => None,
    };
    for suffix in ["am", "pm"] {
        for (index, _) in text.match_indices(suffix) {
            let bytes = text[..index].trim_end().as_bytes();
            if bytes.len() < 4 {
                continue;
            }
            let minute_start = bytes.len() - 2;
            let separator = minute_start - 1;
            if !b":.".contains(&bytes[separator])
                || !bytes[minute_start..].iter().all(|b| digit(*b).is_some())
            {
                continue;
            }
            let mut hour_start = separator;
            while hour_start > 0 && digit(bytes[hour_start - 1]).is_some() {
                hour_start -= 1;
            }
            if !(1..=2).contains(&(separator - hour_start)) {
                continue;
            }
            let number = |part: &[u8]| part.iter().fold(0u8, |n, b| n * 10 + digit(*b).unwrap());
            let hour = number(&bytes[hour_start..separator]);
            let minute = number(&bytes[minute_start..]);
            if (1..=12).contains(&hour) && minute < 60 {
                return true;
            }
        }
    }
    false
}
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | '?' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_ui_is_unknown_instead_of_assuming_gameplay() {
        assert_eq!(classify(&[]), "unknown");
        let lines = vec![Line {
            text: "Charger Sauvegarder".into(),
            left: 0.0,
            top: 0.0,
            right: 1.0,
            bottom: 1.0,
            glyphs: vec![],
        }];
        assert_eq!(classify(&lines), "pause");
    }
    #[test]
    fn clock_accepts_observed_ocr_dot_but_rejects_invalid_times() {
        assert!(hud_clock("Jour 85.Lun 3.48 PM"));
        assert!(hud_clock("12:03 am"));
        assert!(hud_clock("Jour 85 Lun 6:l1 PM"));
        for value in ["13.48 PM", "3.60 PM", "0:15 AM", "123.45 PM", "PM"] {
            assert!(!hud_clock(value));
        }
    }
}
#[test]
fn recent_menu_overlay_with_visible_hud_clock_is_not_gameplay() {
    let line = |text: &str, left: f64, top: f64| Line {
        text: text.into(),
        left,
        top,
        right: left + 150.0,
        bottom: top + 30.0,
        glyphs: vec![],
    };
    // Minimized actual OCR trace: the recent-menu overlay leaves the clock visible.
    let lines = vec![
        line("Menu-rece", 700.0, 300.0),
        line("Jour 87 Mer 5:22 AM", 100.0, 950.0),
    ];
    assert_eq!(classify(&lines), "unknown");
}
