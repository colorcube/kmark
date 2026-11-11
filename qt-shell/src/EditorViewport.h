#pragma once

#include <QAbstractScrollArea>
#include <QFont>
#include <QFontMetricsF>
#include <QHash>
#include <QPixmap>
#include <QPoint>
#include <QRectF>
#include <QSize>
#include <QString>
#include <QTimer>

#include "AppSettings.h"
#include "I18n.h"
#include "kmark_ffi.h"

namespace kmark {

class EditorViewport final : public QAbstractScrollArea {
    Q_OBJECT

public:
    explicit EditorViewport(KMarkSession* session, QWidget* parent = nullptr);
    ~EditorViewport() override;

    void applyAppearanceSettings(const AppSettings& settings);
    void refreshSystemFonts();
    void refreshPalette();
    void clearImageCache();
    void notifyDocumentChanged();
    void detachSession() { m_session = nullptr; }

    QString activeDocumentDir() const;
    QString makeFriendlyMarkdownPath(const QString& absPath) const;

    void copySelectionToClipboard();
    void cutSelectionToClipboard();
    void pasteFromClipboard();

signals:
    void stateChanged();
    void findRequested(bool withReplace);
    void fileDropped(const QString& localPath);
    void codeLanguageBadgeClicked(size_t openLineStart, const QString& currentLanguage, const QPoint& globalPos);
    void calloutBadgeClicked(size_t openLineStart, const QString& currentTag, const QPoint& globalPos);

protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void inputMethodEvent(QInputMethodEvent* event) override;
    QVariant inputMethodQuery(Qt::InputMethodQuery query) const override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseDoubleClickEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void contextMenuEvent(QContextMenuEvent* event) override;
    void wheelEvent(QWheelEvent* event) override;
    void dragEnterEvent(QDragEnterEvent* event) override;
    void dragMoveEvent(QDragMoveEvent* event) override;
    void dropEvent(QDropEvent* event) override;
    void focusInEvent(QFocusEvent* event) override;
    void focusOutEvent(QFocusEvent* event) override;
    bool event(QEvent* event) override;

private:
    struct CachedFontEntry {
        QFont font;
        QFontMetricsF fm;
        qreal spaceWidth = 4.0;

        explicit CachedFontEntry(const QFont& f)
            : font(f)
            , fm(f)
            , spaceWidth(std::max(1.0, fm.horizontalAdvance(QLatin1Char(' ')))) {}
    };

    struct CachedImageEntry {
        qint64 mtimeMsec = -1;
        qint64 fileSize = -1;
        int decodedMaxDim = 0;
        QSize naturalSize;
        QPixmap pixmap;
    };

    static void metricsCallback(
        void* userData,
        uint8_t fontRole,
        float sizePx,
        bool bold,
        bool italic,
        KMarkFontMetrics* outMetrics
    );

    static float widthCallback(
        void* userData,
        const char* utf8Ptr,
        size_t utf8Len,
        uint8_t fontRole,
        float sizePx,
        bool bold,
        bool italic
    );

    static void runLayoutCallback(
        void* userData,
        const char* utf8Ptr,
        size_t utf8Len,
        const uint32_t* utf16EndsPtr,
        size_t graphemeCount,
        uint8_t fontRole,
        float sizePx,
        bool bold,
        bool italic,
        float* outAdvancesPtr
    );

    static bool imageSizeCallback(
        void* userData,
        const char* srcUtf8,
        size_t srcLen,
        float* outW,
        float* outH
    );

    UiLang currentUiLang() const;
    QFont makeFont(uint8_t fontRole, float sizePx, bool bold, bool italic) const;
    const CachedFontEntry& cachedFont(uint8_t fontRole, float sizePx, bool bold, bool italic) const;
    QString resolveImagePath(const QString& rawSrc) const;
    void resetCaretBlink();

    void applyImageUpdate(
        size_t startByte,
        size_t endByte,
        const QString& alt,
        const QString& src,
        const QString& caption,
        uint8_t sizeKind,
        uint16_t sizeValue
    );
    void promptImageCaption(
        size_t startByte,
        size_t endByte,
        const QString& alt,
        const QString& src,
        const QString& currentCaption,
        uint8_t sizeKind,
        uint16_t sizeValue
    );
    void promptReplaceImage(
        size_t startByte,
        size_t endByte,
        const QString& currentAlt,
        const QString& currentSrc,
        const QString& caption,
        uint8_t sizeKind,
        uint16_t sizeValue
    );

    KMarkSession* m_session = nullptr;
    bool m_useSystemSans = true;
    bool m_useSystemMono = true;
    bool m_useSystemColors = true;
    QFont m_sansBase;
    QFont m_monoBase;
    KMarkPalette m_customPalette{};
    QRectF m_lastCaretRect{28.0, 20.0, 2.0, 22.0};
    float m_lastFontPx = 16.0f;
    QTimer m_caretTimer;
    bool m_caretVisible = true;
    mutable QHash<uint32_t, CachedFontEntry> m_fontCache;
    mutable QHash<QString, CachedImageEntry> m_imageCache;
};

} // namespace kmark
