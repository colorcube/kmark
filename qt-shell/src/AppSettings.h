#pragma once

#include <QByteArray>
#include <QColor>
#include <QFont>
#include <QList>
#include <QPalette>
#include <QString>
#include <cstdint>

#include "I18n.h"
#include "kmark_ffi.h"

namespace kmark {

struct AppearanceColors {
    QColor canvasBg{252, 252, 252};
    QColor textFg{35, 38, 41};
    QColor accent{61, 174, 233};
    QColor linkFg{41, 128, 185};
    QColor codeBg{243, 244, 246};
    QColor quoteBar{189, 195, 199};
    QColor highlightBg{255, 235, 110, 175};
    QColor tableHeaderBg{239, 241, 244};

    bool operator==(const AppearanceColors& other) const = default;

    KMarkPalette toFfiPalette() const;
    static AppearanceColors fromQtPalette(const QPalette& qpal);
};

struct AppearancePreset {
    QString id;
    QString nameEn;
    QString nameDe;
    QString descEn;
    QString descDe;
    bool isBuiltin = false;

    bool useSystemSans = true;
    QString sansFamily;
    bool useSystemMono = true;
    QString monoFamily;

    float baseFontPx = 16.0f;
    float lineSpacing = 1.42f;
    float maxProseWidth = 880.0f;
    uint8_t headingMarks = KMARK_MARKS_SLANTED;

    bool useSystemColors = false;
    AppearanceColors colors;

    QString displayName(UiLang lang) const {
        return (lang == UiLang::German && !nameDe.isEmpty()) ? nameDe : nameEn;
    }

    QString displayDescription(UiLang lang) const {
        return (lang == UiLang::German && !descDe.isEmpty()) ? descDe : descEn;
    }
};

const QList<AppearancePreset>& builtinPresets();

struct AppSettings {
    /* Active preset identifier ("builtin:...", "user:...", or empty when customized) */
    QString activePresetId = QStringLiteral("builtin:system");

    /* Fonts & Typography */
    bool useSystemSans = true;
    QString sansFamily;
    bool useSystemMono = true;
    QString monoFamily;
    float baseFontPx = 16.0f;
    float lineSpacing = 1.42f;
    float maxProseWidth = 880.0f;
    uint8_t headingMarks = KMARK_MARKS_SLANTED;

    /* Colors */
    bool useSystemColors = true;
    AppearanceColors customColors;

    /* Saved user presets */
    QList<AppearancePreset> userPresets;

    /* Editor & View toggles */
    uint8_t defaultViewMode = KMARK_VIEW_WYSIWYG;
    bool wordWrap = true;
    bool showLineNumbers = false;
    bool focusMode = false;
    bool typewriterMode = false;

    /* Workspace & Window state */
    QString lastFolderRoot;
    QByteArray windowGeometry;
    QByteArray windowState;

    static AppSettings load();
    void save() const;

    QList<AppearancePreset> allPresets() const;
    const AppearancePreset* findPresetById(const QString& id) const;
    bool matchesAppearanceOf(const AppearancePreset& preset) const;
    void applyPreset(const AppearancePreset& preset);
    AppearancePreset captureAsPreset(const QString& id, const QString& name) const;

    QFont effectiveSansFont() const;
    QFont effectiveMonoFont() const;
    KMarkPalette effectivePalette(const QPalette& systemQPalette) const;
    KMarkLayoutSettings toLayoutSettings(uint8_t currentViewMode) const;
};

} // namespace kmark
