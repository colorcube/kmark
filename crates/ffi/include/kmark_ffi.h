#ifndef KMARK_FFI_H
#define KMARK_FFI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KMarkSession KMarkSession;

typedef struct KMarkRgba {
    uint8_t r;
    uint8_t g;
    uint8_t b;
    uint8_t a;
} KMarkRgba;

typedef struct KMarkRectF {
    float x;
    float y;
    float w;
    float h;
} KMarkRectF;

typedef struct KMarkFontMetrics {
    float ascent;
    float descent;
    float line_height;
    float space_width;
} KMarkFontMetrics;

typedef struct KMarkPalette {
    bool dark;
    KMarkRgba canvas_bg;
    KMarkRgba gutter_bg;
    KMarkRgba gutter_fg;
    KMarkRgba gutter_border;
    KMarkRgba text_fg;
    KMarkRgba muted_fg;
    KMarkRgba link_fg;
    KMarkRgba accent;
    KMarkRgba selection_bg;
    KMarkRgba selection_fg;
    KMarkRgba search_match_bg;
    KMarkRgba search_current_bg;
    KMarkRgba code_bg;
    KMarkRgba inline_code_bg;
    KMarkRgba highlight_bg;
    KMarkRgba quote_bar;
    KMarkRgba quote_fg;
    KMarkRgba rule_line;
    KMarkRgba table_header_bg;
    KMarkRgba table_border;
    KMarkRgba caret;
} KMarkPalette;

/* Draw command tags */
enum KMarkDrawTag {
    KMARK_CMD_FILL_RECT    = 0,
    KMARK_CMD_STROKE_RECT  = 1,
    KMARK_CMD_LINE         = 2,
    KMARK_CMD_HEADING_BARS = 3,
    KMARK_CMD_CHECKBOX     = 4,
    KMARK_CMD_TEXT         = 5,
    KMARK_CMD_IMAGE        = 6,
};

/* Pointer hit result tags */
enum KMarkPointerTag {
    KMARK_PTR_HANDLED            = 0,
    KMARK_PTR_OPEN_URL           = 1,
    KMARK_PTR_EDIT_CODE_LANGUAGE = 2,
    KMARK_PTR_EDIT_CALLOUT_KIND  = 3,
};

/* Callout / GFM Alert kinds (for kmark_session_set_callout_kind) */
enum KMarkAlertKind {
    KMARK_ALERT_NONE      = 0,
    KMARK_ALERT_NOTE      = 1,
    KMARK_ALERT_TIP       = 2,
    KMARK_ALERT_IMPORTANT = 3,
    KMARK_ALERT_WARNING   = 4,
    KMARK_ALERT_CAUTION   = 5,
};

/* UI languages */
enum KMarkUiLanguage {
    KMARK_LANG_ENGLISH = 0,
    KMARK_LANG_GERMAN  = 1,
};

/* Font roles */
enum KMarkFontRole {
    KMARK_FONT_SANS = 0,
    KMARK_FONT_MONO = 1,
};

/* View modes */
enum KMarkViewMode {
    KMARK_VIEW_WYSIWYG      = 0,
    KMARK_VIEW_LIVE_PREVIEW = 1,
    KMARK_VIEW_SOURCE       = 2,
    KMARK_VIEW_PLAIN_TEXT   = 3,
};

/* Heading indicator styles */
enum KMarkHeadingMarks {
    KMARK_MARKS_SLANTED = 0,
    KMARK_MARKS_HASH    = 1,
    KMARK_MARKS_NONE    = 2,
};

/* Table column alignments */
enum KMarkColumnAlign {
    KMARK_ALIGN_LEFT   = 0,
    KMARK_ALIGN_CENTER = 1,
    KMARK_ALIGN_RIGHT  = 2,
};

/* Action command IDs */
enum KMarkCommandId {
    KMARK_ACT_UNDO                  = 0,
    KMARK_ACT_REDO                  = 1,
    KMARK_ACT_SELECT_ALL            = 2,
    KMARK_ACT_BOLD                  = 3,
    KMARK_ACT_ITALIC                = 4,
    KMARK_ACT_STRIKE                = 5,
    KMARK_ACT_HIGHLIGHT             = 6,
    KMARK_ACT_INLINE_CODE           = 7,
    KMARK_ACT_HEADING_0             = 10,
    KMARK_ACT_HEADING_1             = 11,
    KMARK_ACT_HEADING_2             = 12,
    KMARK_ACT_HEADING_3             = 13,
    KMARK_ACT_HEADING_4             = 14,
    KMARK_ACT_HEADING_5             = 15,
    KMARK_ACT_HEADING_6             = 16,
    KMARK_ACT_LIST_BULLET           = 20,
    KMARK_ACT_LIST_ORDERED          = 21,
    KMARK_ACT_LIST_TASK             = 22,
    KMARK_ACT_BLOCKQUOTE            = 23,
    KMARK_ACT_ALERT_NOTE            = 24,
    KMARK_ACT_ALERT_TIP             = 25,
    KMARK_ACT_ALERT_IMPORTANT       = 26,
    KMARK_ACT_ALERT_WARNING         = 27,
    KMARK_ACT_ALERT_CAUTION         = 28,
    KMARK_ACT_HORIZONTAL_RULE       = 29,
    KMARK_ACT_TABLE_ROW_ABOVE       = 30,
    KMARK_ACT_TABLE_ROW_BELOW       = 31,
    KMARK_ACT_TABLE_ROW_DELETE      = 32,
    KMARK_ACT_TABLE_COL_LEFT        = 33,
    KMARK_ACT_TABLE_COL_RIGHT       = 34,
    KMARK_ACT_TABLE_COL_DELETE      = 35,
    KMARK_ACT_TABLE_ALIGN_LEFT      = 36,
    KMARK_ACT_TABLE_ALIGN_CENTER    = 37,
    KMARK_ACT_TABLE_ALIGN_RIGHT     = 38,
    KMARK_ACT_VIEW_WYSIWYG          = 40,
    KMARK_ACT_VIEW_LIVE_PREVIEW     = 41,
    KMARK_ACT_VIEW_SOURCE           = 42,
    KMARK_ACT_VIEW_PLAIN_TEXT       = 43,
    KMARK_ACT_TOGGLE_LINE_NUMBERS   = 44,
    KMARK_ACT_TOGGLE_WORD_WRAP      = 45,
    KMARK_ACT_TOGGLE_FOCUS_MODE     = 46,
    KMARK_ACT_TOGGLE_TYPEWRITER     = 47,
    KMARK_ACT_ZOOM_IN               = 48,
    KMARK_ACT_ZOOM_OUT              = 49,
    KMARK_ACT_ZOOM_RESET            = 50,
    KMARK_ACT_MARKS_SLANTED         = 51,
    KMARK_ACT_MARKS_HASH            = 52,
    KMARK_ACT_MARKS_NONE            = 53,
};

/* Key codes */
enum KMarkKeyCode {
    KMARK_KEY_LEFT      = 0,
    KMARK_KEY_RIGHT     = 1,
    KMARK_KEY_UP        = 2,
    KMARK_KEY_DOWN      = 3,
    KMARK_KEY_HOME      = 4,
    KMARK_KEY_END       = 5,
    KMARK_KEY_PAGE_UP   = 6,
    KMARK_KEY_PAGE_DOWN = 7,
    KMARK_KEY_BACKSPACE = 8,
    KMARK_KEY_DELETE    = 9,
    KMARK_KEY_ENTER     = 10,
    KMARK_KEY_TAB       = 11,
    KMARK_KEY_BACKTAB   = 12,
    KMARK_KEY_ESCAPE    = 13,
};

typedef struct KMarkDrawCmd {
    uint8_t tag;
    uint8_t font_role;
    uint8_t level;   /* heading level or (stroke_radius * 2) */
    uint8_t flags;   /* bit 0 = bold, bit 1 = italic, bit 2 = checked */
    KMarkRectF rect; /* For LINE: x=x1, y=y1, w=x2, h=y2; for TEXT: x=x, y=baseline_y */
    float aux;       /* radius / stroke_width / size_px / bar_height */
    KMarkRgba color;
    uint32_t text_offset;
    uint32_t text_len;
} KMarkDrawCmd;

/* Image sizing modes */
enum KMarkImageSizeKind {
    KMARK_IMG_SIZE_AUTO    = 0,
    KMARK_IMG_SIZE_PERCENT = 1,
    KMARK_IMG_SIZE_PIXELS  = 2,
};

typedef struct KMarkPointerResult {
    uint8_t tag;        /* KMarkPointerTag */
    size_t byte_offset; /* For KMARK_PTR_EDIT_CODE_LANGUAGE / KMARK_PTR_EDIT_CALLOUT_KIND: open_line_start */
    char* str_alloc;    /* URL, current language, or callout tag; free with kmark_string_free if non-NULL */
} KMarkPointerResult;

typedef struct KMarkImageInfo {
    size_t start_byte;
    size_t end_byte;
    uint8_t size_kind;   /* KMarkImageSizeKind */
    uint16_t size_value; /* percent (5..100) or pixels (16..4096), 0 when auto */
    char* alt_alloc;     /* free via kmark_image_info_free */
    char* src_alloc;     /* free via kmark_image_info_free */
    char* caption_alloc; /* NULL when no caption; free via kmark_image_info_free */
} KMarkImageInfo;

typedef struct KMarkFrameView {
    float viewport_w;
    float viewport_h;
    float scroll_y;
    float total_height;
    KMarkRectF caret_rect;
    const KMarkDrawCmd* cmds_ptr;
    size_t cmds_len;
    const uint8_t* text_pool_ptr;
    size_t text_pool_len;
} KMarkFrameView;

typedef struct KMarkToolbarState {
    uint8_t heading_level;
    uint8_t view_mode;
    uint8_t table_col_align; /* KMarkColumnAlign */
    bool bold;
    bool italic;
    bool strike;
    bool highlight;
    bool inline_code;
    bool in_table;
    bool in_code_block;
    bool in_blockquote;
    bool in_bullet_list;
    bool in_ordered_list;
    bool in_task_list;
    bool can_undo;
    bool can_redo;
    bool is_dirty;
    size_t line_1based;
    size_t col_1based;
    size_t char_count;
    size_t word_count;
    size_t search_match_count;
    size_t search_active_1based;
} KMarkToolbarState;

typedef struct KMarkLayoutSettings {
    uint8_t view_mode;     /* KMarkViewMode */
    uint8_t heading_marks; /* KMarkHeadingMarks */
    bool word_wrap;
    bool show_line_numbers;
    bool focus_mode;
    bool typewriter_mode;
    float base_font_px;
    float line_spacing;
    float max_prose_width;
} KMarkLayoutSettings;

typedef struct KMarkOutlineItem {
    uint8_t level;
    size_t byte_offset;
    uint32_t title_offset;
    uint32_t title_len;
} KMarkOutlineItem;

typedef struct KMarkOutlineView {
    const KMarkOutlineItem* items_ptr;
    size_t items_len;
    const uint8_t* text_pool_ptr;
    size_t text_pool_len;
} KMarkOutlineView;

typedef void (*KMarkMetricsFn)(
    void* user_data,
    uint8_t font_role,
    float size_px,
    bool bold,
    bool italic,
    KMarkFontMetrics* out_metrics
);

typedef float (*KMarkWidthFn)(
    void* user_data,
    const char* utf8_ptr,
    size_t utf8_len,
    uint8_t font_role,
    float size_px,
    bool bold,
    bool italic
);

typedef bool (*KMarkImageSizeFn)(
    void* user_data,
    const char* src_utf8,
    size_t src_len,
    float* out_w,
    float* out_h
);

/* Lifecycle, UI language & Qt font/image measurement callbacks */
KMarkSession* kmark_session_new(void);
void kmark_session_free(KMarkSession* session);
void kmark_session_set_ui_language(KMarkSession* session, uint8_t lang);
uint8_t kmark_session_ui_language(const KMarkSession* session);
void kmark_session_set_measurer(
    KMarkSession* session,
    void* user_data,
    KMarkMetricsFn metrics_fn,
    KMarkWidthFn width_fn
);
void kmark_session_set_image_measurer(
    KMarkSession* session,
    void* user_data,
    KMarkImageSizeFn image_fn
);
void kmark_session_clear_measure_cache(KMarkSession* session);

/* Palette & layout settings synchronization */
KMarkPalette kmark_palette_breeze_light(void);
KMarkPalette kmark_palette_breeze_dark(void);
void kmark_session_set_palette(KMarkSession* session, KMarkPalette palette);
KMarkLayoutSettings kmark_session_get_layout_settings(const KMarkSession* session);
void kmark_session_set_layout_settings(KMarkSession* session, KMarkLayoutSettings settings);

/* Multi-tab & file persistence */
size_t kmark_session_tab_count(const KMarkSession* session);
size_t kmark_session_active_tab(const KMarkSession* session);
void kmark_session_set_active_tab(KMarkSession* session, size_t idx);
size_t kmark_session_new_tab(KMarkSession* session);
size_t kmark_session_open_guide_tab(KMarkSession* session);
size_t kmark_session_open_reference_tab(KMarkSession* session);
void kmark_session_close_tab(KMarkSession* session, size_t idx);
bool kmark_session_tab_is_dirty(const KMarkSession* session, size_t idx);
bool kmark_session_tab_has_path(const KMarkSession* session, size_t idx);
char* kmark_session_tab_title_alloc(const KMarkSession* session, size_t idx);
char* kmark_session_tab_path_alloc(const KMarkSession* session, size_t idx);
bool kmark_session_open_file(KMarkSession* session, const char* path_utf8);
bool kmark_session_save_active(KMarkSession* session);
bool kmark_session_save_active_as(KMarkSession* session, const char* path_utf8);
void kmark_session_flush_recovery(const KMarkSession* session);
char* kmark_session_last_error_alloc(const KMarkSession* session);

/* Clipboard, HTML export & strings */
char* kmark_session_selected_text_alloc(const KMarkSession* session);
char* kmark_session_export_html_alloc(const KMarkSession* session);
char* kmark_session_active_code_language_alloc(const KMarkSession* session);
void kmark_string_free(char* s);

/* Commands, typing, IME, keyboard, mouse, drag-drop & scrolling */
void kmark_session_exec_command(KMarkSession* session, uint32_t cmd_id);
void kmark_session_insert_code_fence(KMarkSession* session, const char* language_utf8);
void kmark_session_set_code_fence_language(
    KMarkSession* session,
    bool has_offset,
    size_t open_line_start,
    const char* language_utf8
);
void kmark_session_set_callout_kind(
    KMarkSession* session,
    bool has_offset,
    size_t open_line_start,
    uint8_t alert_kind
);
void kmark_session_insert_link(KMarkSession* session, const char* text_utf8, const char* url_utf8);
void kmark_session_insert_image(KMarkSession* session, const char* alt_utf8, const char* url_utf8);
void kmark_session_insert_image_ex(
    KMarkSession* session,
    const char* alt_utf8,
    const char* url_utf8,
    const char* caption_utf8,
    uint8_t size_kind,
    uint16_t size_value
);
void kmark_session_update_image(
    KMarkSession* session,
    size_t start_byte,
    size_t end_byte,
    const char* alt_utf8,
    const char* url_utf8,
    const char* caption_utf8,
    uint8_t size_kind,
    uint16_t size_value
);
bool kmark_session_image_at(
    const KMarkSession* session,
    float vx,
    float vy,
    KMarkImageInfo* out_info
);
void kmark_image_info_free(KMarkImageInfo* info);
void kmark_session_insert_table(KMarkSession* session, size_t cols, size_t rows);
void kmark_session_insert_text(KMarkSession* session, const char* text_utf8);
void kmark_session_set_ime_preedit(KMarkSession* session, const char* preedit_utf8);
bool kmark_session_key_press(
    KMarkSession* session,
    uint32_t key_code,
    bool shift,
    bool ctrl,
    bool alt
);
KMarkPointerResult kmark_session_pointer_press(
    KMarkSession* session,
    float vx,
    float vy,
    uint8_t click_count,
    bool shift,
    bool ctrl,
    bool alt
);
void kmark_session_pointer_drag(KMarkSession* session, float vx, float vy);
void kmark_session_drop_cursor_at(KMarkSession* session, float vx, float vy);
bool kmark_session_hover_interactive(const KMarkSession* session, float vx, float vy, bool ctrl);
void kmark_session_set_scroll_y(KMarkSession* session, float scroll_y);
void kmark_session_scroll_pixels(KMarkSession* session, float delta_y);

/* Find & Replace */
void kmark_session_set_search(
    KMarkSession* session,
    const char* query_utf8,
    bool case_sensitive,
    bool whole_word,
    bool use_regex
);
bool kmark_session_find_next(KMarkSession* session);
bool kmark_session_find_prev(KMarkSession* session);
bool kmark_session_replace_one(KMarkSession* session, const char* replacement_utf8);
size_t kmark_session_replace_all(KMarkSession* session, const char* replacement_utf8);

/* Outline & Viewport rendering */
void kmark_session_jump_to_offset(KMarkSession* session, size_t byte_offset);
KMarkOutlineView kmark_session_outline(KMarkSession* session);
KMarkToolbarState kmark_session_toolbar_state(const KMarkSession* session);
KMarkFrameView kmark_session_render_frame(
    KMarkSession* session,
    float viewport_w,
    float viewport_h
);

#ifdef __cplusplus
}
#endif

#endif /* KMARK_FFI_H */
