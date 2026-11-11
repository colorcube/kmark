//! `kmark-ffi` (L4): Panic-safe C ABI connecting `kmark-engine` to the native C++ Qt6/KF6 shell.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use kmark_engine::{
    Command, KeyCode, KeyModifiers, PointerAction, RenderStateStamp, SearchOptions, Session,
    UiLanguage,
};
use kmark_layout::{
    DrawCmd, FallbackMeasurer, FontMetrics, FontRole, HeadingMarkStyle, MeasuredRun, RectF, Rgba,
    TextMeasurer, ThemePalette, ViewMode,
};
use kmark_syntax::{AlertKind, ColumnAlign, ImageSize};
use unicode_segmentation::UnicodeSegmentation;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<Rgba> for KMarkRgba {
    fn from(c: Rgba) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

impl From<KMarkRgba> for Rgba {
    fn from(c: KMarkRgba) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkRectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<RectF> for KMarkRectF {
    fn from(r: RectF) -> Self {
        Self {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    pub space_width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkPalette {
    pub dark: bool,
    pub canvas_bg: KMarkRgba,
    pub gutter_bg: KMarkRgba,
    pub gutter_fg: KMarkRgba,
    pub gutter_border: KMarkRgba,
    pub text_fg: KMarkRgba,
    pub muted_fg: KMarkRgba,
    pub link_fg: KMarkRgba,
    pub accent: KMarkRgba,
    pub selection_bg: KMarkRgba,
    pub selection_fg: KMarkRgba,
    pub search_match_bg: KMarkRgba,
    pub search_current_bg: KMarkRgba,
    pub code_bg: KMarkRgba,
    pub inline_code_bg: KMarkRgba,
    pub highlight_bg: KMarkRgba,
    pub quote_bar: KMarkRgba,
    pub quote_fg: KMarkRgba,
    pub rule_line: KMarkRgba,
    pub table_header_bg: KMarkRgba,
    pub table_border: KMarkRgba,
    pub caret: KMarkRgba,
}

impl From<ThemePalette> for KMarkPalette {
    fn from(p: ThemePalette) -> Self {
        Self {
            dark: p.dark,
            canvas_bg: p.canvas_bg.into(),
            gutter_bg: p.gutter_bg.into(),
            gutter_fg: p.gutter_fg.into(),
            gutter_border: p.gutter_border.into(),
            text_fg: p.text_fg.into(),
            muted_fg: p.muted_fg.into(),
            link_fg: p.link_fg.into(),
            accent: p.accent.into(),
            selection_bg: p.selection_bg.into(),
            selection_fg: p.selection_fg.into(),
            search_match_bg: p.search_match_bg.into(),
            search_current_bg: p.search_current_bg.into(),
            code_bg: p.code_bg.into(),
            inline_code_bg: p.inline_code_bg.into(),
            highlight_bg: p.highlight_bg.into(),
            quote_bar: p.quote_bar.into(),
            quote_fg: p.quote_fg.into(),
            rule_line: p.rule_line.into(),
            table_header_bg: p.table_header_bg.into(),
            table_border: p.table_border.into(),
            caret: p.caret.into(),
        }
    }
}

impl From<KMarkPalette> for ThemePalette {
    fn from(p: KMarkPalette) -> Self {
        Self {
            dark: p.dark,
            canvas_bg: p.canvas_bg.into(),
            gutter_bg: p.gutter_bg.into(),
            gutter_fg: p.gutter_fg.into(),
            gutter_border: p.gutter_border.into(),
            text_fg: p.text_fg.into(),
            muted_fg: p.muted_fg.into(),
            link_fg: p.link_fg.into(),
            accent: p.accent.into(),
            selection_bg: p.selection_bg.into(),
            selection_fg: p.selection_fg.into(),
            search_match_bg: p.search_match_bg.into(),
            search_current_bg: p.search_current_bg.into(),
            code_bg: p.code_bg.into(),
            inline_code_bg: p.inline_code_bg.into(),
            highlight_bg: p.highlight_bg.into(),
            quote_bar: p.quote_bar.into(),
            quote_fg: p.quote_fg.into(),
            rule_line: p.rule_line.into(),
            table_header_bg: p.table_header_bg.into(),
            table_border: p.table_border.into(),
            caret: p.caret.into(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkDrawCmd {
    pub tag: u8,
    pub font_role: u8,
    pub level: u8,
    pub flags: u8,
    pub rect: KMarkRectF,
    pub aux: f32,
    pub color: KMarkRgba,
    pub text_offset: u32,
    pub text_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkFrameView {
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub scroll_y: f32,
    pub total_height: f32,
    pub caret_rect: KMarkRectF,
    pub cmds_ptr: *const KMarkDrawCmd,
    pub cmds_len: usize,
    pub text_pool_ptr: *const u8,
    pub text_pool_len: usize,
}

impl Default for KMarkFrameView {
    fn default() -> Self {
        Self {
            viewport_w: 0.0,
            viewport_h: 0.0,
            scroll_y: 0.0,
            total_height: 0.0,
            caret_rect: KMarkRectF::default(),
            cmds_ptr: ptr::null(),
            cmds_len: 0,
            text_pool_ptr: ptr::null(),
            text_pool_len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkToolbarState {
    pub heading_level: u8,
    pub view_mode: u8,
    pub table_col_align: u8,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub highlight: bool,
    pub inline_code: bool,
    pub in_table: bool,
    pub in_code_block: bool,
    pub in_blockquote: bool,
    pub in_bullet_list: bool,
    pub in_ordered_list: bool,
    pub in_task_list: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_dirty: bool,
    pub line_1based: usize,
    pub col_1based: usize,
    pub char_count: usize,
    pub word_count: usize,
    pub search_match_count: usize,
    pub search_active_1based: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkLayoutSettings {
    pub view_mode: u8,
    pub heading_marks: u8,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
    pub base_font_px: f32,
    pub line_spacing: f32,
    pub max_prose_width: f32,
}

impl Default for KMarkLayoutSettings {
    fn default() -> Self {
        Self {
            view_mode: 0,
            heading_marks: 0,
            word_wrap: true,
            show_line_numbers: false,
            focus_mode: false,
            typewriter_mode: false,
            base_font_px: 16.0,
            line_spacing: 1.42,
            max_prose_width: 880.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KMarkOutlineItem {
    pub level: u8,
    pub byte_offset: usize,
    pub title_offset: u32,
    pub title_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkOutlineView {
    pub items_ptr: *const KMarkOutlineItem,
    pub items_len: usize,
    pub text_pool_ptr: *const u8,
    pub text_pool_len: usize,
}

impl Default for KMarkOutlineView {
    fn default() -> Self {
        Self {
            items_ptr: ptr::null(),
            items_len: 0,
            text_pool_ptr: ptr::null(),
            text_pool_len: 0,
        }
    }
}

pub type KMarkMetricsFn = extern "C" fn(
    user_data: *mut c_void,
    font_role: u8,
    size_px: f32,
    bold: bool,
    italic: bool,
    out_metrics: *mut KMarkFontMetrics,
);

pub type KMarkWidthFn = extern "C" fn(
    user_data: *mut c_void,
    utf8_ptr: *const c_char,
    utf8_len: usize,
    font_role: u8,
    size_px: f32,
    bold: bool,
    italic: bool,
) -> f32;

pub type KMarkRunLayoutFn = extern "C" fn(
    user_data: *mut c_void,
    utf8_ptr: *const c_char,
    utf8_len: usize,
    utf16_ends_ptr: *const u32,
    grapheme_count: usize,
    font_role: u8,
    size_px: f32,
    bold: bool,
    italic: bool,
    out_advances_ptr: *mut f32,
);

pub type KMarkImageSizeFn = extern "C" fn(
    user_data: *mut c_void,
    src_utf8: *const c_char,
    src_len: usize,
    out_w: *mut f32,
    out_h: *mut f32,
) -> bool;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkPointerResult {
    pub tag: u8,
    pub byte_offset: usize,
    pub str_alloc: *mut c_char,
}

impl Default for KMarkPointerResult {
    fn default() -> Self {
        Self {
            tag: 0,
            byte_offset: 0,
            str_alloc: ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KMarkImageInfo {
    pub start_byte: usize,
    pub end_byte: usize,
    pub size_kind: u8,
    pub size_value: u16,
    pub alt_alloc: *mut c_char,
    pub src_alloc: *mut c_char,
    pub caption_alloc: *mut c_char,
}

impl Default for KMarkImageInfo {
    fn default() -> Self {
        Self {
            start_byte: 0,
            end_byte: 0,
            size_kind: 0,
            size_value: 0,
            alt_alloc: ptr::null_mut(),
            src_alloc: ptr::null_mut(),
            caption_alloc: ptr::null_mut(),
        }
    }
}

fn decode_image_size(size_kind: u8, size_value: u16) -> Option<ImageSize> {
    match size_kind {
        1 => Some(ImageSize::Percent(size_value.clamp(5, 100))),
        2 => Some(ImageSize::Pixels(size_value.clamp(16, 4096))),
        _ => None,
    }
}

fn encode_image_size(size: Option<ImageSize>) -> (u8, u16) {
    match size {
        None => (0, 0),
        Some(ImageSize::Percent(p)) => (1, p),
        Some(ImageSize::Pixels(px)) => (2, px),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct StyleKey {
    role: FontRole,
    size_tenths: u32,
    bold: bool,
    italic: bool,
}

impl StyleKey {
    fn new(role: FontRole, size_px: f32, bold: bool, italic: bool) -> Self {
        let tenths = (size_px.clamp(4.0, 200.0) * 10.0).round() as u32;
        Self {
            role,
            size_tenths: tenths,
            bold,
            italic,
        }
    }
}

const MAX_RUN_CACHE_ENTRIES: usize = 16_384;
const MAX_CACHED_RUN_BYTES: usize = 4_096;
const MAX_IMAGE_CACHE_ENTRIES: usize = 512;

struct QtBridgeMeasurer {
    user_data: *mut c_void,
    metrics_fn: Option<KMarkMetricsFn>,
    width_fn: Option<KMarkWidthFn>,
    run_user_data: *mut c_void,
    run_layout_fn: Option<KMarkRunLayoutFn>,
    image_user_data: *mut c_void,
    image_fn: Option<KMarkImageSizeFn>,
    fallback: FallbackMeasurer,
    metrics_cache: HashMap<StyleKey, FontMetrics>,
    run_cache: HashMap<(StyleKey, String), MeasuredRun>,
    image_cache: HashMap<String, Option<(f32, f32)>>,
    scratch_byte_ends: Vec<usize>,
    scratch_utf16_ends: Vec<u32>,
    scratch_advances: Vec<f32>,
    epoch: u64,
}

impl Default for QtBridgeMeasurer {
    fn default() -> Self {
        Self {
            user_data: ptr::null_mut(),
            metrics_fn: None,
            width_fn: None,
            run_user_data: ptr::null_mut(),
            run_layout_fn: None,
            image_user_data: ptr::null_mut(),
            image_fn: None,
            fallback: FallbackMeasurer,
            metrics_cache: HashMap::new(),
            run_cache: HashMap::new(),
            image_cache: HashMap::new(),
            scratch_byte_ends: Vec::new(),
            scratch_utf16_ends: Vec::new(),
            scratch_advances: Vec::new(),
            epoch: 1,
        }
    }
}

impl QtBridgeMeasurer {
    fn clear_cache(&mut self) {
        self.metrics_cache.clear();
        self.run_cache.clear();
        self.image_cache.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn role_code(role: FontRole) -> u8 {
        match role {
            FontRole::SansBody => 0,
            FontRole::Monospace => 1,
        }
    }
}

impl TextMeasurer for QtBridgeMeasurer {
    fn font_metrics(
        &mut self,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
    ) -> FontMetrics {
        let Some(cb) = self.metrics_fn else {
            return self.fallback.font_metrics(role, size_px, bold, italic);
        };
        let key = StyleKey::new(role, size_px, bold, italic);
        if let Some(&cached) = self.metrics_cache.get(&key) {
            return cached;
        }
        let mut raw = KMarkFontMetrics::default();
        cb(
            self.user_data,
            Self::role_code(role),
            size_px,
            bold,
            italic,
            &raw mut raw,
        );
        let resolved = FontMetrics {
            ascent: raw.ascent.max(4.0),
            descent: raw.descent.max(1.0),
            line_height: raw.line_height.max(6.0),
            space_width: raw.space_width.max(1.0),
        };
        self.metrics_cache.insert(key, resolved);
        resolved
    }

    fn measure_run(
        &mut self,
        text: &str,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
    ) -> MeasuredRun {
        if self.run_layout_fn.is_none() && self.width_fn.is_none() {
            return self.fallback.measure_run(text, role, size_px, bold, italic);
        }
        let s_key = StyleKey::new(role, size_px, bold, italic);
        if text.is_empty() {
            let metrics = self.font_metrics(role, size_px, bold, italic);
            return MeasuredRun {
                total_width: 0.0,
                metrics,
                grapheme_ends: Vec::new(),
            };
        }

        let cacheable = text.len() <= MAX_CACHED_RUN_BYTES;
        if cacheable {
            let lookup_key = (s_key, text.to_owned());
            if let Some(hit) = self.run_cache.get(&lookup_key) {
                return hit.clone();
            }
        }

        let metrics = self.font_metrics(role, size_px, bold, italic);
        let role_u8 = Self::role_code(role);

        let run = if let Some(run_cb) = self.run_layout_fn {
            self.scratch_byte_ends.clear();
            self.scratch_utf16_ends.clear();
            let mut cum_utf16 = 0u32;
            for (rel_idx, g) in text.grapheme_indices(true) {
                let end_b = rel_idx.saturating_add(g.len());
                let g_utf16: u32 = g.chars().map(|ch| ch.len_utf16() as u32).sum();
                cum_utf16 = cum_utf16.saturating_add(g_utf16);
                self.scratch_byte_ends.push(end_b);
                self.scratch_utf16_ends.push(cum_utf16);
            }

            let count = self.scratch_byte_ends.len();
            self.scratch_advances.clear();
            self.scratch_advances.resize(count, 0.0);

            if count > 0 {
                run_cb(
                    self.run_user_data,
                    text.as_ptr().cast::<c_char>(),
                    text.len(),
                    self.scratch_utf16_ends.as_ptr(),
                    count,
                    role_u8,
                    size_px,
                    bold,
                    italic,
                    self.scratch_advances.as_mut_ptr(),
                );
            }

            let mut ends = Vec::with_capacity(count);
            let mut last_w = 0.0f32;
            for (&end_b, &raw_w) in self
                .scratch_byte_ends
                .iter()
                .zip(self.scratch_advances.iter())
            {
                let w = if raw_w.is_finite() {
                    raw_w.max(last_w)
                } else {
                    last_w
                };
                last_w = w;
                ends.push((end_b, w));
            }

            MeasuredRun {
                total_width: last_w,
                metrics,
                grapheme_ends: ends,
            }
        } else if let Some(w_cb) = self.width_fn {
            let mut ends = Vec::new();
            let mut last_w = 0.0f32;
            for (rel_idx, g) in text.grapheme_indices(true) {
                let end_b = rel_idx.saturating_add(g.len());
                if let Some(prefix) = text.get(..end_b) {
                    let w = w_cb(
                        self.user_data,
                        prefix.as_ptr().cast::<c_char>(),
                        prefix.len(),
                        role_u8,
                        size_px,
                        bold,
                        italic,
                    )
                    .max(last_w);
                    last_w = w;
                    ends.push((end_b, w));
                }
            }
            MeasuredRun {
                total_width: last_w,
                metrics,
                grapheme_ends: ends,
            }
        } else {
            self.fallback.measure_run(text, role, size_px, bold, italic)
        };

        if cacheable {
            if self.run_cache.len() >= MAX_RUN_CACHE_ENTRIES {
                self.run_cache.clear();
            }
            self.run_cache.insert((s_key, text.to_owned()), run.clone());
        }
        run
    }

    fn image_dimensions(&mut self, src: &str) -> Option<(f32, f32)> {
        if src.is_empty() {
            return None;
        }
        let img_cb = self.image_fn?;
        if let Some(&cached) = self.image_cache.get(src) {
            return cached;
        }
        if self.image_cache.len() >= MAX_IMAGE_CACHE_ENTRIES {
            self.image_cache.clear();
        }
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let ok = img_cb(
            self.image_user_data,
            src.as_ptr().cast::<c_char>(),
            src.len(),
            &raw mut w,
            &raw mut h,
        );
        let res = if ok && w > 1.0 && h > 1.0 {
            Some((w, h))
        } else {
            None
        };
        self.image_cache.insert(src.to_owned(), res);
        res
    }
}

struct CachedRenderedFrame {
    stamp: RenderStateStamp,
    measurer_epoch: u64,
    vw_bits: u32,
    vh_bits: u32,
    viewport_w: f32,
    viewport_h: f32,
    scroll_y: f32,
    total_height: f32,
    caret_rect: KMarkRectF,
}

pub struct KMarkSession {
    session: Session,
    measurer: QtBridgeMeasurer,
    last_error: String,
    frame_cmds: Vec<KMarkDrawCmd>,
    frame_text_pool: Vec<u8>,
    cached_frame: Option<CachedRenderedFrame>,
    outline_items: Vec<KMarkOutlineItem>,
    outline_text_pool: Vec<u8>,
}

unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        return "";
    }
    // SAFETY: Caller passes a NUL-terminated string valid for the duration of the FFI call.
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
}

fn alloc_c_string(s: &str) -> *mut c_char {
    let sanitized = if s.as_bytes().contains(&0) {
        s.replace('\0', "")
    } else {
        s.to_owned()
    };
    CString::new(sanitized).map_or(ptr::null_mut(), CString::into_raw)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_new() -> *mut KMarkSession {
    catch_unwind(|| {
        Box::into_raw(Box::new(KMarkSession {
            session: Session::new(),
            measurer: QtBridgeMeasurer::default(),
            last_error: String::new(),
            frame_cmds: Vec::with_capacity(512),
            frame_text_pool: Vec::with_capacity(8192),
            cached_frame: None,
            outline_items: Vec::new(),
            outline_text_pool: Vec::new(),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_free(session: *mut KMarkSession) {
    if session.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `session` was allocated by `kmark_session_new` and is freed once.
        let _ = unsafe { Box::from_raw(session) };
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_ui_language(session: *mut KMarkSession, lang: u8) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ui_lang = if lang == 0 {
            UiLanguage::English
        } else {
            UiLanguage::German
        };
        s.session.set_ui_language(ui_lang);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_ui_language(session: *const KMarkSession) -> u8 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return 1;
    };
    catch_unwind(AssertUnwindSafe(|| match s.session.ui_language {
        UiLanguage::English => 0,
        UiLanguage::German => 1,
    }))
    .unwrap_or(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_measurer(
    session: *mut KMarkSession,
    user_data: *mut c_void,
    metrics_fn: Option<KMarkMetricsFn>,
    width_fn: Option<KMarkWidthFn>,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.measurer.user_data = user_data;
        s.measurer.metrics_fn = metrics_fn;
        s.measurer.width_fn = width_fn;
        s.measurer.clear_cache();
        s.cached_frame = None;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_run_measurer(
    session: *mut KMarkSession,
    user_data: *mut c_void,
    run_layout_fn: Option<KMarkRunLayoutFn>,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.measurer.run_user_data = user_data;
        s.measurer.run_layout_fn = run_layout_fn;
        s.measurer.clear_cache();
        s.cached_frame = None;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_image_measurer(
    session: *mut KMarkSession,
    user_data: *mut c_void,
    image_fn: Option<KMarkImageSizeFn>,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.measurer.image_user_data = user_data;
        s.measurer.image_fn = image_fn;
        s.measurer.image_cache.clear();
        s.measurer.epoch = s.measurer.epoch.wrapping_add(1);
        s.cached_frame = None;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_clear_measure_cache(session: *mut KMarkSession) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.measurer.clear_cache();
        s.cached_frame = None;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_palette_breeze_light() -> KMarkPalette {
    ThemePalette::breeze_light().into()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_palette_breeze_dark() -> KMarkPalette {
    ThemePalette::breeze_dark().into()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_palette(session: *mut KMarkSession, palette: KMarkPalette) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.set_palette(palette.into());
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_get_layout_settings(
    session: *const KMarkSession,
) -> KMarkLayoutSettings {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return KMarkLayoutSettings::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        let cfg = s.session.layout_config();
        let view_mode = match cfg.view_mode {
            ViewMode::Wysiwyg => 0,
            ViewMode::LivePreview => 1,
            ViewMode::Source => 2,
            ViewMode::PlainText => 3,
        };
        let heading_marks = match cfg.heading_marks {
            HeadingMarkStyle::SlantedBars => 0,
            HeadingMarkStyle::HashSymbol => 1,
            HeadingMarkStyle::None => 2,
        };
        KMarkLayoutSettings {
            view_mode,
            heading_marks,
            word_wrap: cfg.word_wrap,
            show_line_numbers: cfg.show_line_numbers,
            focus_mode: cfg.focus_mode,
            typewriter_mode: cfg.typewriter_mode,
            base_font_px: cfg.base_font_px,
            line_spacing: cfg.line_spacing_scale,
            max_prose_width: cfg.max_prose_width_px,
        }
    }))
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_layout_settings(
    session: *mut KMarkSession,
    settings: KMarkLayoutSettings,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let view_mode = match settings.view_mode {
            1 => ViewMode::LivePreview,
            2 => ViewMode::Source,
            3 => ViewMode::PlainText,
            _ => ViewMode::Wysiwyg,
        };
        let heading_marks = match settings.heading_marks {
            1 => HeadingMarkStyle::HashSymbol,
            2 => HeadingMarkStyle::None,
            _ => HeadingMarkStyle::SlantedBars,
        };
        s.session.set_layout_options(
            view_mode,
            heading_marks,
            settings.word_wrap,
            settings.show_line_numbers,
            settings.focus_mode,
            settings.typewriter_mode,
            settings.base_font_px,
            settings.line_spacing,
            settings.max_prose_width,
        );
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_tab_count(session: *const KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.tab_count())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_active_tab(session: *const KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.active_tab_index())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_active_tab(session: *mut KMarkSession, idx: usize) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.set_active_tab(idx);
        s.measurer.image_cache.clear();
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_new_tab(session: *mut KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.new_empty_tab())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_open_guide_tab(session: *mut KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.open_guide_tab())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_open_reference_tab(session: *mut KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.open_reference_tab())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_close_tab(session: *mut KMarkSession, idx: usize) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.close_tab(idx);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_tab_is_dirty(session: *const KMarkSession, idx: usize) -> bool {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session.tabs().get(idx).is_some_and(|t| t.is_dirty())
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_tab_has_path(session: *const KMarkSession, idx: usize) -> bool {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session.tabs().get(idx).is_some_and(|t| t.path.is_some())
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_tab_title_alloc(
    session: *const KMarkSession,
    idx: usize,
) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session
            .tabs()
            .get(idx)
            .map_or(ptr::null_mut(), |t| alloc_c_string(&t.title))
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_tab_path_alloc(
    session: *const KMarkSession,
    idx: usize,
) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session
            .tabs()
            .get(idx)
            .and_then(|t| t.path.as_ref())
            .map_or(ptr::null_mut(), |p| alloc_c_string(&p.to_string_lossy()))
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_open_file(
    session: *mut KMarkSession,
    path_utf8: *const c_char,
) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let path_str = unsafe { cstr_to_str(path_utf8) };
        match s.session.open_file(path_str) {
            Ok(_) => {
                s.last_error.clear();
                true
            }
            Err(err) => {
                s.last_error = err.to_string();
                false
            }
        }
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_save_active(session: *mut KMarkSession) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| match s.session.save_active_tab() {
        Ok(()) => {
            s.last_error.clear();
            true
        }
        Err(err) => {
            s.last_error = err.to_string();
            false
        }
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_save_active_as(
    session: *mut KMarkSession,
    path_utf8: *const c_char,
) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let path_str = unsafe { cstr_to_str(path_utf8) };
        match s.session.save_active_tab_as(path_str) {
            Ok(()) => {
                s.last_error.clear();
                true
            }
            Err(err) => {
                s.last_error = err.to_string();
                false
            }
        }
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_flush_recovery(session: *mut KMarkSession) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.flush_recovery_draft();
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_recover_drafts(session: *mut KMarkSession) -> usize {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.recover_crash_drafts())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_clear_recovery(session: *mut KMarkSession) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.clear_all_recovery_drafts();
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_last_error_alloc(session: *const KMarkSession) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| alloc_c_string(&s.last_error))).unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_selected_text_alloc(session: *const KMarkSession) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session
            .active_doc()
            .map_or(ptr::null_mut(), |d| alloc_c_string(&d.selected_text()))
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_export_html_alloc(session: *const KMarkSession) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session
            .active_doc()
            .map_or(ptr::null_mut(), |d| alloc_c_string(&d.export_html()))
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_active_code_language_alloc(
    session: *const KMarkSession,
) -> *mut c_char {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return ptr::null_mut();
    };
    catch_unwind(AssertUnwindSafe(|| {
        let st = s.session.toolbar_state();
        alloc_c_string(&st.code_language)
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    let _ = catch_unwind(|| {
        // SAFETY: Allocated by `CString::into_raw` in `alloc_c_string`.
        let _ = unsafe { CString::from_raw(s) };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_exec_command(session: *mut KMarkSession, cmd_id: u32) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let mapped = match cmd_id {
            0 => Some(Command::Undo),
            1 => Some(Command::Redo),
            2 => Some(Command::SelectAll),
            3 => Some(Command::ToggleBold),
            4 => Some(Command::ToggleItalic),
            5 => Some(Command::ToggleStrike),
            6 => Some(Command::ToggleHighlight),
            7 => Some(Command::ToggleInlineCode),
            10..=16 => Some(Command::SetHeading((cmd_id - 10) as u8)),
            20 => Some(Command::ToggleBulletList),
            21 => Some(Command::ToggleOrderedList),
            22 => Some(Command::ToggleTaskList),
            23 => Some(Command::ToggleBlockQuote),
            24 => Some(Command::InsertAlert(AlertKind::Note)),
            25 => Some(Command::InsertAlert(AlertKind::Tip)),
            26 => Some(Command::InsertAlert(AlertKind::Important)),
            27 => Some(Command::InsertAlert(AlertKind::Warning)),
            28 => Some(Command::InsertAlert(AlertKind::Caution)),
            29 => Some(Command::InsertHorizontalRule),
            30 => Some(Command::TableInsertRowAbove),
            31 => Some(Command::TableInsertRowBelow),
            32 => Some(Command::TableDeleteRow),
            33 => Some(Command::TableInsertColLeft),
            34 => Some(Command::TableInsertColRight),
            35 => Some(Command::TableDeleteCol),
            36 => Some(Command::TableSetAlign(ColumnAlign::Left)),
            37 => Some(Command::TableSetAlign(ColumnAlign::Center)),
            38 => Some(Command::TableSetAlign(ColumnAlign::Right)),
            40 => Some(Command::SetViewMode(ViewMode::Wysiwyg)),
            41 => Some(Command::SetViewMode(ViewMode::LivePreview)),
            42 => Some(Command::SetViewMode(ViewMode::Source)),
            43 => Some(Command::SetViewMode(ViewMode::PlainText)),
            44 => Some(Command::ToggleLineNumbers),
            45 => Some(Command::ToggleWordWrap),
            46 => Some(Command::ToggleFocusMode),
            47 => Some(Command::ToggleTypewriterMode),
            48 => Some(Command::ZoomIn),
            49 => Some(Command::ZoomOut),
            50 => Some(Command::ZoomReset),
            51 => Some(Command::SetHeadingMarks(HeadingMarkStyle::SlantedBars)),
            52 => Some(Command::SetHeadingMarks(HeadingMarkStyle::HashSymbol)),
            53 => Some(Command::SetHeadingMarks(HeadingMarkStyle::None)),
            _ => None,
        };
        if let Some(cmd) = mapped {
            s.session.execute_command(cmd);
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_code_fence(
    session: *mut KMarkSession,
    language_utf8: *const c_char,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let lang = unsafe { cstr_to_str(language_utf8) };
        s.session.execute_command(Command::InsertCodeFence {
            language: lang.to_owned(),
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_code_fence_language(
    session: *mut KMarkSession,
    has_offset: bool,
    open_line_start: usize,
    language_utf8: *const c_char,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let lang = unsafe { cstr_to_str(language_utf8) };
        s.session.execute_command(Command::SetCodeFenceLanguage {
            open_line_start: has_offset.then_some(open_line_start),
            language: lang.to_owned(),
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_callout_kind(
    session: *mut KMarkSession,
    has_offset: bool,
    open_line_start: usize,
    alert_kind: u8,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let kind = match alert_kind {
            1 => Some(AlertKind::Note),
            2 => Some(AlertKind::Tip),
            3 => Some(AlertKind::Important),
            4 => Some(AlertKind::Warning),
            5 => Some(AlertKind::Caution),
            _ => None,
        };
        s.session.execute_command(Command::SetCalloutKind {
            open_line_start: has_offset.then_some(open_line_start),
            kind,
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_link(
    session: *mut KMarkSession,
    text_utf8: *const c_char,
    url_utf8: *const c_char,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let txt = unsafe { cstr_to_str(text_utf8) };
        let url = unsafe { cstr_to_str(url_utf8) };
        s.session.execute_command(Command::InsertLink {
            text: txt.to_owned(),
            url: url.to_owned(),
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_image(
    session: *mut KMarkSession,
    alt_utf8: *const c_char,
    url_utf8: *const c_char,
) {
    kmark_session_insert_image_ex(session, alt_utf8, url_utf8, ptr::null(), 0, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_image_ex(
    session: *mut KMarkSession,
    alt_utf8: *const c_char,
    url_utf8: *const c_char,
    caption_utf8: *const c_char,
    size_kind: u8,
    size_value: u16,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let alt = unsafe { cstr_to_str(alt_utf8) };
        let url = unsafe { cstr_to_str(url_utf8) };
        let caption = if caption_utf8.is_null() {
            None
        } else {
            let cap = unsafe { cstr_to_str(caption_utf8) }.trim();
            (!cap.is_empty()).then(|| cap.to_owned())
        };
        s.session.execute_command(Command::InsertImage {
            alt: alt.to_owned(),
            url: url.to_owned(),
            caption,
            size: decode_image_size(size_kind, size_value),
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_update_image(
    session: *mut KMarkSession,
    start_byte: usize,
    end_byte: usize,
    alt_utf8: *const c_char,
    url_utf8: *const c_char,
    caption_utf8: *const c_char,
    size_kind: u8,
    size_value: u16,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let alt = unsafe { cstr_to_str(alt_utf8) };
        let url = unsafe { cstr_to_str(url_utf8) };
        let caption = if caption_utf8.is_null() {
            None
        } else {
            let cap = unsafe { cstr_to_str(caption_utf8) }.trim();
            (!cap.is_empty()).then(|| cap.to_owned())
        };
        s.session.execute_command(Command::UpdateImage {
            range: start_byte..end_byte,
            alt: alt.to_owned(),
            url: url.to_owned(),
            caption,
            size: decode_image_size(size_kind, size_value),
        });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_image_at(
    session: *const KMarkSession,
    vx: f32,
    vy: f32,
    out_info: *mut KMarkImageInfo,
) -> bool {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return false;
    };
    let Some(out) = (unsafe { out_info.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        *out = KMarkImageInfo::default();
        let Some(img) = s.session.image_at(vx, vy) else {
            return false;
        };
        let (size_kind, size_value) = encode_image_size(img.size);
        *out = KMarkImageInfo {
            start_byte: img.range.start,
            end_byte: img.range.end,
            size_kind,
            size_value,
            alt_alloc: alloc_c_string(&img.alt),
            src_alloc: alloc_c_string(&img.src),
            caption_alloc: img
                .caption
                .as_deref()
                .map_or(ptr::null_mut(), alloc_c_string),
        };
        true
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_image_info_free(info: *mut KMarkImageInfo) {
    let Some(info) = (unsafe { info.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !info.alt_alloc.is_null() {
            kmark_string_free(info.alt_alloc);
            info.alt_alloc = ptr::null_mut();
        }
        if !info.src_alloc.is_null() {
            kmark_string_free(info.src_alloc);
            info.src_alloc = ptr::null_mut();
        }
        if !info.caption_alloc.is_null() {
            kmark_string_free(info.caption_alloc);
            info.caption_alloc = ptr::null_mut();
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_table(session: *mut KMarkSession, cols: usize, rows: usize) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session
            .execute_command(Command::InsertTable { cols, rows });
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_insert_text(session: *mut KMarkSession, text_utf8: *const c_char) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let text = unsafe { cstr_to_str(text_utf8) };
        s.session.insert_text(text);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_ime_preedit(
    session: *mut KMarkSession,
    preedit_utf8: *const c_char,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if preedit_utf8.is_null() {
            s.session.set_ime_preedit(None);
        } else {
            let text = unsafe { cstr_to_str(preedit_utf8) };
            s.session.set_ime_preedit(Some(text));
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_key_press(
    session: *mut KMarkSession,
    key_code: u32,
    shift: bool,
    ctrl: bool,
    alt: bool,
) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let mapped = match key_code {
            0 => Some(KeyCode::Left),
            1 => Some(KeyCode::Right),
            2 => Some(KeyCode::Up),
            3 => Some(KeyCode::Down),
            4 => Some(KeyCode::Home),
            5 => Some(KeyCode::End),
            6 => Some(KeyCode::PageUp),
            7 => Some(KeyCode::PageDown),
            8 => Some(KeyCode::Backspace),
            9 => Some(KeyCode::Delete),
            10 => Some(KeyCode::Enter),
            11 => Some(KeyCode::Tab),
            12 => Some(KeyCode::Backtab),
            13 => Some(KeyCode::Escape),
            _ => None,
        };
        let Some(k) = mapped else {
            return false;
        };
        s.session.handle_key(k, KeyModifiers { shift, ctrl, alt })
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_pointer_press(
    session: *mut KMarkSession,
    vx: f32,
    vy: f32,
    click_count: u8,
    shift: bool,
    ctrl: bool,
    alt: bool,
) -> KMarkPointerResult {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return KMarkPointerResult::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        match s
            .session
            .handle_pointer_press(vx, vy, click_count, KeyModifiers { shift, ctrl, alt })
        {
            PointerAction::Handled => KMarkPointerResult::default(),
            PointerAction::OpenExternalUrl(url) => KMarkPointerResult {
                tag: 1,
                byte_offset: 0,
                str_alloc: alloc_c_string(&url),
            },
            PointerAction::EditCodeLanguage {
                open_line_start,
                current_language,
            } => KMarkPointerResult {
                tag: 2,
                byte_offset: open_line_start,
                str_alloc: alloc_c_string(&current_language),
            },
            PointerAction::EditCalloutKind {
                open_line_start,
                current_kind,
            } => KMarkPointerResult {
                tag: 3,
                byte_offset: open_line_start,
                str_alloc: alloc_c_string(current_kind.tag_str()),
            },
        }
    }))
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_pointer_drag(session: *mut KMarkSession, vx: f32, vy: f32) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.handle_pointer_drag(vx, vy);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_drop_cursor_at(session: *mut KMarkSession, vx: f32, vy: f32) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.drop_cursor_at(vx, vy);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_hover_interactive(
    session: *const KMarkSession,
    vx: f32,
    vy: f32,
    ctrl: bool,
) -> bool {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.session.hover_interactive_at(vx, vy, ctrl)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_scroll_y(session: *mut KMarkSession, scroll_y: f32) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.set_scroll_y(scroll_y);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_scroll_pixels(session: *mut KMarkSession, delta_y: f32) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.scroll_by_pixels(delta_y);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_set_search(
    session: *mut KMarkSession,
    query_utf8: *const c_char,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let q = unsafe { cstr_to_str(query_utf8) };
        s.session.set_search_query(
            q,
            SearchOptions {
                case_sensitive,
                whole_word,
                use_regex,
            },
        );
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_find_next(session: *mut KMarkSession) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.find_next())).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_find_prev(session: *mut KMarkSession) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| s.session.find_prev())).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_replace_one(
    session: *mut KMarkSession,
    replacement_utf8: *const c_char,
) -> bool {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let rep = unsafe { cstr_to_str(replacement_utf8) };
        s.session.replace_current_match(rep)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_replace_all(
    session: *mut KMarkSession,
    replacement_utf8: *const c_char,
) -> usize {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let rep = unsafe { cstr_to_str(replacement_utf8) };
        s.session.replace_all_matches(rep)
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_jump_to_offset(session: *mut KMarkSession, byte_offset: usize) {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        s.session.jump_to_byte_offset(byte_offset);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_outline(session: *mut KMarkSession) -> KMarkOutlineView {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return KMarkOutlineView::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        s.outline_items.clear();
        s.outline_text_pool.clear();

        if let Some(doc) = s.session.active_doc() {
            for entry in &doc.outline {
                let off = s.outline_text_pool.len() as u32;
                let bytes = entry.title.as_bytes();
                s.outline_text_pool.extend_from_slice(bytes);
                s.outline_items.push(KMarkOutlineItem {
                    level: entry.level,
                    byte_offset: entry.byte_offset,
                    title_offset: off,
                    title_len: bytes.len() as u32,
                });
            }
        }

        KMarkOutlineView {
            items_ptr: s.outline_items.as_ptr(),
            items_len: s.outline_items.len(),
            text_pool_ptr: s.outline_text_pool.as_ptr(),
            text_pool_len: s.outline_text_pool.len(),
        }
    }))
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_toolbar_state(session: *const KMarkSession) -> KMarkToolbarState {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return KMarkToolbarState::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        let st = s.session.toolbar_state();
        let vm = match st.view_mode {
            ViewMode::Wysiwyg => 0,
            ViewMode::LivePreview => 1,
            ViewMode::Source => 2,
            ViewMode::PlainText => 3,
        };
        let col_align = match st.table_col_align {
            ColumnAlign::Left => 0,
            ColumnAlign::Center => 1,
            ColumnAlign::Right => 2,
        };
        KMarkToolbarState {
            heading_level: st.heading_level,
            view_mode: vm,
            table_col_align: col_align,
            bold: st.bold,
            italic: st.italic,
            strike: st.strike,
            highlight: st.highlight,
            inline_code: st.inline_code,
            in_table: st.in_table,
            in_code_block: st.in_code_block,
            in_blockquote: st.in_blockquote,
            in_bullet_list: st.in_bullet_list,
            in_ordered_list: st.in_ordered_list,
            in_task_list: st.in_task_list,
            can_undo: st.can_undo,
            can_redo: st.can_redo,
            is_dirty: st.is_dirty,
            line_1based: st.line_1based,
            col_1based: st.col_1based,
            char_count: st.char_count,
            word_count: st.word_count,
            search_match_count: s.session.search.matches.len(),
            search_active_1based: s
                .session
                .search
                .active_index
                .map_or(0, |i| i.saturating_add(1)),
        }
    }))
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "C" fn kmark_session_render_frame(
    session: *mut KMarkSession,
    viewport_w: f32,
    viewport_h: f32,
) -> KMarkFrameView {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return KMarkFrameView::default();
    };
    catch_unwind(AssertUnwindSafe(|| {
        let vw_bits = viewport_w.to_bits();
        let vh_bits = viewport_h.to_bits();

        if let Some(pre_stamp) = s.session.render_state_stamp()
            && !pre_stamp.needs_scroll_to_caret
            && let Some(cached) = s.cached_frame.as_ref()
            && cached.vw_bits == vw_bits
            && cached.vh_bits == vh_bits
            && cached.measurer_epoch == s.measurer.epoch
            && cached.stamp == pre_stamp
        {
            return KMarkFrameView {
                viewport_w: cached.viewport_w,
                viewport_h: cached.viewport_h,
                scroll_y: cached.scroll_y,
                total_height: cached.total_height,
                caret_rect: cached.caret_rect,
                cmds_ptr: s.frame_cmds.as_ptr(),
                cmds_len: s.frame_cmds.len(),
                text_pool_ptr: s.frame_text_pool.as_ptr(),
                text_pool_len: s.frame_text_pool.len(),
            };
        }

        s.frame_cmds.clear();
        s.frame_text_pool.clear();
        s.measurer.image_cache.clear();

        let (frame_vw, frame_vh, frame_scroll_y, frame_total_h, frame_caret) = {
            let Some(frame) = s
                .session
                .render_viewport(viewport_w, viewport_h, &mut s.measurer)
            else {
                s.cached_frame = None;
                return KMarkFrameView::default();
            };

            for cmd in &frame.draw_cmds {
                match cmd {
                    DrawCmd::FillRect {
                        rect,
                        color,
                        radius,
                    } => {
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 0,
                            font_role: 0,
                            level: 0,
                            flags: 0,
                            rect: (*rect).into(),
                            aux: *radius,
                            color: (*color).into(),
                            text_offset: 0,
                            text_len: 0,
                        });
                    }
                    DrawCmd::StrokeRect {
                        rect,
                        color,
                        stroke_width,
                        radius,
                    } => {
                        let encoded_rad = ((*radius).clamp(0.0, 60.0) * 2.0).round() as u8;
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 1,
                            font_role: 0,
                            level: encoded_rad,
                            flags: 0,
                            rect: (*rect).into(),
                            aux: *stroke_width,
                            color: (*color).into(),
                            text_offset: 0,
                            text_len: 0,
                        });
                    }
                    DrawCmd::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        color,
                        width,
                    } => {
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 2,
                            font_role: 0,
                            level: 0,
                            flags: 0,
                            rect: KMarkRectF {
                                x: *x1,
                                y: *y1,
                                w: *x2,
                                h: *y2,
                            },
                            aux: *width,
                            color: (*color).into(),
                            text_offset: 0,
                            text_len: 0,
                        });
                    }
                    DrawCmd::HeadingBars {
                        x,
                        top_y,
                        height,
                        level,
                        color,
                    } => {
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 3,
                            font_role: 0,
                            level: *level,
                            flags: 0,
                            rect: KMarkRectF {
                                x: *x,
                                y: *top_y,
                                w: 0.0,
                                h: *height,
                            },
                            aux: *height,
                            color: (*color).into(),
                            text_offset: 0,
                            text_len: 0,
                        });
                    }
                    DrawCmd::Checkbox { rect, checked } => {
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 4,
                            font_role: 0,
                            level: 0,
                            flags: if *checked { 4 } else { 0 },
                            rect: (*rect).into(),
                            aux: 0.0,
                            color: KMarkRgba::default(),
                            text_offset: 0,
                            text_len: 0,
                        });
                    }
                    DrawCmd::Text {
                        x,
                        baseline_y,
                        text,
                        role,
                        size_px,
                        bold,
                        italic,
                        color,
                    } => {
                        let off = s.frame_text_pool.len() as u32;
                        let bytes = text.as_bytes();
                        s.frame_text_pool.extend_from_slice(bytes);
                        let mut flags = 0u8;
                        if *bold {
                            flags |= 1;
                        }
                        if *italic {
                            flags |= 2;
                        }
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 5,
                            font_role: QtBridgeMeasurer::role_code(*role),
                            level: 0,
                            flags,
                            rect: KMarkRectF {
                                x: *x,
                                y: *baseline_y,
                                w: 0.0,
                                h: 0.0,
                            },
                            aux: *size_px,
                            color: (*color).into(),
                            text_offset: off,
                            text_len: bytes.len() as u32,
                        });
                    }
                    DrawCmd::Image { rect, src } => {
                        let off = s.frame_text_pool.len() as u32;
                        let bytes = src.as_bytes();
                        s.frame_text_pool.extend_from_slice(bytes);
                        s.frame_cmds.push(KMarkDrawCmd {
                            tag: 6,
                            font_role: 0,
                            level: 0,
                            flags: 0,
                            rect: (*rect).into(),
                            aux: 0.0,
                            color: KMarkRgba::default(),
                            text_offset: off,
                            text_len: bytes.len() as u32,
                        });
                    }
                }
            }

            (
                frame.viewport_w,
                frame.viewport_h,
                frame.scroll_y,
                frame.total_height,
                frame.caret_viewport_rect.into(),
            )
        };

        if let Some(post_stamp) = s.session.render_state_stamp() {
            s.cached_frame = Some(CachedRenderedFrame {
                stamp: post_stamp,
                measurer_epoch: s.measurer.epoch,
                vw_bits,
                vh_bits,
                viewport_w: frame_vw,
                viewport_h: frame_vh,
                scroll_y: frame_scroll_y,
                total_height: frame_total_h,
                caret_rect: frame_caret,
            });
        }

        KMarkFrameView {
            viewport_w: frame_vw,
            viewport_h: frame_vh,
            scroll_y: frame_scroll_y,
            total_height: frame_total_h,
            caret_rect: frame_caret,
            cmds_ptr: s.frame_cmds.as_ptr(),
            cmds_len: s.frame_cmds.len(),
            text_pool_ptr: s.frame_text_pool.as_ptr(),
            text_pool_len: s.frame_text_pool.len(),
        }
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn ffi_lifecycle_guide_reference_and_frame_generation_work() {
        let ptr = kmark_session_new();
        assert!(!ptr.is_null());

        kmark_session_set_ui_language(ptr, 0);
        assert_eq!(kmark_session_ui_language(ptr), 0);

        kmark_session_open_guide_tab(ptr);
        let outline = kmark_session_outline(ptr);
        assert!(outline.items_len >= 3);

        let frame = kmark_session_render_frame(ptr, 1280.0, 800.0);
        assert!(frame.cmds_len > 10);
        assert!(frame.total_height > 200.0);

        kmark_session_open_reference_tab(ptr);
        let ref_outline = kmark_session_outline(ptr);
        assert!(ref_outline.items_len >= 8);

        let html_ptr = kmark_session_export_html_alloc(ptr);
        assert!(!html_ptr.is_null());
        kmark_string_free(html_ptr);

        kmark_session_free(ptr);
    }

    extern "C" fn test_img_size_fn(
        _user_data: *mut c_void,
        _src_utf8: *const c_char,
        _src_len: usize,
        out_w: *mut f32,
        out_h: *mut f32,
    ) -> bool {
        unsafe {
            *out_w = 600.0;
            *out_h = 300.0;
        }
        true
    }

    #[test]
    fn ffi_image_caption_size_and_update_roundtrip() {
        let ptr = kmark_session_new();
        assert!(!ptr.is_null());
        kmark_session_set_image_measurer(ptr, ptr::null_mut(), Some(test_img_size_fn));

        let alt = CString::new("Diagramm").expect("cstring");
        let url = CString::new("assets/arch.png").expect("cstring");
        let cap = CString::new("Abb. 1: Übersicht").expect("cstring");
        kmark_session_insert_image_ex(ptr, alt.as_ptr(), url.as_ptr(), cap.as_ptr(), 1, 50);

        let frame = kmark_session_render_frame(ptr, 900.0, 600.0);
        assert!(frame.cmds_len > 0);
        let cmds = unsafe { std::slice::from_raw_parts(frame.cmds_ptr, frame.cmds_len) };
        let img_rect1 = cmds
            .iter()
            .find(|c| c.tag == 6)
            .expect("expected rendered image DrawCmd")
            .rect;

        // Hit-test directly on the rendered image
        let mut info = KMarkImageInfo::default();
        assert!(kmark_session_image_at(
            ptr,
            img_rect1.x + 12.0,
            img_rect1.y + 12.0,
            &mut info
        ));
        assert_eq!(info.size_kind, 1);
        assert_eq!(info.size_value, 50);
        assert!(!info.caption_alloc.is_null());
        assert_eq!(
            unsafe { cstr_to_str(info.caption_alloc) },
            "Abb. 1: Übersicht"
        );

        // Hit-test on the caption below the image as well
        let mut cap_hit = KMarkImageInfo::default();
        assert!(kmark_session_image_at(
            ptr,
            img_rect1.x + 12.0,
            img_rect1.y + img_rect1.h + 10.0,
            &mut cap_hit
        ));
        kmark_image_info_free(&mut cap_hit);

        let new_url = CString::new("assets/arch-v2.png").expect("cstring");
        let new_cap = CString::new("Abb. 1: Aktualisiert").expect("cstring");
        kmark_session_update_image(
            ptr,
            info.start_byte,
            info.end_byte,
            info.alt_alloc,
            new_url.as_ptr(),
            new_cap.as_ptr(),
            1,
            75,
        );
        kmark_image_info_free(&mut info);

        let frame2 = kmark_session_render_frame(ptr, 900.0, 600.0);
        let cmds2 = unsafe { std::slice::from_raw_parts(frame2.cmds_ptr, frame2.cmds_len) };
        let img_rect2 = cmds2
            .iter()
            .find(|c| c.tag == 6)
            .expect("expected updated image DrawCmd")
            .rect;
        assert!(img_rect2.w > img_rect1.w);

        let mut updated = KMarkImageInfo::default();
        assert!(kmark_session_image_at(
            ptr,
            img_rect2.x + 12.0,
            img_rect2.y + 12.0,
            &mut updated
        ));
        assert_eq!(updated.size_kind, 1);
        assert_eq!(updated.size_value, 75);
        assert_eq!(
            unsafe { cstr_to_str(updated.src_alloc) },
            "assets/arch-v2.png"
        );
        assert_eq!(
            unsafe { cstr_to_str(updated.caption_alloc) },
            "Abb. 1: Aktualisiert"
        );
        kmark_image_info_free(&mut updated);

        kmark_session_free(ptr);
    }

    #[test]
    fn ffi_layout_settings_and_table_toolbar_state_roundtrip() {
        let ptr = kmark_session_new();
        assert!(!ptr.is_null());

        let next_cfg = KMarkLayoutSettings {
            view_mode: 1,
            heading_marks: 2,
            word_wrap: false,
            show_line_numbers: true,
            focus_mode: true,
            typewriter_mode: true,
            base_font_px: 19.0,
            line_spacing: 1.55,
            max_prose_width: 960.0,
        };
        kmark_session_set_layout_settings(ptr, next_cfg);
        let got = kmark_session_get_layout_settings(ptr);
        assert_eq!(got.view_mode, 1);
        assert_eq!(got.heading_marks, 2);
        assert!(!got.word_wrap);
        assert!(got.show_line_numbers);
        assert!(got.focus_mode);
        assert!(got.typewriter_mode);
        assert!((got.base_font_px - 19.0).abs() < 0.01);
        assert!((got.line_spacing - 1.55).abs() < 0.01);
        assert!((got.max_prose_width - 960.0).abs() < 0.01);

        // Insert a 2x2 table, align first column center, and verify toolbar_state
        kmark_session_insert_table(ptr, 2, 2);
        let st_before = kmark_session_toolbar_state(ptr);
        assert!(st_before.in_table);
        assert_eq!(st_before.table_col_align, 0);

        kmark_session_exec_command(ptr, 37); // KMARK_ACT_TABLE_ALIGN_CENTER
        let st_after = kmark_session_toolbar_state(ptr);
        assert!(st_after.in_table);
        assert_eq!(st_after.table_col_align, 1);

        kmark_session_free(ptr);
    }

    extern "C" fn test_single_pass_run_fn(
        user_data: *mut c_void,
        utf8_ptr: *const c_char,
        utf8_len: usize,
        utf16_ends_ptr: *const u32,
        grapheme_count: usize,
        _font_role: u8,
        size_px: f32,
        _bold: bool,
        _italic: bool,
        out_advances_ptr: *mut f32,
    ) {
        let counter = unsafe { &*(user_data.cast::<AtomicUsize>()) };
        counter.fetch_add(1, Ordering::SeqCst);

        let bytes = unsafe { std::slice::from_raw_parts(utf8_ptr.cast::<u8>(), utf8_len) };
        let text = std::str::from_utf8(bytes).expect("valid utf8");
        let utf16_ends = unsafe { std::slice::from_raw_parts(utf16_ends_ptr, grapheme_count) };
        let out_advances =
            unsafe { std::slice::from_raw_parts_mut(out_advances_ptr, grapheme_count) };

        let total_utf16 = text.encode_utf16().count() as u32;
        assert_eq!(
            utf16_ends.last().copied().unwrap_or(0),
            total_utf16,
            "final grapheme UTF-16 offset must equal QString UTF-16 length"
        );

        for (i, &u16_end) in utf16_ends.iter().enumerate() {
            out_advances[i] = (u16_end as f32) * (size_px * 0.55);
        }
    }

    #[test]
    fn ffi_single_pass_run_measurer_handles_utf16_surrogates_and_skips_blink_relayout() {
        let ptr = kmark_session_new();
        assert!(!ptr.is_null());

        let call_counter = AtomicUsize::new(0);
        kmark_session_set_run_measurer(
            ptr,
            (&raw const call_counter).cast_mut().cast::<c_void>(),
            Some(test_single_pass_run_fn),
        );

        // 5,000+ byte single paragraph mixing ASCII, umlauts, CJK, and 4-byte ZWJ surrogate-pair emojis
        let chunk = "Größe 日本語 👨‍👩‍👧‍👦 Textmessung ohne O(n^2) Präfix-Schleife. ";
        let long_para = chunk.repeat(90);
        assert!(long_para.len() > 5_000);
        let c_para = CString::new(long_para).expect("cstring");
        kmark_session_insert_text(ptr, c_para.as_ptr());

        let frame1 = kmark_session_render_frame(ptr, 1000.0, 700.0);
        assert!(frame1.cmds_len > 3);
        let calls_after_first_frame = call_counter.load(Ordering::SeqCst);
        // Previously O(n^2) made ~5,000+ FFI calls for one paragraph; single-pass makes <= 30 (1 per wrapped line/span)!
        assert!(
            (1..60).contains(&calls_after_first_frame),
            "expected single-pass O(n) run layout calls, got {calls_after_first_frame}"
        );

        // Simulating 530 ms cursor-blink repaint with identical session/viewport state:
        // cached_frame hits in O(1) with ZERO layout/measurement FFI calls!
        let frame2 = kmark_session_render_frame(ptr, 1000.0, 700.0);
        assert_eq!(frame2.cmds_len, frame1.cmds_len);
        assert_eq!(
            call_counter.load(Ordering::SeqCst),
            calls_after_first_frame,
            "blink repaint must reuse cached frame without re-running layout"
        );

        kmark_session_free(ptr);
    }
}
