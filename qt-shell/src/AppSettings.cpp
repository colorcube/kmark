#include "AppSettings.h"

#include <QApplication>
#include <QFontDatabase>
#include <QSettings>
#include <algorithm>
#include <cmath>

#include "PaletteBridge.h"

namespace kmark {

namespace {

QColor blendColor(const QColor& a, const QColor& b, double t) {
    const double clamped = std::clamp(t, 0.0, 1.0);
    const QColor ra = a.toRgb();
    const QColor rb = b.toRgb();
    auto mix = [clamped](int va, int vb) {
        return std::clamp(static_cast<int>(std::lround(va + (vb - va) * clamped)), 0, 255);
    };
    return QColor(
        mix(ra.red(), rb.red()),
        mix(ra.green(), rb.green()),
        mix(ra.blue(), rb.blue()),
        255
    );
}

QString pickInstalledFamily(
    std::initializer_list<const char*> candidates,
    QFontDatabase::SystemFont fallbackRole
) {
    const QStringList installed = QFontDatabase::families();
    for (const char* name : candidates) {
        const QString target = QString::fromLatin1(name);
        for (const QString& fam : installed) {
            if (fam.compare(target, Qt::CaseInsensitive) == 0) {
                return fam;
            }
        }
    }
    return QFontDatabase::systemFont(fallbackRole).family();
}

QString colorToHexArgb(const QColor& c) {
    return c.name(c.alpha() < 255 ? QColor::HexArgb : QColor::HexRgb);
}

QColor colorFromHexArgb(const QString& s, const QColor& fallback) {
    const QColor parsed = QColor::fromString(s.trimmed());
    return parsed.isValid() ? parsed : fallback;
}

} // namespace

KMarkPalette AppearanceColors::toFfiPalette() const {
    const bool isDark = canvasBg.lightness() < 128;
    KMarkPalette pal = isDark ? kmark_palette_breeze_dark() : kmark_palette_breeze_light();

    pal.dark = isDark;
    pal.canvas_bg = toFfiColor(canvasBg);
    pal.text_fg = toFfiColor(textFg);
    pal.caret = pal.text_fg;

    pal.accent = toFfiColor(accent);
    pal.link_fg = toFfiColor(linkFg);

    QColor selBg = accent;
    selBg.setAlpha(isDark ? 94 : 76);
    pal.selection_bg = toFfiColor(selBg);
    pal.selection_fg = pal.text_fg;

    pal.gutter_bg = toFfiColor(blendColor(canvasBg, textFg, isDark ? 0.045 : 0.03));
    pal.gutter_border = toFfiColor(blendColor(canvasBg, textFg, isDark ? 0.14 : 0.11));
    pal.muted_fg = toFfiColor(blendColor(canvasBg, textFg, 0.54));
    pal.gutter_fg = pal.muted_fg;

    pal.code_bg = toFfiColor(codeBg);
    pal.inline_code_bg = toFfiColor(blendColor(codeBg, textFg, isDark ? 0.06 : 0.045));

    pal.quote_bar = toFfiColor(quoteBar);
    pal.quote_fg = toFfiColor(blendColor(canvasBg, textFg, 0.74));
    pal.rule_line = toFfiColor(blendColor(canvasBg, textFg, isDark ? 0.18 : 0.14));

    QColor hl = highlightBg;
    if (hl.alpha() == 255) {
        hl.setAlpha(isDark ? 125 : 175);
    }
    pal.highlight_bg = toFfiColor(hl);

    pal.table_header_bg = toFfiColor(tableHeaderBg);
    pal.table_border = toFfiColor(blendColor(canvasBg, textFg, isDark ? 0.24 : 0.20));

    return pal;
}

AppearanceColors AppearanceColors::fromQtPalette(const QPalette& qpal) {
    const KMarkPalette p = paletteFromQt(qpal);
    AppearanceColors out;
    out.canvasBg = toQColor(p.canvas_bg);
    out.textFg = toQColor(p.text_fg);
    out.accent = toQColor(p.accent);
    out.linkFg = toQColor(p.link_fg);
    out.codeBg = toQColor(p.code_bg);
    out.quoteBar = toQColor(p.quote_bar);
    out.highlightBg = toQColor(p.highlight_bg);
    out.tableHeaderBg = toQColor(p.table_header_bg);
    return out;
}

const QList<AppearancePreset>& builtinPresets() {
    static const QList<AppearancePreset> list = []() {
        const QString modernSans = pickInstalledFamily(
            {"Inter", "Noto Sans", "Cantarell", "IBM Plex Sans", "DejaVu Sans"},
            QFontDatabase::GeneralFont
        );
        const QString bookSerif = pickInstalledFamily(
            {"Noto Serif", "Liberation Serif", "Merriweather", "Georgia", "DejaVu Serif"},
            QFontDatabase::GeneralFont
        );
        const QString calmSans = pickInstalledFamily(
            {"Noto Sans", "Fira Sans", "Cantarell", "Ubuntu", "DejaVu Sans"},
            QFontDatabase::GeneralFont
        );
        const QString modernMono = pickInstalledFamily(
            {"JetBrains Mono", "Fira Code", "Hack", "IBM Plex Mono", "Noto Sans Mono", "DejaVu Sans Mono"},
            QFontDatabase::FixedFont
        );

        QList<AppearancePreset> presets;

        /* 1. System (KDE Breeze) */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:system");
            p.nameEn = QStringLiteral("System (KDE Breeze)");
            p.nameDe = QStringLiteral("System (KDE Breeze)");
            p.descEn = QStringLiteral("Follows active KDE Plasma fonts and Light/Dark system colors.");
            p.descDe = QStringLiteral("Folgt automatisch den aktiven KDE-Plasma-Schriften und Systemfarben.");
            p.isBuiltin = true;
            p.useSystemSans = true;
            p.useSystemMono = true;
            p.baseFontPx = 16.0f;
            p.lineSpacing = 1.42f;
            p.maxProseWidth = 880.0f;
            p.headingMarks = KMARK_MARKS_SLANTED;
            p.useSystemColors = true;
            p.colors = AppearanceColors::fromQtPalette(QApplication::palette());
            presets.append(p);
        }

        /* 2. Modern */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:modern");
            p.nameEn = QStringLiteral("Modern");
            p.nameDe = QStringLiteral("Modern");
            p.descEn = QStringLiteral("Clean sans typography, crisp slate-white paper, cobalt accents, slanted heading bars.");
            p.descDe = QStringLiteral("Klare Sans-Typografie, kühles Weiß, Kobaltblau-Akzent und schräge Überschriften-Balken.");
            p.isBuiltin = true;
            p.useSystemSans = false;
            p.sansFamily = modernSans;
            p.useSystemMono = false;
            p.monoFamily = modernMono;
            p.baseFontPx = 16.0f;
            p.lineSpacing = 1.45f;
            p.maxProseWidth = 880.0f;
            p.headingMarks = KMARK_MARKS_SLANTED;
            p.useSystemColors = false;
            p.colors.canvasBg = QColor(0xF8, 0xFA, 0xFC);
            p.colors.textFg = QColor(0x0F, 0x17, 0x2A);
            p.colors.accent = QColor(0x25, 0x63, 0xEB);
            p.colors.linkFg = QColor(0x1D, 0x4E, 0xD8);
            p.colors.codeBg = QColor(0xF1, 0xF5, 0xF9);
            p.colors.quoteBar = QColor(0x94, 0xA3, 0xB8);
            p.colors.highlightBg = QColor(0xFE, 0xF0, 0x8A, 190);
            p.colors.tableHeaderBg = QColor(0xE2, 0xE8, 0xF0);
            presets.append(p);
        }

        /* 3. Klassisch / Classic */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:classic");
            p.nameEn = QStringLiteral("Classic");
            p.nameDe = QStringLiteral("Klassisch");
            p.descEn = QStringLiteral("Book-like serif typography on warm ivory paper without margin heading bars.");
            p.descDe = QStringLiteral("Klassische Buch-Serifenschrift auf warmem Elfenbeinpapier ohne Randmarkierungen.");
            p.isBuiltin = true;
            p.useSystemSans = false;
            p.sansFamily = bookSerif;
            p.useSystemMono = false;
            p.monoFamily = modernMono;
            p.baseFontPx = 17.0f;
            p.lineSpacing = 1.54f;
            p.maxProseWidth = 800.0f;
            p.headingMarks = KMARK_MARKS_NONE;
            p.useSystemColors = false;
            p.colors.canvasBg = QColor(0xFA, 0xF6, 0xEE);
            p.colors.textFg = QColor(0x2B, 0x25, 0x20);
            p.colors.accent = QColor(0x9A, 0x34, 0x12);
            p.colors.linkFg = QColor(0x7C, 0x2D, 0x12);
            p.colors.codeBg = QColor(0xF1, 0xEA, 0xDF);
            p.colors.quoteBar = QColor(0xC7, 0xB8, 0xA5);
            p.colors.highlightBg = QColor(0xFD, 0xE6, 0x8A, 180);
            p.colors.tableHeaderBg = QColor(0xEF, 0xE7, 0xDA);
            presets.append(p);
        }

        /* 4. Ruhig / Calm */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:calm");
            p.nameEn = QStringLiteral("Calm");
            p.nameDe = QStringLiteral("Ruhig");
            p.descEn = QStringLiteral("Soft sage-tinted surface, gentle low-glare contrast, generous line spacing.");
            p.descDe = QStringLiteral("Augenschonender Salbei-Papierfarbton, sanfter Kontrast und großzügiger Zeilenabstand.");
            p.isBuiltin = true;
            p.useSystemSans = false;
            p.sansFamily = calmSans;
            p.useSystemMono = false;
            p.monoFamily = modernMono;
            p.baseFontPx = 17.0f;
            p.lineSpacing = 1.58f;
            p.maxProseWidth = 820.0f;
            p.headingMarks = KMARK_MARKS_SLANTED;
            p.useSystemColors = false;
            p.colors.canvasBg = QColor(0xF1, 0xF4, 0xEF);
            p.colors.textFg = QColor(0x2D, 0x36, 0x33);
            p.colors.accent = QColor(0x3B, 0x7A, 0x6B);
            p.colors.linkFg = QColor(0x28, 0x66, 0x5B);
            p.colors.codeBg = QColor(0xE6, 0xEB, 0xE3);
            p.colors.quoteBar = QColor(0xA3, 0xB5, 0xAD);
            p.colors.highlightBg = QColor(0xEE, 0xE8, 0xA9, 185);
            p.colors.tableHeaderBg = QColor(0xE1, 0xE7, 0xDE);
            presets.append(p);
        }

        /* 5. Dunkel (Nacht) / Dark (Night) */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:dark");
            p.nameEn = QStringLiteral("Dark (Night Studio)");
            p.nameDe = QStringLiteral("Dunkel (Nachtmodus)");
            p.descEn = QStringLiteral("Deep slate night canvas with soft snow ink and frost-blue accents.");
            p.descDe = QStringLiteral("Dunkler Schiefer-Hintergrund mit sanftem Hellgrau und eisblauen Akzenten.");
            p.isBuiltin = true;
            p.useSystemSans = false;
            p.sansFamily = modernSans;
            p.useSystemMono = false;
            p.monoFamily = modernMono;
            p.baseFontPx = 16.0f;
            p.lineSpacing = 1.46f;
            p.maxProseWidth = 880.0f;
            p.headingMarks = KMARK_MARKS_SLANTED;
            p.useSystemColors = false;
            p.colors.canvasBg = QColor(0x1B, 0x1F, 0x27);
            p.colors.textFg = QColor(0xE5, 0xE9, 0xF0);
            p.colors.accent = QColor(0x63, 0xB3, 0xED);
            p.colors.linkFg = QColor(0x88, 0xC0, 0xD0);
            p.colors.codeBg = QColor(0x24, 0x29, 0x33);
            p.colors.quoteBar = QColor(0x4C, 0x56, 0x6A);
            p.colors.highlightBg = QColor(0xB7, 0x89, 0x17, 115);
            p.colors.tableHeaderBg = QColor(0x26, 0x2C, 0x36);
            presets.append(p);
        }

        /* 6. Technisch / Technical */
        {
            AppearancePreset p;
            p.id = QStringLiteral("builtin:technical");
            p.nameEn = QStringLiteral("Technical");
            p.nameDe = QStringLiteral("Technisch");
            p.descEn = QStringLiteral("Compact spacing, wider column, crisp cyan accents, and '#' heading markers.");
            p.descDe = QStringLiteral("Kompakte Dichte, breitere Textspalte, Cyan-Akzente und Raute-Symbol (#) vor Überschriften.");
            p.isBuiltin = true;
            p.useSystemSans = false;
            p.sansFamily = modernSans;
            p.useSystemMono = false;
            p.monoFamily = modernMono;
            p.baseFontPx = 15.0f;
            p.lineSpacing = 1.38f;
            p.maxProseWidth = 980.0f;
            p.headingMarks = KMARK_MARKS_HASH;
            p.useSystemColors = false;
            p.colors.canvasBg = QColor(0xFF, 0xFF, 0xFF);
            p.colors.textFg = QColor(0x1E, 0x29, 0x3B);
            p.colors.accent = QColor(0x02, 0x84, 0xC7);
            p.colors.linkFg = QColor(0x03, 0x69, 0xA1);
            p.colors.codeBg = QColor(0xF4, 0xF6, 0xF8);
            p.colors.quoteBar = QColor(0x94, 0xA3, 0xB8);
            p.colors.highlightBg = QColor(0xFE, 0xF0, 0x8A, 185);
            p.colors.tableHeaderBg = QColor(0xED, 0xF2, 0xF7);
            presets.append(p);
        }

        return presets;
    }();
    return list;
}

AppSettings AppSettings::load() {
    AppSettings s;
    s.customColors = AppearanceColors::fromQtPalette(QApplication::palette());

    QSettings qs(QStringLiteral("colorcube"), QStringLiteral("kmark"));

    qs.beginGroup(QStringLiteral("Appearance"));
    s.activePresetId = qs.value(QStringLiteral("activePresetId"), s.activePresetId).toString();
    s.useSystemSans = qs.value(QStringLiteral("useSystemSans"), s.useSystemSans).toBool();
    s.sansFamily = qs.value(QStringLiteral("sansFamily"), s.sansFamily).toString();
    s.useSystemMono = qs.value(QStringLiteral("useSystemMono"), s.useSystemMono).toBool();
    s.monoFamily = qs.value(QStringLiteral("monoFamily"), s.monoFamily).toString();
    s.baseFontPx = std::clamp(
        qs.value(QStringLiteral("baseFontPx"), static_cast<double>(s.baseFontPx)).toFloat(),
        10.0f,
        40.0f
    );
    s.lineSpacing = std::clamp(
        qs.value(QStringLiteral("lineSpacing"), static_cast<double>(s.lineSpacing)).toFloat(),
        1.10f,
        2.20f
    );
    s.maxProseWidth = std::clamp(
        qs.value(QStringLiteral("maxProseWidth"), static_cast<double>(s.maxProseWidth)).toFloat(),
        520.0f,
        2400.0f
    );
    s.headingMarks = static_cast<uint8_t>(
        std::clamp(qs.value(QStringLiteral("headingMarks"), static_cast<int>(s.headingMarks)).toInt(), 0, 2)
    );
    s.useSystemColors = qs.value(QStringLiteral("useSystemColors"), s.useSystemColors).toBool();
    s.customColors.canvasBg = colorFromHexArgb(qs.value(QStringLiteral("canvasBg")).toString(), s.customColors.canvasBg);
    s.customColors.textFg = colorFromHexArgb(qs.value(QStringLiteral("textFg")).toString(), s.customColors.textFg);
    s.customColors.accent = colorFromHexArgb(qs.value(QStringLiteral("accent")).toString(), s.customColors.accent);
    s.customColors.linkFg = colorFromHexArgb(qs.value(QStringLiteral("linkFg")).toString(), s.customColors.linkFg);
    s.customColors.codeBg = colorFromHexArgb(qs.value(QStringLiteral("codeBg")).toString(), s.customColors.codeBg);
    s.customColors.quoteBar = colorFromHexArgb(qs.value(QStringLiteral("quoteBar")).toString(), s.customColors.quoteBar);
    s.customColors.highlightBg = colorFromHexArgb(qs.value(QStringLiteral("highlightBg")).toString(), s.customColors.highlightBg);
    s.customColors.tableHeaderBg = colorFromHexArgb(qs.value(QStringLiteral("tableHeaderBg")).toString(), s.customColors.tableHeaderBg);
    qs.endGroup();

    qs.beginGroup(QStringLiteral("Editor"));
    s.defaultViewMode = static_cast<uint8_t>(
        std::clamp(qs.value(QStringLiteral("defaultViewMode"), static_cast<int>(s.defaultViewMode)).toInt(), 0, 3)
    );
    s.wordWrap = qs.value(QStringLiteral("wordWrap"), s.wordWrap).toBool();
    s.showLineNumbers = qs.value(QStringLiteral("showLineNumbers"), s.showLineNumbers).toBool();
    s.focusMode = qs.value(QStringLiteral("focusMode"), s.focusMode).toBool();
    s.typewriterMode = qs.value(QStringLiteral("typewriterMode"), s.typewriterMode).toBool();
    qs.endGroup();

    qs.beginGroup(QStringLiteral("Workspace"));
    s.lastFolderRoot = qs.value(QStringLiteral("lastFolderRoot")).toString();
    s.windowGeometry = qs.value(QStringLiteral("windowGeometry")).toByteArray();
    s.windowState = qs.value(QStringLiteral("windowState")).toByteArray();
    qs.endGroup();

    const int presetCount = qs.beginReadArray(QStringLiteral("UserPresets"));
    for (int i = 0; i < presetCount; ++i) {
        qs.setArrayIndex(i);
        AppearancePreset up;
        up.id = qs.value(QStringLiteral("id")).toString();
        const QString name = qs.value(QStringLiteral("name")).toString().trimmed();
        if (up.id.isEmpty() || name.isEmpty()) {
            continue;
        }
        up.nameEn = name;
        up.nameDe = name;
        up.descEn = QStringLiteral("Custom user preset");
        up.descDe = QStringLiteral("Eigenes gespeichertes Preset");
        up.isBuiltin = false;
        up.useSystemSans = qs.value(QStringLiteral("useSystemSans"), false).toBool();
        up.sansFamily = qs.value(QStringLiteral("sansFamily")).toString();
        up.useSystemMono = qs.value(QStringLiteral("useSystemMono"), false).toBool();
        up.monoFamily = qs.value(QStringLiteral("monoFamily")).toString();
        up.baseFontPx = std::clamp(qs.value(QStringLiteral("baseFontPx"), 16.0).toFloat(), 10.0f, 40.0f);
        up.lineSpacing = std::clamp(qs.value(QStringLiteral("lineSpacing"), 1.42).toFloat(), 1.10f, 2.20f);
        up.maxProseWidth = std::clamp(qs.value(QStringLiteral("maxProseWidth"), 880.0).toFloat(), 520.0f, 2400.0f);
        up.headingMarks = static_cast<uint8_t>(
            std::clamp(qs.value(QStringLiteral("headingMarks"), 0).toInt(), 0, 2)
        );
        up.useSystemColors = qs.value(QStringLiteral("useSystemColors"), false).toBool();
        up.colors.canvasBg = colorFromHexArgb(qs.value(QStringLiteral("canvasBg")).toString(), up.colors.canvasBg);
        up.colors.textFg = colorFromHexArgb(qs.value(QStringLiteral("textFg")).toString(), up.colors.textFg);
        up.colors.accent = colorFromHexArgb(qs.value(QStringLiteral("accent")).toString(), up.colors.accent);
        up.colors.linkFg = colorFromHexArgb(qs.value(QStringLiteral("linkFg")).toString(), up.colors.linkFg);
        up.colors.codeBg = colorFromHexArgb(qs.value(QStringLiteral("codeBg")).toString(), up.colors.codeBg);
        up.colors.quoteBar = colorFromHexArgb(qs.value(QStringLiteral("quoteBar")).toString(), up.colors.quoteBar);
        up.colors.highlightBg = colorFromHexArgb(qs.value(QStringLiteral("highlightBg")).toString(), up.colors.highlightBg);
        up.colors.tableHeaderBg = colorFromHexArgb(qs.value(QStringLiteral("tableHeaderBg")).toString(), up.colors.tableHeaderBg);
        s.userPresets.append(up);
    }
    qs.endArray();

    if (!s.activePresetId.isEmpty() && !qs.contains(QStringLiteral("Appearance/canvasBg"))) {
        if (const AppearancePreset* preset = s.findPresetById(s.activePresetId)) {
            s.applyPreset(*preset);
        }
    }

    return s;
}

void AppSettings::save() const {
    QSettings qs(QStringLiteral("colorcube"), QStringLiteral("kmark"));

    qs.beginGroup(QStringLiteral("Appearance"));
    qs.setValue(QStringLiteral("activePresetId"), activePresetId);
    qs.setValue(QStringLiteral("useSystemSans"), useSystemSans);
    qs.setValue(QStringLiteral("sansFamily"), sansFamily);
    qs.setValue(QStringLiteral("useSystemMono"), useSystemMono);
    qs.setValue(QStringLiteral("monoFamily"), monoFamily);
    qs.setValue(QStringLiteral("baseFontPx"), static_cast<double>(baseFontPx));
    qs.setValue(QStringLiteral("lineSpacing"), static_cast<double>(lineSpacing));
    qs.setValue(QStringLiteral("maxProseWidth"), static_cast<double>(maxProseWidth));
    qs.setValue(QStringLiteral("headingMarks"), static_cast<int>(headingMarks));
    qs.setValue(QStringLiteral("useSystemColors"), useSystemColors);
    qs.setValue(QStringLiteral("canvasBg"), colorToHexArgb(customColors.canvasBg));
    qs.setValue(QStringLiteral("textFg"), colorToHexArgb(customColors.textFg));
    qs.setValue(QStringLiteral("accent"), colorToHexArgb(customColors.accent));
    qs.setValue(QStringLiteral("linkFg"), colorToHexArgb(customColors.linkFg));
    qs.setValue(QStringLiteral("codeBg"), colorToHexArgb(customColors.codeBg));
    qs.setValue(QStringLiteral("quoteBar"), colorToHexArgb(customColors.quoteBar));
    qs.setValue(QStringLiteral("highlightBg"), colorToHexArgb(customColors.highlightBg));
    qs.setValue(QStringLiteral("tableHeaderBg"), colorToHexArgb(customColors.tableHeaderBg));
    qs.endGroup();

    qs.beginGroup(QStringLiteral("Editor"));
    qs.setValue(QStringLiteral("defaultViewMode"), static_cast<int>(defaultViewMode));
    qs.setValue(QStringLiteral("wordWrap"), wordWrap);
    qs.setValue(QStringLiteral("showLineNumbers"), showLineNumbers);
    qs.setValue(QStringLiteral("focusMode"), focusMode);
    qs.setValue(QStringLiteral("typewriterMode"), typewriterMode);
    qs.endGroup();

    qs.beginGroup(QStringLiteral("Workspace"));
    qs.setValue(QStringLiteral("lastFolderRoot"), lastFolderRoot);
    qs.setValue(QStringLiteral("windowGeometry"), windowGeometry);
    qs.setValue(QStringLiteral("windowState"), windowState);
    qs.endGroup();

    qs.remove(QStringLiteral("UserPresets"));
    qs.beginWriteArray(QStringLiteral("UserPresets"), static_cast<int>(userPresets.size()));
    for (int i = 0; i < userPresets.size(); ++i) {
        qs.setArrayIndex(i);
        const AppearancePreset& up = userPresets.at(i);
        qs.setValue(QStringLiteral("id"), up.id);
        qs.setValue(QStringLiteral("name"), up.nameDe.isEmpty() ? up.nameEn : up.nameDe);
        qs.setValue(QStringLiteral("useSystemSans"), up.useSystemSans);
        qs.setValue(QStringLiteral("sansFamily"), up.sansFamily);
        qs.setValue(QStringLiteral("useSystemMono"), up.useSystemMono);
        qs.setValue(QStringLiteral("monoFamily"), up.monoFamily);
        qs.setValue(QStringLiteral("baseFontPx"), static_cast<double>(up.baseFontPx));
        qs.setValue(QStringLiteral("lineSpacing"), static_cast<double>(up.lineSpacing));
        qs.setValue(QStringLiteral("maxProseWidth"), static_cast<double>(up.maxProseWidth));
        qs.setValue(QStringLiteral("headingMarks"), static_cast<int>(up.headingMarks));
        qs.setValue(QStringLiteral("useSystemColors"), up.useSystemColors);
        qs.setValue(QStringLiteral("canvasBg"), colorToHexArgb(up.colors.canvasBg));
        qs.setValue(QStringLiteral("textFg"), colorToHexArgb(up.colors.textFg));
        qs.setValue(QStringLiteral("accent"), colorToHexArgb(up.colors.accent));
        qs.setValue(QStringLiteral("linkFg"), colorToHexArgb(up.colors.linkFg));
        qs.setValue(QStringLiteral("codeBg"), colorToHexArgb(up.colors.codeBg));
        qs.setValue(QStringLiteral("quoteBar"), colorToHexArgb(up.colors.quoteBar));
        qs.setValue(QStringLiteral("highlightBg"), colorToHexArgb(up.colors.highlightBg));
        qs.setValue(QStringLiteral("tableHeaderBg"), colorToHexArgb(up.colors.tableHeaderBg));
    }
    qs.endArray();
    qs.sync();
}

QList<AppearancePreset> AppSettings::allPresets() const {
    QList<AppearancePreset> out = builtinPresets();
    out.append(userPresets);
    return out;
}

const AppearancePreset* AppSettings::findPresetById(const QString& id) const {
    for (const AppearancePreset& p : builtinPresets()) {
        if (p.id == id) {
            return &p;
        }
    }
    for (const AppearancePreset& p : userPresets) {
        if (p.id == id) {
            return &p;
        }
    }
    return nullptr;
}

bool AppSettings::matchesAppearanceOf(const AppearancePreset& preset) const {
    if (useSystemSans != preset.useSystemSans) {
        return false;
    }
    if (!useSystemSans && sansFamily.compare(preset.sansFamily, Qt::CaseInsensitive) != 0) {
        return false;
    }
    if (useSystemMono != preset.useSystemMono) {
        return false;
    }
    if (!useSystemMono && monoFamily.compare(preset.monoFamily, Qt::CaseInsensitive) != 0) {
        return false;
    }
    if (std::abs(baseFontPx - preset.baseFontPx) > 0.2f) {
        return false;
    }
    if (std::abs(lineSpacing - preset.lineSpacing) > 0.015f) {
        return false;
    }
    if (std::abs(maxProseWidth - preset.maxProseWidth) > 2.0f) {
        return false;
    }
    if (headingMarks != preset.headingMarks) {
        return false;
    }
    if (useSystemColors != preset.useSystemColors) {
        return false;
    }
    if (!useSystemColors && !(customColors == preset.colors)) {
        return false;
    }
    return true;
}

void AppSettings::applyPreset(const AppearancePreset& preset) {
    activePresetId = preset.id;
    useSystemSans = preset.useSystemSans;
    sansFamily = preset.sansFamily;
    useSystemMono = preset.useSystemMono;
    monoFamily = preset.monoFamily;
    baseFontPx = preset.baseFontPx;
    lineSpacing = preset.lineSpacing;
    maxProseWidth = preset.maxProseWidth;
    headingMarks = preset.headingMarks;
    useSystemColors = preset.useSystemColors;
    if (!preset.useSystemColors) {
        customColors = preset.colors;
    } else {
        customColors = AppearanceColors::fromQtPalette(QApplication::palette());
    }
}

AppearancePreset AppSettings::captureAsPreset(const QString& id, const QString& name) const {
    AppearancePreset p;
    p.id = id;
    p.nameEn = name;
    p.nameDe = name;
    p.descEn = QStringLiteral("Custom user preset");
    p.descDe = QStringLiteral("Eigenes gespeichertes Preset");
    p.isBuiltin = false;
    p.useSystemSans = useSystemSans;
    p.sansFamily = sansFamily;
    p.useSystemMono = useSystemMono;
    p.monoFamily = monoFamily;
    p.baseFontPx = baseFontPx;
    p.lineSpacing = lineSpacing;
    p.maxProseWidth = maxProseWidth;
    p.headingMarks = headingMarks;
    p.useSystemColors = useSystemColors;
    p.colors = customColors;
    return p;
}

QFont AppSettings::effectiveSansFont() const {
    QFont sys = QFontDatabase::systemFont(QFontDatabase::GeneralFont);
    if (useSystemSans || sansFamily.trimmed().isEmpty()) {
        return sys;
    }
    QFont custom(sansFamily.trimmed());
    custom.setStyleHint(QFont::SansSerif);
    return custom;
}

QFont AppSettings::effectiveMonoFont() const {
    QFont sys = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    if (useSystemMono || monoFamily.trimmed().isEmpty()) {
        return sys;
    }
    QFont custom(monoFamily.trimmed());
    custom.setStyleHint(QFont::Monospace);
    custom.setFixedPitch(true);
    return custom;
}

KMarkPalette AppSettings::effectivePalette(const QPalette& systemQPalette) const {
    if (useSystemColors) {
        return paletteFromQt(systemQPalette);
    }
    return customColors.toFfiPalette();
}

KMarkLayoutSettings AppSettings::toLayoutSettings(uint8_t currentViewMode) const {
    KMarkLayoutSettings ls{};
    ls.view_mode = currentViewMode;
    ls.heading_marks = headingMarks;
    ls.word_wrap = wordWrap;
    ls.show_line_numbers = showLineNumbers;
    ls.focus_mode = focusMode;
    ls.typewriter_mode = typewriterMode;
    ls.base_font_px = baseFontPx;
    ls.line_spacing = lineSpacing;
    ls.max_prose_width = maxProseWidth;
    return ls;
}

} // namespace kmark
