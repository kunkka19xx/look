use serde::Serialize;

/// One answer block, mirroring the macOS `WebAnswer`/answer-card model so the
/// JSON each shell receives is identical. `url`/`image_url` are present only for
/// sources that have a canonical page or thumbnail (e.g. crypto, Wikipedia).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub text: String,
    pub source: String,
    pub url: Option<String>,
    pub image_url: Option<String>,
}

impl Answer {
    /// Text-only answer (currency, weather) with no source link or image.
    pub fn text(text: impl Into<String>, source: impl Into<String>) -> Self {
        Answer {
            text: text.into(),
            source: source.into(),
            url: None,
            image_url: None,
        }
    }

    /// Answer with an optional source link and thumbnail (crypto, DuckDuckGo,
    /// Wikipedia).
    pub fn linked(
        text: impl Into<String>,
        source: impl Into<String>,
        url: Option<String>,
        image_url: Option<String>,
    ) -> Self {
        Answer {
            text: text.into(),
            source: source.into(),
            url,
            image_url,
        }
    }
}
