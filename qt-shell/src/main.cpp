#include <QApplication>
#include <QCommandLineOption>
#include <QCommandLineParser>
#include <QDir>
#include <QFileInfo>
#include <QIcon>
#include <QPixmap>
#include <QTimer>

#ifdef KMARK_HAVE_KF6
#include <KAboutData>
#include <KLocalizedString>
#endif

#include "I18n.h"
#include "MainWindow.h"
#include "SettingsDialog.h"

int main(int argc, char* argv[]) {
    QApplication app(argc, argv);
#ifdef KMARK_HAVE_KF6
    KLocalizedString::setApplicationDomain(QByteArrayLiteral("kmark"));
#endif
    QApplication::setApplicationName(QStringLiteral("kmark"));
    QApplication::setApplicationDisplayName(QStringLiteral("KMark"));
    QApplication::setOrganizationName(QStringLiteral("colorcube"));
    QApplication::setOrganizationDomain(QStringLiteral("colorcube.de"));
    QApplication::setApplicationVersion(QStringLiteral("0.1.0"));
    QApplication::setDesktopFileName(QStringLiteral("de.colorcube.kmark"));

    if (!QIcon::hasThemeIcon(QStringLiteral("format-text-bold"))) {
        QIcon::setThemeName(QStringLiteral("breeze"));
    }
    QApplication::setWindowIcon(
        QIcon::fromTheme(QStringLiteral("de.colorcube.kmark"), QIcon(QStringLiteral(":/icons/de.colorcube.kmark.svg")))
    );

    const kmark::UiLang uiLang = kmark::detectSystemLanguage();

    QCommandLineParser parser;
    parser.setApplicationDescription(
        kmark::trBi(
            uiLang,
            "KMark — Native WYSIWYG Markdown & Text Editor for KDE Plasma",
            "KMark — Nativer WYSIWYG-Markdown- & Texteditor für KDE Plasma"
        )
    );
    parser.addHelpOption();
    parser.addVersionOption();

    QCommandLineOption screenshotOpt(
        QStringLiteral("screenshot"),
        QStringLiteral("Render the main window to <png-path> and exit."),
        QStringLiteral("png-path")
    );
    parser.addOption(screenshotOpt);

    QCommandLineOption screenshotSettingsOpt(
        QStringLiteral("screenshot-settings"),
        QStringLiteral("Render the settings dialog to <png-path> and exit."),
        QStringLiteral("png-path")
    );
    parser.addOption(screenshotSettingsOpt);

    parser.addPositionalArgument(
        QStringLiteral("files"),
        kmark::trBi(uiLang, "Files or folders to open.", "Zu öffnende Dateien oder Ordner."),
        QStringLiteral("[files...]")
    );
    parser.process(app);

#ifdef KMARK_HAVE_KF6
    KAboutData aboutData(
        QStringLiteral("kmark"),
        QStringLiteral("KMark"),
        QStringLiteral("0.1.0"),
        kmark::trBi(
            uiLang,
            "Native WYSIWYG Markdown and plain-text editor for KDE Plasma",
            "Nativer WYSIWYG-Markdown- und Texteditor für KDE Plasma"
        ),
        KAboutLicense::GPL_V3
    );
    KAboutData::setApplicationData(aboutData);
#endif

    kmark::MainWindow window(uiLang);
    const QStringList args = parser.positionalArguments();
    if (args.isEmpty()) {
        window.openWelcomeGuideIfEmpty();
    } else {
        for (const QString& arg : args) {
            window.openPathFromCli(arg);
        }
    }

    if (parser.isSet(screenshotSettingsOpt)) {
        const QString outPath = parser.value(screenshotSettingsOpt);
        kmark::SettingsDialog dlg(kmark::AppSettings::load(), uiLang);
        dlg.show();
        QTimer::singleShot(220, &dlg, [&app, &dlg, outPath]() {
            const QFileInfo fi(outPath);
            QDir().mkpath(fi.absolutePath());
            const QPixmap shot = dlg.grab();
            const bool ok = shot.save(outPath, "PNG");
            app.exit(ok ? 0 : 1);
        });
        return app.exec();
    }

    if (parser.isSet(screenshotOpt)) {
        const QString outPath = parser.value(screenshotOpt);
        window.resize(1380, 1040);
        window.show();
        QTimer::singleShot(220, &window, [&app, &window, outPath]() {
            const QFileInfo fi(outPath);
            QDir().mkpath(fi.absolutePath());
            const QPixmap shot = window.grab();
            const bool ok = shot.save(outPath, "PNG");
            app.exit(ok ? 0 : 1);
        });
        return app.exec();
    }

    window.show();
    return app.exec();
}
