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
    let clock = lines.iter().any(|line| {
        let value = line.text.to_lowercase();
        line.left < 500.0 && value.contains(':') && (value.contains("pm") || value.contains("am"))
    });
    if clock {
        return "world".into();
    }
    "unknown".into()
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
}
