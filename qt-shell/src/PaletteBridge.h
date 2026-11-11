#pragma once

#include <QColor>
#include <QPalette>

#include "kmark_ffi.h"

namespace kmark {

inline KMarkRgba toFfiColor(const QColor& c) {
    const QColor rgb = c.toRgb();
    return KMarkRgba{
        static_cast<uint8_t>(rgb.red()),
        static_cast<uint8_t>(rgb.green()),
        static_cast<uint8_t>(rgb.blue()),
        static_cast<uint8_t>(rgb.alpha()),
    };
}

inline QColor toQColor(KMarkRgba c) {
    return QColor(c.r, c.g, c.b, c.a);
}

/// Builds a KMarkPalette synchronized with the active KDE Plasma QPalette (Breeze Light / Dark / Custom).
inline KMarkPalette paletteFromQt(const QPalette& qpal) {
    const QColor base = qpal.color(QPalette::Base);
    const bool isDark = base.lightness() < 128;

    KMarkPalette pal = isDark ? kmark_palette_breeze_dark() : kmark_palette_breeze_light();
    pal.dark = isDark;
    pal.canvas_bg = toFfiColor(base);
    pal.gutter_bg = toFfiColor(qpal.color(QPalette::AlternateBase));
    pal.text_fg = toFfiColor(qpal.color(QPalette::Text));
    pal.caret = pal.text_fg;

    const QColor placeholder = qpal.color(QPalette::PlaceholderText);
    if (placeholder.alpha() > 0) {
        QColor muted = placeholder;
        muted.setAlpha(255);
        pal.muted_fg = toFfiColor(muted);
        pal.gutter_fg = pal.muted_fg;
    }

    const QColor link = qpal.color(QPalette::Link);
    if (link.isValid()) {
        pal.link_fg = toFfiColor(link);
    }

    const QColor highlight = qpal.color(QPalette::Highlight);
    if (highlight.isValid()) {
        pal.accent = toFfiColor(highlight);
        QColor selBg = highlight;
        selBg.setAlpha(isDark ? 92 : 76);
        pal.selection_bg = toFfiColor(selBg);
        pal.selection_fg = pal.text_fg;
    }

    return pal;
}

} // namespace kmark
