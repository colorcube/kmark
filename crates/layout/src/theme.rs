use kmark_syntax::AlertKind;

/// RGBA color (`0..=255` per channel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(C)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[must_use]
    #[allow(clippy::self_named_constructors)]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
}

/// Complete editor color palette synchronized with KDE Plasma's `QPalette` / `KColorScheme`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThemePalette {
    pub dark: bool,
    pub canvas_bg: Rgba,
    pub gutter_bg: Rgba,
    pub gutter_fg: Rgba,
    pub gutter_border: Rgba,
    pub text_fg: Rgba,
    pub muted_fg: Rgba,
    pub link_fg: Rgba,
    pub accent: Rgba,
    pub selection_bg: Rgba,
    pub selection_fg: Rgba,
    pub search_match_bg: Rgba,
    pub search_current_bg: Rgba,
    pub code_bg: Rgba,
    pub inline_code_bg: Rgba,
    pub highlight_bg: Rgba,
    pub quote_bar: Rgba,
    pub quote_fg: Rgba,
    pub rule_line: Rgba,
    pub table_header_bg: Rgba,
    pub table_border: Rgba,
    pub caret: Rgba,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::breeze_light()
    }
}

impl ThemePalette {
    /// KDE Breeze Light default colors (`#fcfcfc` view, `#232629` ink, `#3daee9` Plasma blue).
    #[must_use]
    pub const fn breeze_light() -> Self {
        Self {
            dark: false,
            canvas_bg: Rgba::rgb(252, 252, 252),
            gutter_bg: Rgba::rgb(245, 246, 247),
            gutter_fg: Rgba::rgb(127, 140, 141),
            gutter_border: Rgba::rgb(225, 228, 232),
            text_fg: Rgba::rgb(35, 38, 41),
            muted_fg: Rgba::rgb(127, 140, 141),
            link_fg: Rgba::rgb(41, 128, 185),
            accent: Rgba::rgb(61, 174, 233),
            selection_bg: Rgba::rgba(61, 174, 233, 76),
            selection_fg: Rgba::rgb(35, 38, 41),
            search_match_bg: Rgba::rgba(246, 196, 69, 110),
            search_current_bg: Rgba::rgba(243, 156, 18, 180),
            code_bg: Rgba::rgb(243, 244, 246),
            inline_code_bg: Rgba::rgb(236, 239, 243),
            highlight_bg: Rgba::rgba(255, 235, 110, 165),
            quote_bar: Rgba::rgb(189, 195, 199),
            quote_fg: Rgba::rgb(90, 98, 106),
            rule_line: Rgba::rgb(213, 218, 222),
            table_header_bg: Rgba::rgb(239, 241, 244),
            table_border: Rgba::rgb(198, 205, 212),
            caret: Rgba::rgb(35, 38, 41),
        }
    }

    /// KDE Breeze Dark default colors (`#1b1e20` view, `#fcfcfc` ink, `#3daee9` Plasma blue).
    #[must_use]
    pub const fn breeze_dark() -> Self {
        Self {
            dark: true,
            canvas_bg: Rgba::rgb(27, 30, 32),
            gutter_bg: Rgba::rgb(35, 38, 41),
            gutter_fg: Rgba::rgb(142, 150, 158),
            gutter_border: Rgba::rgb(52, 57, 62),
            text_fg: Rgba::rgb(252, 252, 252),
            muted_fg: Rgba::rgb(142, 150, 158),
            link_fg: Rgba::rgb(91, 185, 240),
            accent: Rgba::rgb(61, 174, 233),
            selection_bg: Rgba::rgba(61, 174, 233, 92),
            selection_fg: Rgba::rgb(252, 252, 252),
            search_match_bg: Rgba::rgba(246, 196, 69, 95),
            search_current_bg: Rgba::rgba(243, 156, 18, 165),
            code_bg: Rgba::rgb(36, 40, 44),
            inline_code_bg: Rgba::rgb(46, 51, 56),
            highlight_bg: Rgba::rgba(196, 160, 0, 110),
            quote_bar: Rgba::rgb(96, 105, 114),
            quote_fg: Rgba::rgb(184, 192, 200),
            rule_line: Rgba::rgb(62, 68, 74),
            table_header_bg: Rgba::rgb(38, 43, 48),
            table_border: Rgba::rgb(74, 82, 92),
            caret: Rgba::rgb(252, 252, 252),
        }
    }

    #[must_use]
    pub const fn alert_color(&self, kind: AlertKind) -> Rgba {
        match kind {
            AlertKind::Note => Rgba::rgb(41, 128, 185),
            AlertKind::Tip => Rgba::rgb(39, 174, 96),
            AlertKind::Important => Rgba::rgb(142, 68, 173),
            AlertKind::Warning => Rgba::rgb(218, 132, 22),
            AlertKind::Caution => Rgba::rgb(218, 68, 83),
        }
    }
}
