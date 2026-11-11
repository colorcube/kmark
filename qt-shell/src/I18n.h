#pragma once

#include <QLatin1Char>
#include <QLocale>
#include <QString>
#include <QStringList>
#include <cstdint>

#ifdef KMARK_HAVE_KF6
#include <KLocalizedString>
#endif

namespace kmark {

enum class UiLang : uint8_t {
    English = 0,
    German  = 1,
};

inline UiLang detectSystemLanguage() {
    QStringList candidates;
#ifdef KMARK_HAVE_KF6
    candidates.append(KLocalizedString::languages());
#endif
    const QString envLang = qEnvironmentVariable("LANGUAGE");
    if (!envLang.isEmpty()) {
        candidates.append(envLang.split(QLatin1Char(':'), Qt::SkipEmptyParts));
    }
    for (const char* var : {"LC_ALL", "LC_MESSAGES", "LANG"}) {
        const QString val = qEnvironmentVariable(var).trimmed();
        if (!val.isEmpty()) {
            candidates.append(val);
            break;
        }
    }
    candidates.append(QLocale::system().uiLanguages());
    candidates.append(QLocale::system().name());

    for (QString raw : candidates) {
        const qsizetype dot = raw.indexOf(QLatin1Char('.'));
        if (dot >= 0) {
            raw.truncate(dot);
        }
        const qsizetype at = raw.indexOf(QLatin1Char('@'));
        if (at >= 0) {
            raw.truncate(at);
        }
        raw = raw.trimmed().toLower();
        raw.replace(QLatin1Char('-'), QLatin1Char('_'));
        if (raw.isEmpty()) {
            continue;
        }
        if (raw == QStringLiteral("c")
            || raw == QStringLiteral("posix")
            || raw == QStringLiteral("en")
            || raw.startsWith(QStringLiteral("en_"))) {
            return UiLang::English;
        }
        if (raw == QStringLiteral("de") || raw.startsWith(QStringLiteral("de_"))) {
            return UiLang::German;
        }
    }
    return UiLang::English;
}

inline QString trBi(UiLang lang, const char* enUtf8, const char* deUtf8) {
    return QString::fromUtf8(lang == UiLang::German ? deUtf8 : enUtf8);
}

} // namespace kmark
