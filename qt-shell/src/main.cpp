#include <QApplication>
#include <QCommandLineOption>
#include <QCommandLineParser>
#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocalServer>
#include <QLocalSocket>
#include <QPixmap>
#include <QStandardPaths>
#include <QTimer>
#include <memory>

#ifdef Q_OS_UNIX
#include <unistd.h>
#endif

#ifdef KMARK_HAVE_KF6
#include <KAboutData>
#include <KLocalizedString>
#endif

#include "AppSettings.h"
#include "I18n.h"
#include "MainWindow.h"
#include "SettingsDialog.h"

namespace {

constexpr qsizetype kMaxIpcPayloadBytes = 1024 * 1024;

QString singleInstanceSocketPath() {
    QString runtimeDir = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
    if (runtimeDir.isEmpty() || runtimeDir.size() > 64) {
#ifdef Q_OS_UNIX
        runtimeDir = QStringLiteral("/tmp/kmark-%1").arg(static_cast<qulonglong>(::getuid()));
#else
        runtimeDir = QDir::tempPath() + QStringLiteral("/kmark-ipc");
#endif
        QDir().mkpath(runtimeDir);
        QFile::setPermissions(
            runtimeDir,
            QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner
        );
    }

    const QByteArray sessionKey =
        qEnvironmentVariable("WAYLAND_DISPLAY").toUtf8()
        + QByteArrayLiteral("|")
        + qEnvironmentVariable("DISPLAY").toUtf8();
    const QString hashHex = QString::fromLatin1(
        QCryptographicHash::hash(sessionKey, QCryptographicHash::Sha256).toHex().left(12)
    );
    return QDir(runtimeDir).filePath(QStringLiteral("de.colorcube.kmark-%1.sock").arg(hashHex));
}

bool forwardToRunningInstance(const QString& socketPath, const QStringList& args) {
    QLocalSocket socket;
    socket.connectToServer(socketPath);
    if (!socket.waitForConnected(400)) {
        return false;
    }

    QJsonArray jsonArgs;
    for (const QString& arg : args) {
        jsonArgs.append(arg);
    }

    QJsonObject payload;
    payload.insert(QStringLiteral("cwd"), QDir::currentPath());
    payload.insert(QStringLiteral("args"), jsonArgs);
    payload.insert(QStringLiteral("token"), qEnvironmentVariable("XDG_ACTIVATION_TOKEN"));

    QByteArray wire = QJsonDocument(payload).toJson(QJsonDocument::Compact);
    wire.append('\n');

    if (socket.write(wire) != wire.size()) {
        return false;
    }
    socket.flush();
    socket.waitForBytesWritten(1000);

    QByteArray reply;
    while (!reply.contains('\n') && socket.waitForReadyRead(1500)) {
        reply.append(socket.readAll());
        if (reply.size() > 4096) {
            break;
        }
    }
    socket.disconnectFromServer();
    return reply.startsWith("OK");
}

bool startSingleInstanceServer(QLocalServer& server, const QString& socketPath, kmark::MainWindow& window) {
    {
        QLocalSocket probe;
        probe.connectToServer(socketPath);
        if (probe.waitForConnected(150)) {
            probe.disconnectFromServer();
            return false;
        }
    }

    QLocalServer::removeServer(socketPath);
    server.setSocketOptions(QLocalServer::UserAccessOption);
    if (!server.listen(socketPath)) {
        return false;
    }

    QObject::connect(&server, &QLocalServer::newConnection, &window, [&server, &window]() {
        while (QLocalSocket* client = server.nextPendingConnection()) {
            auto buffer = std::make_shared<QByteArray>();
            QObject::connect(client, &QLocalSocket::disconnected, client, &QObject::deleteLater);
            QObject::connect(client, &QLocalSocket::readyRead, &window, [client, buffer, &window]() {
                buffer->append(client->readAll());
                if (buffer->size() > kMaxIpcPayloadBytes) {
                    client->disconnectFromServer();
                    return;
                }
                const qsizetype nlPos = buffer->indexOf('\n');
                if (nlPos < 0) {
                    return;
                }

                const QByteArray line = buffer->left(nlPos).trimmed();
                buffer->remove(0, nlPos + 1);

                QJsonParseError err{};
                const QJsonDocument doc = QJsonDocument::fromJson(line, &err);
                if (err.error != QJsonParseError::NoError || !doc.isObject()) {
                    client->write("ERR\n");
                    client->flush();
                    client->disconnectFromServer();
                    return;
                }

                const QJsonObject obj = doc.object();
                const QString cwd = obj.value(QStringLiteral("cwd")).toString();
                const QString token = obj.value(QStringLiteral("token")).toString();
                QStringList forwardedArgs;
                const QJsonArray arr = obj.value(QStringLiteral("args")).toArray();
                forwardedArgs.reserve(arr.size());
                for (const QJsonValue& v : arr) {
                    if (v.isString()) {
                        forwardedArgs.append(v.toString());
                    }
                }

                window.activateFromExternalInstance(forwardedArgs, cwd, token);

                const QByteArray ack =
                    QStringLiteral("OK %1\n").arg(window.tabCount()).toUtf8();
                client->write(ack);
                client->flush();
                client->disconnectFromServer();
            });
        }
    });
    return true;
}

} // namespace

int main(int argc, char* argv[]) {
    QApplication app(argc, argv);
#ifdef KMARK_HAVE_KF6
    KLocalizedString::setApplicationDomain(QByteArrayLiteral("kmark"));
#endif
    QApplication::setApplicationName(QStringLiteral("kmark"));
    QApplication::setApplicationDisplayName(QStringLiteral("KMark"));
    QApplication::setOrganizationName(QStringLiteral("colorcube"));
    QApplication::setOrganizationDomain(QStringLiteral("colorcube.de"));
    QApplication::setApplicationVersion(QStringLiteral("0.1.1"));
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

    QCommandLineOption newWindowOpt(
        {QStringLiteral("n"), QStringLiteral("new-window")},
        kmark::trBi(
            uiLang,
            "Open a new KMark window instead of reusing an existing instance.",
            "Ein neues KMark-Fenster öffnen, statt eine laufende Instanz zu verwenden."
        )
    );
    parser.addOption(newWindowOpt);

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

    const bool isScreenshotMode =
        parser.isSet(screenshotOpt) || parser.isSet(screenshotSettingsOpt);
    const bool forceNewWindow = parser.isSet(newWindowOpt);
    const QStringList args = parser.positionalArguments();
    const QString ipcSocketPath = singleInstanceSocketPath();

    if (!isScreenshotMode && !forceNewWindow && kmark::AppSettings::load().singleInstanceTabs) {
        if (forwardToRunningInstance(ipcSocketPath, args)) {
            return 0;
        }
    }

#ifdef KMARK_HAVE_KF6
    KAboutData aboutData(
        QStringLiteral("kmark"),
        QStringLiteral("KMark"),
        QStringLiteral("0.1.1"),
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
    if (!isScreenshotMode) {
        window.restoreCrashDrafts();
    }
    if (args.isEmpty()) {
        window.openWelcomeGuideIfEmpty();
    } else {
        for (const QString& arg : args) {
            window.openPathFromCli(arg);
        }
    }

    QLocalServer ipcServer;
    if (!isScreenshotMode && !forceNewWindow) {
        startSingleInstanceServer(ipcServer, ipcSocketPath, window);
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
