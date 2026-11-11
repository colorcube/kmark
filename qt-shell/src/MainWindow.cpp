#include "MainWindow.h"
#include "SettingsDialog.h"

#include <QActionGroup>
#include <QApplication>
#include <QCloseEvent>
#include <QCursor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QFontMetrics>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QIcon>
#include <QInputDialog>
#include <QItemSelectionModel>
#include <QKeySequence>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QPainter>
#include <QPen>
#include <QPrintDialog>
#include <QPrintPreviewDialog>
#include <QPrinter>
#include <QPushButton>
#include <QSaveFile>
#include <QSettings>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QStatusBar>
#include <QTextDocument>
#include <QToolButton>
#include <QUuid>
#include <QVBoxLayout>
#include <algorithm>

namespace kmark {

namespace {

constexpr int kMaxRecentFiles = 12;

enum class TableIconKind : uint8_t {
    InsertTable,
    RowAbove,
    RowBelow,
    RowDelete,
    ColLeft,
    ColRight,
    ColDelete,
    AlignLeft,
    AlignCenter,
    AlignRight,
};

QIcon makeTableActionIcon(TableIconKind kind, const QPalette& pal) {
    auto tryTheme = [](std::initializer_list<const char*> names) -> QIcon {
        for (const char* n : names) {
            const QIcon ic = QIcon::fromTheme(QString::fromLatin1(n));
            if (!ic.isNull()) {
                return ic;
            }
        }
        return {};
    };

    QIcon themed;
    switch (kind) {
        case TableIconKind::InsertTable:
            themed = tryTheme({"insert-table", "table"});
            break;
        case TableIconKind::RowAbove:
            themed = tryTheme({"edit-table-insert-row-above"});
            break;
        case TableIconKind::RowBelow:
            themed = tryTheme({"edit-table-insert-row-below"});
            break;
        case TableIconKind::RowDelete:
            themed = tryTheme({"edit-table-delete-row"});
            break;
        case TableIconKind::ColLeft:
            themed = tryTheme({"edit-table-insert-column-left"});
            break;
        case TableIconKind::ColRight:
            themed = tryTheme({"edit-table-insert-column-right"});
            break;
        case TableIconKind::ColDelete:
            themed = tryTheme({"edit-table-delete-column"});
            break;
        case TableIconKind::AlignLeft:
            themed = tryTheme({"format-justify-left"});
            break;
        case TableIconKind::AlignCenter:
            themed = tryTheme({"format-justify-center"});
            break;
        case TableIconKind::AlignRight:
            themed = tryTheme({"format-justify-right"});
            break;
    }
    if (!themed.isNull()) {
        return themed;
    }

    QPixmap pm(24, 24);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);

    QColor fg = pal.color(QPalette::WindowText);
    fg.setAlpha(220);
    const QColor addCol(39, 174, 96);
    const QColor delCol(214, 69, 65);

    if (kind == TableIconKind::AlignLeft
        || kind == TableIconKind::AlignCenter
        || kind == TableIconKind::AlignRight) {
        p.setPen(QPen(fg, 1.9, Qt::SolidLine, Qt::RoundCap));
        const std::array<double, 4> widths = {15.0, 10.0, 14.0, 9.0};
        for (size_t i = 0; i < widths.size(); ++i) {
            const double y = 5.0 + static_cast<double>(i) * 4.5;
            const double w = widths[i];
            double x0 = 4.0;
            if (kind == TableIconKind::AlignCenter) {
                x0 = (24.0 - w) * 0.5;
            } else if (kind == TableIconKind::AlignRight) {
                x0 = 20.0 - w;
            }
            p.drawLine(QPointF(x0, y), QPointF(x0 + w, y));
        }
        return QIcon(pm);
    }

    const QRectF grid(2.5, 4.0, 15.0, 14.0);

    /* Highlight target row or column */
    p.setPen(Qt::NoPen);
    if (kind == TableIconKind::RowAbove) {
        QColor hi = addCol;
        hi.setAlpha(95);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.left(), grid.top(), grid.width(), grid.height() / 3.0));
    } else if (kind == TableIconKind::RowBelow) {
        QColor hi = addCol;
        hi.setAlpha(95);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.left(), grid.bottom() - grid.height() / 3.0, grid.width(), grid.height() / 3.0));
    } else if (kind == TableIconKind::RowDelete) {
        QColor hi = delCol;
        hi.setAlpha(105);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.left(), grid.top() + grid.height() / 3.0, grid.width(), grid.height() / 3.0));
    } else if (kind == TableIconKind::ColLeft) {
        QColor hi = addCol;
        hi.setAlpha(95);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.left(), grid.top(), grid.width() / 3.0, grid.height()));
    } else if (kind == TableIconKind::ColRight) {
        QColor hi = addCol;
        hi.setAlpha(95);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.right() - grid.width() / 3.0, grid.top(), grid.width() / 3.0, grid.height()));
    } else if (kind == TableIconKind::ColDelete) {
        QColor hi = delCol;
        hi.setAlpha(105);
        p.setBrush(hi);
        p.drawRect(QRectF(grid.left() + grid.width() / 3.0, grid.top(), grid.width() / 3.0, grid.height()));
    }

    /* Outer grid + 3x3 cell lines */
    QColor gridCol = fg;
    gridCol.setAlpha(185);
    p.setPen(QPen(gridCol, 1.3));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(grid, 1.5, 1.5);
    for (int i = 1; i <= 2; ++i) {
        const double y = grid.top() + grid.height() * (static_cast<double>(i) / 3.0);
        const double x = grid.left() + grid.width() * (static_cast<double>(i) / 3.0);
        p.drawLine(QPointF(grid.left(), y), QPointF(grid.right(), y));
        p.drawLine(QPointF(x, grid.top()), QPointF(x, grid.bottom()));
    }

    /* Corner badge (+ or x) */
    const QPointF badgeCenter(18.5, 17.5);
    const bool isDel = (kind == TableIconKind::RowDelete || kind == TableIconKind::ColDelete);
    p.setPen(Qt::NoPen);
    p.setBrush(isDel ? delCol : addCol);
    p.drawEllipse(badgeCenter, 4.5, 4.5);

    p.setPen(QPen(Qt::white, 1.6, Qt::SolidLine, Qt::RoundCap));
    if (isDel) {
        p.drawLine(badgeCenter + QPointF(-1.9, -1.9), badgeCenter + QPointF(1.9, 1.9));
        p.drawLine(badgeCenter + QPointF(1.9, -1.9), badgeCenter + QPointF(-1.9, 1.9));
    } else {
        p.drawLine(badgeCenter + QPointF(-2.2, 0.0), badgeCenter + QPointF(2.2, 0.0));
        p.drawLine(badgeCenter + QPointF(0.0, -2.2), badgeCenter + QPointF(0.0, 2.2));
    }

    return QIcon(pm);
}

enum class CoreIconKind : uint8_t {
    NewDoc,
    OpenDoc,
    SaveDoc,
    Undo,
    Redo,
    Bold,
    Italic,
    Strike,
    Highlight,
    InlineCode,
    BulletList,
    OrderedList,
    TaskList,
    Blockquote,
    Link,
    Image,
    CodeFence,
};

QIcon makeCoreToolbarIcon(CoreIconKind kind, const QPalette& pal) {
    auto tryTheme = [](std::initializer_list<const char*> names) -> QIcon {
        for (const char* n : names) {
            const QIcon ic = QIcon::fromTheme(QString::fromLatin1(n));
            if (!ic.isNull()) {
                return ic;
            }
        }
        return {};
    };

    QIcon themed;
    switch (kind) {
        case CoreIconKind::NewDoc:      themed = tryTheme({"document-new"}); break;
        case CoreIconKind::OpenDoc:     themed = tryTheme({"document-open"}); break;
        case CoreIconKind::SaveDoc:     themed = tryTheme({"document-save"}); break;
        case CoreIconKind::Undo:        themed = tryTheme({"edit-undo"}); break;
        case CoreIconKind::Redo:        themed = tryTheme({"edit-redo"}); break;
        case CoreIconKind::Bold:        themed = tryTheme({"format-text-bold"}); break;
        case CoreIconKind::Italic:      themed = tryTheme({"format-text-italic"}); break;
        case CoreIconKind::Strike:      themed = tryTheme({"format-text-strikethrough"}); break;
        case CoreIconKind::Highlight:   themed = tryTheme({"format-text-highlight", "format-text-color"}); break;
        case CoreIconKind::InlineCode:  themed = tryTheme({"format-text-code"}); break;
        case CoreIconKind::BulletList:  themed = tryTheme({"format-list-unordered"}); break;
        case CoreIconKind::OrderedList: themed = tryTheme({"format-list-ordered"}); break;
        case CoreIconKind::TaskList:    themed = tryTheme({"view-task"}); break;
        case CoreIconKind::Blockquote:  themed = tryTheme({"format-text-blockquote"}); break;
        case CoreIconKind::Link:        themed = tryTheme({"insert-link"}); break;
        case CoreIconKind::Image:       themed = tryTheme({"insert-image", "image-x-generic"}); break;
        case CoreIconKind::CodeFence:   themed = tryTheme({"code-context", "text-x-script"}); break;
    }
    if (!themed.isNull()) {
        return themed;
    }

    QPixmap pm(24, 24);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::TextAntialiasing, true);

    QColor fg = pal.color(QPalette::WindowText);
    fg.setAlpha(225);
    const QPen pen(fg, 1.8, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);

    auto drawGlyph = [&](const QString& text, bool bold, bool italic, double px = 15.0) {
        QFont f = QApplication::font();
        f.setPixelSize(static_cast<int>(px));
        f.setBold(bold);
        f.setItalic(italic);
        p.setFont(f);
        p.drawText(QRectF(0, 0, 24, 24), Qt::AlignCenter, text);
    };

    switch (kind) {
        case CoreIconKind::NewDoc: {
            p.drawRoundedRect(QRectF(5.0, 3.5, 13.0, 17.0), 1.8, 1.8);
            p.setPen(QPen(QColor(39, 174, 96), 2.0, Qt::SolidLine, Qt::RoundCap));
            p.drawLine(QPointF(11.5, 9.0), QPointF(11.5, 15.0));
            p.drawLine(QPointF(8.5, 12.0), QPointF(14.5, 12.0));
            break;
        }
        case CoreIconKind::OpenDoc: {
            p.drawRoundedRect(QRectF(3.5, 6.5, 17.0, 13.0), 1.8, 1.8);
            p.drawLine(QPointF(3.5, 9.0), QPointF(20.5, 9.0));
            p.drawLine(QPointF(5.0, 6.5), QPointF(9.5, 4.5));
            p.drawLine(QPointF(9.5, 4.5), QPointF(12.0, 6.5));
            break;
        }
        case CoreIconKind::SaveDoc: {
            p.drawRoundedRect(QRectF(4.0, 4.0, 16.0, 16.0), 1.8, 1.8);
            p.drawRect(QRectF(7.5, 4.0, 9.0, 5.5));
            p.drawRect(QRectF(7.0, 13.0, 10.0, 7.0));
            break;
        }
        case CoreIconKind::Undo: {
            p.drawArc(QRectF(6.0, 7.0, 13.0, 11.0), 0 * 16, 180 * 16);
            p.drawLine(QPointF(6.0, 12.5), QPointF(9.5, 9.0));
            p.drawLine(QPointF(6.0, 12.5), QPointF(9.5, 16.0));
            break;
        }
        case CoreIconKind::Redo: {
            p.drawArc(QRectF(5.0, 7.0, 13.0, 11.0), 0 * 16, 180 * 16);
            p.drawLine(QPointF(18.0, 12.5), QPointF(14.5, 9.0));
            p.drawLine(QPointF(18.0, 12.5), QPointF(14.5, 16.0));
            break;
        }
        case CoreIconKind::Bold:
            drawGlyph(QStringLiteral("B"), true, false, 16.0);
            break;
        case CoreIconKind::Italic:
            drawGlyph(QStringLiteral("I"), false, true, 16.0);
            break;
        case CoreIconKind::Strike:
            drawGlyph(QStringLiteral("S"), true, false, 15.0);
            p.drawLine(QPointF(5.0, 12.5), QPointF(19.0, 12.5));
            break;
        case CoreIconKind::Highlight: {
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(255, 224, 92, 210));
            p.drawRoundedRect(QRectF(4.0, 14.5, 16.0, 5.5), 1.5, 1.5);
            p.setPen(pen);
            drawGlyph(QStringLiteral("A"), true, false, 14.0);
            break;
        }
        case CoreIconKind::InlineCode: {
            p.drawLine(QPointF(9.0, 7.5), QPointF(4.5, 12.0));
            p.drawLine(QPointF(4.5, 12.0), QPointF(9.0, 16.5));
            p.drawLine(QPointF(15.0, 7.5), QPointF(19.5, 12.0));
            p.drawLine(QPointF(19.5, 12.0), QPointF(15.0, 16.5));
            break;
        }
        case CoreIconKind::BulletList: {
            for (int i = 0; i < 3; ++i) {
                const double y = 6.5 + i * 5.5;
                p.setPen(Qt::NoPen);
                p.setBrush(fg);
                p.drawEllipse(QPointF(5.5, y), 1.6, 1.6);
                p.setPen(pen);
                p.drawLine(QPointF(9.5, y), QPointF(19.5, y));
            }
            break;
        }
        case CoreIconKind::OrderedList: {
            QFont nf = QApplication::font();
            nf.setPixelSize(8);
            nf.setBold(true);
            p.setFont(nf);
            for (int i = 0; i < 3; ++i) {
                const double y = 6.5 + i * 5.5;
                p.drawText(QRectF(2.5, y - 4.5, 6.0, 9.0), Qt::AlignCenter, QString::number(i + 1));
                p.drawLine(QPointF(10.0, y), QPointF(19.5, y));
            }
            break;
        }
        case CoreIconKind::TaskList: {
            p.drawRoundedRect(QRectF(4.0, 5.0, 6.0, 6.0), 1.0, 1.0);
            p.drawLine(QPointF(5.3, 8.2), QPointF(6.8, 9.7));
            p.drawLine(QPointF(6.8, 9.7), QPointF(9.2, 6.2));
            p.drawLine(QPointF(12.5, 8.0), QPointF(19.5, 8.0));

            p.drawRoundedRect(QRectF(4.0, 13.5, 6.0, 6.0), 1.0, 1.0);
            p.drawLine(QPointF(12.5, 16.5), QPointF(19.5, 16.5));
            break;
        }
        case CoreIconKind::Blockquote: {
            p.setPen(QPen(pal.color(QPalette::Highlight), 2.8, Qt::SolidLine, Qt::RoundCap));
            p.drawLine(QPointF(5.5, 5.5), QPointF(5.5, 18.5));
            p.setPen(pen);
            p.drawLine(QPointF(10.0, 7.5), QPointF(19.0, 7.5));
            p.drawLine(QPointF(10.0, 12.0), QPointF(18.0, 12.0));
            p.drawLine(QPointF(10.0, 16.5), QPointF(15.5, 16.5));
            break;
        }
        case CoreIconKind::Link: {
            p.drawRoundedRect(QRectF(3.5, 8.5, 8.5, 7.0), 3.2, 3.2);
            p.drawRoundedRect(QRectF(12.0, 8.5, 8.5, 7.0), 3.2, 3.2);
            p.drawLine(QPointF(9.5, 12.0), QPointF(14.5, 12.0));
            break;
        }
        case CoreIconKind::Image: {
            p.drawRoundedRect(QRectF(3.5, 5.0, 17.0, 14.0), 1.8, 1.8);
            p.drawEllipse(QPointF(8.5, 9.5), 1.6, 1.6);
            p.drawLine(QPointF(5.0, 17.5), QPointF(10.5, 12.5));
            p.drawLine(QPointF(10.5, 12.5), QPointF(14.0, 15.5));
            p.drawLine(QPointF(14.0, 15.5), QPointF(16.5, 13.5));
            p.drawLine(QPointF(16.5, 13.5), QPointF(19.5, 17.5));
            break;
        }
        case CoreIconKind::CodeFence: {
            p.drawRoundedRect(QRectF(3.5, 4.5, 17.0, 15.0), 2.2, 2.2);
            drawGlyph(QStringLiteral("{}"), true, false, 11.0);
            break;
        }
    }

    return QIcon(pm);
}

QString takeFfiString(char* ptr) {
    if (ptr == nullptr) {
        return {};
    }
    QString out = QString::fromUtf8(ptr);
    kmark_string_free(ptr);
    return out;
}

QIcon makeFolderUpIcon(const QPalette& pal) {
    for (const char* name : {"go-up", "arrow-up"}) {
        const QIcon themed = QIcon::fromTheme(QString::fromLatin1(name));
        if (!themed.isNull()) {
            return themed;
        }
    }
    QPixmap pm(32, 32);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    QColor fg = pal.color(QPalette::WindowText);
    fg.setAlpha(225);
    QPen pen(fg, 2.4, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.drawLine(QPointF(16.0, 26.0), QPointF(16.0, 7.0));
    p.drawLine(QPointF(8.5, 14.5), QPointF(16.0, 7.0));
    p.drawLine(QPointF(23.5, 14.5), QPointF(16.0, 7.0));
    return QIcon(pm);
}

QIcon makeFolderHomeIcon(const QPalette& pal) {
    for (const char* name : {"user-home", "go-home"}) {
        const QIcon themed = QIcon::fromTheme(QString::fromLatin1(name));
        if (!themed.isNull()) {
            return themed;
        }
    }
    QPixmap pm(32, 32);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    QColor fg = pal.color(QPalette::WindowText);
    fg.setAlpha(225);
    QPen pen(fg, 2.3, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawLine(QPointF(4.5, 16.0), QPointF(16.0, 6.0));
    p.drawLine(QPointF(16.0, 6.0), QPointF(27.5, 16.0));
    p.drawLine(QPointF(8.0, 14.0), QPointF(8.0, 26.0));
    p.drawLine(QPointF(24.0, 14.0), QPointF(24.0, 26.0));
    p.drawLine(QPointF(8.0, 26.0), QPointF(24.0, 26.0));
    p.drawRect(QRectF(13.5, 18.5, 5.0, 7.5));
    return QIcon(pm);
}

QIcon makeSelectOpenedFileIcon(const QPalette& pal) {
    for (const char* name : {"find-location", "crosshairs", "view-InProject", "compass"}) {
        const QIcon themed = QIcon::fromTheme(QString::fromLatin1(name));
        if (!themed.isNull()) {
            return themed;
        }
    }
    /* Crisp vector target/crosshair fallback (PhpStorm "Select Opened File" style) */
    QPixmap pm(32, 32);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    QColor fg = pal.color(QPalette::WindowText);
    fg.setAlpha(225);
    QPen pen(fg, 2.2, Qt::SolidLine, Qt::RoundCap);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawEllipse(QRectF(7.0, 7.0, 18.0, 18.0));
    p.drawLine(QPointF(16.0, 2.5), QPointF(16.0, 8.5));
    p.drawLine(QPointF(16.0, 23.5), QPointF(16.0, 29.5));
    p.drawLine(QPointF(2.5, 16.0), QPointF(8.5, 16.0));
    p.drawLine(QPointF(23.5, 16.0), QPointF(29.5, 16.0));
    p.setPen(Qt::NoPen);
    p.setBrush(fg);
    p.drawEllipse(QPointF(16.0, 16.0), 2.6, 2.6);
    return QIcon(pm);
}

class ElidedPathLabel final : public QLabel {
public:
    explicit ElidedPathLabel(QWidget* parent = nullptr) : QLabel(parent) {
        setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
        setMinimumWidth(40);
    }

    void setFullText(const QString& full) {
        m_full = full;
        refreshElision();
    }

protected:
    void resizeEvent(QResizeEvent* ev) override {
        QLabel::resizeEvent(ev);
        refreshElision();
    }

private:
    void refreshElision() {
        const QFontMetrics fm(font());
        const int avail = std::max(40, width() - 4);
        QLabel::setText(fm.elidedText(m_full, Qt::ElideMiddle, avail));
    }

    QString m_full;
};

QString compactHomePath(const QString& absPath) {
    const QString home = QDir::cleanPath(QDir::homePath());
    const QString clean = QDir::cleanPath(absPath);
    if (!home.isEmpty() && clean == home) {
        return QStringLiteral("~");
    }
    if (!home.isEmpty() && clean.startsWith(home + QLatin1Char('/'))) {
        return QStringLiteral("~") + clean.mid(home.size());
    }
    return clean;
}

const QStringList& commonCodeLanguages() {
    static const QStringList langs = {
        QStringLiteral("rust"),
        QStringLiteral("python"),
        QStringLiteral("bash"),
        QStringLiteral("c"),
        QStringLiteral("cpp"),
        QStringLiteral("javascript"),
        QStringLiteral("typescript"),
        QStringLiteral("go"),
        QStringLiteral("java"),
        QStringLiteral("kotlin"),
        QStringLiteral("json"),
        QStringLiteral("toml"),
        QStringLiteral("yaml"),
        QStringLiteral("html"),
        QStringLiteral("css"),
        QStringLiteral("sql"),
        QStringLiteral("xml"),
        QStringLiteral("cmake"),
        QStringLiteral("markdown"),
        QStringLiteral("diff"),
        QStringLiteral("text"),
    };
    return langs;
}

} // namespace

MainWindow::MainWindow(UiLang initialLang, QWidget* parent)
    : KMarkWindowBase(parent)
    , m_lang(initialLang)
    , m_settings(AppSettings::load())
    , m_session(kmark_session_new()) {
    kmark_session_set_ui_language(m_session, static_cast<uint8_t>(m_lang));
    setAttribute(Qt::WA_DeleteOnClose, false);

    setWindowTitle(QStringLiteral("KMark"));
    const QIcon appIcon = QIcon::fromTheme(
        QStringLiteral("de.colorcube.kmark"),
        QIcon(QStringLiteral(":/icons/de.colorcube.kmark.svg"))
    );
    setWindowIcon(appIcon);
    resize(1340, 860);

    loadRecentFiles();
    setupCentralArea();
    setupDocks();
    setupMenusAndToolbars();
    setupStatusBar();
    retranslateUi();
    applySettingsToRuntime(true);

    if (!m_settings.windowGeometry.isEmpty()) {
        restoreGeometry(m_settings.windowGeometry);
    }
    if (!m_settings.windowState.isEmpty()) {
        restoreState(m_settings.windowState);
    }

    connect(m_viewport, &EditorViewport::stateChanged, this, &MainWindow::syncUiState);
    connect(m_viewport, &EditorViewport::findRequested, this, &MainWindow::showFindBar);
    connect(m_viewport, &EditorViewport::fileDropped, this, &MainWindow::openPathFromCli);
    connect(
        m_viewport,
        &EditorViewport::codeLanguageBadgeClicked,
        this,
        [this](size_t openLineStart, const QString& currentLanguage, const QPoint& globalPos) {
            promptSetCodeFenceLanguage(true, openLineStart, currentLanguage, globalPos);
        }
    );
    connect(
        m_viewport,
        &EditorViewport::calloutBadgeClicked,
        this,
        [this](size_t openLineStart, const QString& currentTag, const QPoint& globalPos) {
            promptSetCalloutKind(true, openLineStart, currentTag, globalPos);
        }
    );

    m_recoveryTimer.setInterval(10000);
    connect(&m_recoveryTimer, &QTimer::timeout, this, [this]() {
        if (m_session != nullptr) {
            kmark_session_flush_recovery(m_session);
        }
    });
    m_recoveryTimer.start();

    syncTabBar();
    syncOutlineTree();
    syncUiState();
    m_viewport->setFocus();
}

MainWindow::~MainWindow() {
    m_recoveryTimer.stop();
    if (m_viewport != nullptr) {
        m_viewport->detachSession();
    }
    if (m_session != nullptr) {
        kmark_session_free(m_session);
        m_session = nullptr;
    }
}

void MainWindow::openPathFromCli(const QString& path) {
    const QFileInfo fi(path);
    if (fi.isDir()) {
        setFolderRoot(fi.absoluteFilePath());
        m_folderDock->show();
        return;
    }
    const QString absFile = fi.absoluteFilePath();
    const QByteArray utf8 = absFile.toUtf8();
    if (!kmark_session_open_file(m_session, utf8.constData())) {
        const QString err = takeFfiString(kmark_session_last_error_alloc(m_session));
        QMessageBox::warning(this, T("Failed to Open File", "Datei öffnen fehlgeschlagen"), err);
        return;
    }
    recordRecentFile(absFile);
    m_viewport->clearImageCache();
    syncTabBar();
    syncOutlineTree();
    m_viewport->notifyDocumentChanged();
}

void MainWindow::openWelcomeGuideIfEmpty() {
    if (kmark_session_tab_count(m_session) == 1
        && !kmark_session_tab_has_path(m_session, 0)
        && !kmark_session_tab_is_dirty(m_session, 0)) {
        kmark_session_open_guide_tab(m_session);
        syncTabBar();
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    }
}

size_t MainWindow::restoreCrashDrafts() {
    if (m_session == nullptr) {
        return 0;
    }
    const size_t recovered = kmark_session_recover_drafts(m_session);
    if (recovered > 0) {
        syncTabBar();
        syncOutlineTree();
        syncUiState();
        m_viewport->notifyDocumentChanged();
        statusBar()->showMessage(
            T(
                "Recovered %1 unsaved document(s) from a previous session.",
                "%1 ungespeicherte(s) Dokument(e) aus einer früheren Sitzung wiederhergestellt."
            )
                .arg(recovered),
            8000
        );
    }
    return recovered;
}

void MainWindow::setupCentralArea() {
    auto* container = new QWidget(this);
    auto* layout = new QVBoxLayout(container);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    m_tabBar = new QTabBar(container);
    m_tabBar->setDocumentMode(true);
    m_tabBar->setTabsClosable(true);
    m_tabBar->setMovable(false);
    m_tabBar->setExpanding(false);

    connect(m_tabBar, &QTabBar::currentChanged, this, [this](int index) {
        if (m_updatingUi || index < 0) {
            return;
        }
        kmark_session_set_active_tab(m_session, static_cast<size_t>(index));
        m_viewport->clearImageCache();
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    });

    connect(m_tabBar, &QTabBar::tabCloseRequested, this, [this](int index) {
        if (index >= 0 && confirmCloseTab(static_cast<size_t>(index))) {
            kmark_session_close_tab(m_session, static_cast<size_t>(index));
            syncTabBar();
            syncOutlineTree();
            m_viewport->notifyDocumentChanged();
        }
    });

    m_viewport = new EditorViewport(m_session, container);

    /* Kate-style bottom inline Find / Replace bar */
    m_findBar = new QWidget(container);
    auto* findOuter = new QVBoxLayout(m_findBar);
    findOuter->setContentsMargins(8, 6, 8, 6);
    findOuter->setSpacing(4);

    auto* findRow = new QHBoxLayout();
    findRow->setSpacing(6);
    m_findLabel = new QLabel(m_findBar);
    findRow->addWidget(m_findLabel);
    m_findInput = new QLineEdit(m_findBar);
    m_findInput->setClearButtonEnabled(true);
    findRow->addWidget(m_findInput, 1);

    m_btnFindPrev = new QPushButton(QIcon::fromTheme(QStringLiteral("go-up-search")), QString(), m_findBar);
    m_btnFindNext = new QPushButton(QIcon::fromTheme(QStringLiteral("go-down-search")), QString(), m_findBar);
    m_caseCheck = new QCheckBox(m_findBar);
    m_wordCheck = new QCheckBox(m_findBar);
    m_regexCheck = new QCheckBox(m_findBar);
    m_matchCountLabel = new QLabel(m_findBar);
    m_btnCloseFind = new QToolButton(m_findBar);
    m_btnCloseFind->setIcon(QIcon::fromTheme(QStringLiteral("dialog-close")));

    findRow->addWidget(m_btnFindPrev);
    findRow->addWidget(m_btnFindNext);
    findRow->addWidget(m_caseCheck);
    findRow->addWidget(m_wordCheck);
    findRow->addWidget(m_regexCheck);
    findRow->addWidget(m_matchCountLabel);
    findRow->addWidget(m_btnCloseFind);
    findOuter->addLayout(findRow);

    m_replaceRow = new QWidget(m_findBar);
    auto* repLayout = new QHBoxLayout(m_replaceRow);
    repLayout->setContentsMargins(0, 0, 0, 0);
    repLayout->setSpacing(6);
    m_replaceLabel = new QLabel(m_replaceRow);
    repLayout->addWidget(m_replaceLabel);
    m_replaceInput = new QLineEdit(m_replaceRow);
    m_btnReplaceOne = new QPushButton(m_replaceRow);
    m_btnReplaceAll = new QPushButton(m_replaceRow);
    repLayout->addWidget(m_replaceInput, 1);
    repLayout->addWidget(m_btnReplaceOne);
    repLayout->addWidget(m_btnReplaceAll);
    findOuter->addWidget(m_replaceRow);

    m_findBar->hide();

    connect(m_findInput, &QLineEdit::textChanged, this, [this]() { updateSearchQuery(); });
    connect(m_findInput, &QLineEdit::returnPressed, this, [this]() {
        kmark_session_find_next(m_session);
        m_viewport->notifyDocumentChanged();
    });
    connect(m_caseCheck, &QCheckBox::toggled, this, [this]() { updateSearchQuery(); });
    connect(m_wordCheck, &QCheckBox::toggled, this, [this]() { updateSearchQuery(); });
    connect(m_regexCheck, &QCheckBox::toggled, this, [this]() { updateSearchQuery(); });
    connect(m_btnFindNext, &QPushButton::clicked, this, [this]() {
        kmark_session_find_next(m_session);
        m_viewport->notifyDocumentChanged();
    });
    connect(m_btnFindPrev, &QPushButton::clicked, this, [this]() {
        kmark_session_find_prev(m_session);
        m_viewport->notifyDocumentChanged();
    });
    connect(m_btnReplaceOne, &QPushButton::clicked, this, [this]() {
        const QByteArray rep = m_replaceInput->text().toUtf8();
        kmark_session_replace_one(m_session, rep.constData());
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    });
    connect(m_btnReplaceAll, &QPushButton::clicked, this, [this]() {
        const QByteArray rep = m_replaceInput->text().toUtf8();
        const size_t n = kmark_session_replace_all(m_session, rep.constData());
        statusBar()->showMessage(
            T("Replaced %1 occurrences.", "%1 Fundstellen ersetzt.").arg(n),
            4000
        );
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    });
    connect(m_btnCloseFind, &QToolButton::clicked, this, &MainWindow::hideFindBar);

    layout->addWidget(m_tabBar);
    layout->addWidget(m_viewport, 1);
    layout->addWidget(m_findBar);
    setCentralWidget(container);
}

void MainWindow::setupDocks() {
    /* Left Dock: Folder Browser with PhpStorm-style top-right "Select Opened File" action */
    m_folderDock = new QDockWidget(this);
    m_folderDock->setObjectName(QStringLiteral("FolderDock"));

    auto* folderContainer = new QWidget(m_folderDock);
    auto* folderLayout = new QVBoxLayout(folderContainer);
    folderLayout->setContentsMargins(0, 0, 0, 0);
    folderLayout->setSpacing(0);

    auto* headerBar = new QWidget(folderContainer);
    auto* headerLayout = new QHBoxLayout(headerBar);
    headerLayout->setContentsMargins(6, 4, 6, 4);
    headerLayout->setSpacing(4);

    m_btnFolderUp = new QToolButton(headerBar);
    m_btnFolderUp->setAutoRaise(true);
    m_btnFolderUp->setIcon(makeFolderUpIcon(palette()));
    connect(m_btnFolderUp, &QToolButton::clicked, this, [this]() {
        QDir dir(m_folderRootPath);
        if (dir.cdUp()) {
            setFolderRoot(dir.absolutePath());
        }
    });

    m_btnFolderHome = new QToolButton(headerBar);
    m_btnFolderHome->setAutoRaise(true);
    m_btnFolderHome->setIcon(makeFolderHomeIcon(palette()));
    connect(m_btnFolderHome, &QToolButton::clicked, this, [this]() {
        setFolderRoot(QDir::homePath());
    });

    m_folderPathLabel = new ElidedPathLabel(headerBar);

    m_btnRevealActiveFile = new QToolButton(headerBar);
    m_btnRevealActiveFile->setAutoRaise(true);
    m_btnRevealActiveFile->setIcon(makeSelectOpenedFileIcon(palette()));
    connect(m_btnRevealActiveFile, &QToolButton::clicked, this, &MainWindow::revealActiveFileInFolderTree);

    headerLayout->addWidget(m_btnFolderUp);
    headerLayout->addWidget(m_btnFolderHome);
    headerLayout->addWidget(m_folderPathLabel, 1);
    headerLayout->addWidget(m_btnRevealActiveFile);

    m_fsModel = new QFileSystemModel(this);
    m_fsModel->setFilter(QDir::AllDirs | QDir::Files | QDir::NoDotAndDotDot);
    m_fsModel->setRootPath(QDir::rootPath());

    connect(m_fsModel, &QFileSystemModel::directoryLoaded, this, [this](const QString&) {
        if (!m_pendingRevealPath.isEmpty()) {
            performPendingRevealStep();
        }
    });

    m_folderView = new QTreeView(folderContainer);
    m_folderView->setModel(m_fsModel);
    m_folderView->setHeaderHidden(true);
    m_folderView->setUniformRowHeights(true);
    m_folderView->setContextMenuPolicy(Qt::CustomContextMenu);
    for (int col = 1; col < m_fsModel->columnCount(); ++col) {
        m_folderView->hideColumn(col);
    }

    QString initialRoot = m_settings.lastFolderRoot;
    if (initialRoot.isEmpty() || !QFileInfo(initialRoot).isDir()) {
        initialRoot = QDir::homePath().isEmpty() ? QDir::rootPath() : QDir::homePath();
    }
    setFolderRoot(initialRoot);

    connect(m_folderView, &QTreeView::activated, this, [this](const QModelIndex& index) {
        if (!index.isValid()) {
            return;
        }
        if (!m_fsModel->isDir(index)) {
            openPathFromCli(m_fsModel->filePath(index));
        }
    });

    connect(m_folderView, &QTreeView::customContextMenuRequested, this, [this](const QPoint& pos) {
        const QModelIndex idx = m_folderView->indexAt(pos);
        QMenu menu(this);
        if (idx.isValid() && m_fsModel->isDir(idx)) {
            const QString dirPath = m_fsModel->filePath(idx);
            QAction* actFocusDir = menu.addAction(
                QIcon::fromTheme(QStringLiteral("folder-open")),
                T("Set as Root Folder", "Als Wurzelordner setzen")
            );
            connect(actFocusDir, &QAction::triggered, this, [this, dirPath]() {
                setFolderRoot(dirPath);
            });
            menu.addSeparator();
        }
        QAction* actReveal = menu.addAction(
            makeSelectOpenedFileIcon(palette()),
            T("Select Opened File", "Aktive Datei aufklappen")
        );
        const size_t active = kmark_session_active_tab(m_session);
        actReveal->setEnabled(kmark_session_tab_has_path(m_session, active));
        connect(actReveal, &QAction::triggered, this, &MainWindow::revealActiveFileInFolderTree);

        QAction* actParent = menu.addAction(
            QIcon::fromTheme(QStringLiteral("go-up")),
            T("Parent Folder", "Übergeordneter Ordner")
        );
        actParent->setEnabled(!QDir(m_folderRootPath).isRoot());
        connect(actParent, &QAction::triggered, this, [this]() {
            QDir dir(m_folderRootPath);
            if (dir.cdUp()) {
                setFolderRoot(dir.absolutePath());
            }
        });
        menu.exec(m_folderView->viewport()->mapToGlobal(pos));
    });

    folderLayout->addWidget(headerBar);
    folderLayout->addWidget(m_folderView, 1);

    m_folderDock->setWidget(folderContainer);
    addDockWidget(Qt::LeftDockWidgetArea, m_folderDock);
    m_folderDock->hide();

    /* Right Dock: Document Outline */
    m_outlineDock = new QDockWidget(this);
    m_outlineDock->setObjectName(QStringLiteral("OutlineDock"));
    m_outlineTree = new QTreeWidget(m_outlineDock);
    m_outlineTree->setHeaderHidden(true);
    m_outlineTree->setIndentation(14);

    connect(m_outlineTree, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem* item, int) {
        if (item != nullptr) {
            const size_t offset = static_cast<size_t>(item->data(0, Qt::UserRole).toULongLong());
            kmark_session_jump_to_offset(m_session, offset);
            m_viewport->notifyDocumentChanged();
            m_viewport->setFocus();
        }
    });

    m_outlineDock->setWidget(m_outlineTree);
    addDockWidget(Qt::RightDockWidgetArea, m_outlineDock);
}

void MainWindow::setupMenusAndToolbars() {
    auto icon = [](const char* name) { return QIcon::fromTheme(QString::fromLatin1(name)); };

    /* ======================== 1. FILE / DATEI ======================== */
    m_fileMenu = menuBar()->addMenu(QString());
    m_actNew = m_fileMenu->addAction(icon("document-new"), QString());
    m_actNew->setShortcut(QKeySequence::New);
    connect(m_actNew, &QAction::triggered, this, &MainWindow::actionNewTab);

    m_actOpen = m_fileMenu->addAction(icon("document-open"), QString());
    m_actOpen->setShortcut(QKeySequence::Open);
    connect(m_actOpen, &QAction::triggered, this, &MainWindow::actionOpenFile);

    m_actOpenFolder = m_fileMenu->addAction(icon("folder-open"), QString());
    connect(m_actOpenFolder, &QAction::triggered, this, &MainWindow::actionOpenFolder);

    m_recentMenu = m_fileMenu->addMenu(icon("document-open-recent"), QString());

    m_fileMenu->addSeparator();

    m_actSave = m_fileMenu->addAction(icon("document-save"), QString());
    m_actSave->setShortcut(QKeySequence::Save);
    connect(m_actSave, &QAction::triggered, this, &MainWindow::actionSave);

    m_actSaveAs = m_fileMenu->addAction(icon("document-save-as"), QString());
    m_actSaveAs->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+S")));
    connect(m_actSaveAs, &QAction::triggered, this, &MainWindow::actionSaveAs);

    m_actExportHtml = m_fileMenu->addAction(icon("text-html"), QString());
    connect(m_actExportHtml, &QAction::triggered, this, &MainWindow::actionExportHtml);

    m_fileMenu->addSeparator();

    m_actPrintPreview = m_fileMenu->addAction(icon("document-print-preview"), QString());
    connect(m_actPrintPreview, &QAction::triggered, this, &MainWindow::actionPrintPreview);

    m_actPrint = m_fileMenu->addAction(icon("document-print"), QString());
    m_actPrint->setShortcut(QKeySequence::Print);
    connect(m_actPrint, &QAction::triggered, this, &MainWindow::actionPrint);

    m_fileMenu->addSeparator();

    m_actCloseTab = m_fileMenu->addAction(icon("tab-close"), QString());
    m_actCloseTab->setShortcut(QKeySequence::Close);
    connect(m_actCloseTab, &QAction::triggered, this, [this]() {
        const size_t active = kmark_session_active_tab(m_session);
        if (confirmCloseTab(active)) {
            kmark_session_close_tab(m_session, active);
            syncTabBar();
            syncOutlineTree();
            m_viewport->notifyDocumentChanged();
        }
    });

    m_actQuit = m_fileMenu->addAction(icon("application-exit"), QString());
    m_actQuit->setShortcut(QKeySequence::Quit);
    connect(m_actQuit, &QAction::triggered, this, &QWidget::close);

    /* ======================== 2. EDIT / BEARBEITEN ======================== */
    m_editMenu = menuBar()->addMenu(QString());
    m_actUndo = m_editMenu->addAction(icon("edit-undo"), QString());
    m_actUndo->setShortcut(QKeySequence::Undo);
    connect(m_actUndo, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_UNDO); });

    m_actRedo = m_editMenu->addAction(icon("edit-redo"), QString());
    m_actRedo->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+Z")));
    connect(m_actRedo, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_REDO); });

    m_editMenu->addSeparator();

    m_actCut = m_editMenu->addAction(icon("edit-cut"), QString());
    m_actCut->setShortcut(QKeySequence::Cut);
    connect(m_actCut, &QAction::triggered, m_viewport, &EditorViewport::cutSelectionToClipboard);

    m_actCopy = m_editMenu->addAction(icon("edit-copy"), QString());
    m_actCopy->setShortcut(QKeySequence::Copy);
    connect(m_actCopy, &QAction::triggered, m_viewport, &EditorViewport::copySelectionToClipboard);

    m_actPaste = m_editMenu->addAction(icon("edit-paste"), QString());
    m_actPaste->setShortcut(QKeySequence::Paste);
    connect(m_actPaste, &QAction::triggered, m_viewport, &EditorViewport::pasteFromClipboard);

    m_actSelectAll = m_editMenu->addAction(icon("edit-select-all"), QString());
    m_actSelectAll->setShortcut(QKeySequence::SelectAll);
    connect(m_actSelectAll, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_SELECT_ALL); });

    m_editMenu->addSeparator();

    m_actFind = m_editMenu->addAction(icon("edit-find"), QString());
    m_actFind->setShortcut(QKeySequence::Find);
    connect(m_actFind, &QAction::triggered, this, [this]() { showFindBar(false); });

    m_actReplace = m_editMenu->addAction(icon("edit-find-replace"), QString());
    m_actReplace->setShortcut(QKeySequence(QStringLiteral("Ctrl+R")));
    connect(m_actReplace, &QAction::triggered, this, [this]() { showFindBar(true); });

    auto* actEscFind = new QAction(this);
    actEscFind->setShortcut(QKeySequence(Qt::Key_Escape));
    actEscFind->setShortcutContext(Qt::WindowShortcut);
    connect(actEscFind, &QAction::triggered, this, [this]() {
        if (m_findBar->isVisible()) {
            hideFindBar();
        }
    });
    addAction(actEscFind);

    /* ======================== 3. VIEW / ANSICHT ======================== */
    m_viewMenu = menuBar()->addMenu(QString());
    auto* modeGroup = new QActionGroup(this);

    m_actModeWysiwyg = m_viewMenu->addAction(icon("view-preview"), QString());
    m_actModeWysiwyg->setCheckable(true);
    m_actModeWysiwyg->setChecked(true);
    m_actModeWysiwyg->setShortcut(QKeySequence(QStringLiteral("Ctrl+Alt+1")));
    modeGroup->addAction(m_actModeWysiwyg);
    connect(m_actModeWysiwyg, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_VIEW_WYSIWYG); });

    m_actModeLive = m_viewMenu->addAction(icon("view-split-left-right"), QString());
    m_actModeLive->setCheckable(true);
    m_actModeLive->setShortcut(QKeySequence(QStringLiteral("Ctrl+Alt+2")));
    modeGroup->addAction(m_actModeLive);
    connect(m_actModeLive, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_VIEW_LIVE_PREVIEW); });

    m_actModeSource = m_viewMenu->addAction(icon("text-x-markdown"), QString());
    m_actModeSource->setCheckable(true);
    m_actModeSource->setShortcut(QKeySequence(QStringLiteral("Ctrl+Alt+3")));
    modeGroup->addAction(m_actModeSource);
    connect(m_actModeSource, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_VIEW_SOURCE); });

    m_actModePlain = m_viewMenu->addAction(icon("text-plain"), QString());
    m_actModePlain->setCheckable(true);
    m_actModePlain->setShortcut(QKeySequence(QStringLiteral("Ctrl+Alt+4")));
    modeGroup->addAction(m_actModePlain);
    connect(m_actModePlain, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_VIEW_PLAIN_TEXT); });

    m_viewMenu->addSeparator();

    m_marksMenu = m_viewMenu->addMenu(QString());
    auto* marksGroup = new QActionGroup(this);
    m_actMarksSlanted = m_marksMenu->addAction(QString());
    m_actMarksSlanted->setCheckable(true);
    m_actMarksSlanted->setChecked(true);
    marksGroup->addAction(m_actMarksSlanted);
    connect(m_actMarksSlanted, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_MARKS_SLANTED); });

    m_actMarksHash = m_marksMenu->addAction(QString());
    m_actMarksHash->setCheckable(true);
    marksGroup->addAction(m_actMarksHash);
    connect(m_actMarksHash, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_MARKS_HASH); });

    m_actMarksNone = m_marksMenu->addAction(QString());
    m_actMarksNone->setCheckable(true);
    marksGroup->addAction(m_actMarksNone);
    connect(m_actMarksNone, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_MARKS_NONE); });

    m_actLineNumbers = m_viewMenu->addAction(icon("view-list-numeric"), QString());
    m_actLineNumbers->setCheckable(true);
    m_actLineNumbers->setShortcut(QKeySequence(Qt::Key_F11));
    connect(m_actLineNumbers, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TOGGLE_LINE_NUMBERS); });

    m_actWrap = m_viewMenu->addAction(QString());
    m_actWrap->setCheckable(true);
    m_actWrap->setChecked(true);
    m_actWrap->setShortcut(QKeySequence(Qt::Key_F10));
    connect(m_actWrap, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TOGGLE_WORD_WRAP); });

    m_actFocus = m_viewMenu->addAction(QString());
    m_actFocus->setCheckable(true);
    connect(m_actFocus, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TOGGLE_FOCUS_MODE); });

    m_actTypewriter = m_viewMenu->addAction(QString());
    m_actTypewriter->setCheckable(true);
    connect(m_actTypewriter, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TOGGLE_TYPEWRITER); });

    m_viewMenu->addSeparator();
    QAction* actToggleFolder = m_folderDock->toggleViewAction();
    actToggleFolder->setShortcut(QKeySequence(Qt::Key_F7));
    m_viewMenu->addAction(actToggleFolder);

    QAction* actToggleOutline = m_outlineDock->toggleViewAction();
    actToggleOutline->setShortcut(QKeySequence(Qt::Key_F9));
    m_viewMenu->addAction(actToggleOutline);
    m_viewMenu->addSeparator();

    m_actZoomIn = m_viewMenu->addAction(icon("zoom-in"), QString());
    m_actZoomIn->setShortcut(QKeySequence::ZoomIn);
    connect(m_actZoomIn, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ZOOM_IN); });

    m_actZoomOut = m_viewMenu->addAction(icon("zoom-out"), QString());
    m_actZoomOut->setShortcut(QKeySequence::ZoomOut);
    connect(m_actZoomOut, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ZOOM_OUT); });

    m_actZoomReset = m_viewMenu->addAction(icon("zoom-original"), QString());
    connect(m_actZoomReset, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ZOOM_RESET); });

    /* ======================== 4. FORMAT ======================== */
    m_formatMenu = menuBar()->addMenu(QString());

    m_actBold = m_formatMenu->addAction(icon("format-text-bold"), QString());
    m_actBold->setCheckable(true);
    m_actBold->setShortcut(QKeySequence::Bold);
    connect(m_actBold, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_BOLD); });

    m_actItalic = m_formatMenu->addAction(icon("format-text-italic"), QString());
    m_actItalic->setCheckable(true);
    m_actItalic->setShortcut(QKeySequence::Italic);
    connect(m_actItalic, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ITALIC); });

    m_actStrike = m_formatMenu->addAction(icon("format-text-strikethrough"), QString());
    m_actStrike->setCheckable(true);
    m_actStrike->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+X")));
    connect(m_actStrike, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_STRIKE); });

    m_actHighlight = m_formatMenu->addAction(icon("format-text-color"), QString());
    m_actHighlight->setCheckable(true);
    m_actHighlight->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+H")));
    connect(m_actHighlight, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_HIGHLIGHT); });

    m_actInlineCode = m_formatMenu->addAction(icon("format-text-code"), QString());
    m_actInlineCode->setCheckable(true);
    m_actInlineCode->setShortcut(QKeySequence(QStringLiteral("Ctrl+E")));
    connect(m_actInlineCode, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_INLINE_CODE); });

    m_formatMenu->addSeparator();

    m_headingMenu = m_formatMenu->addMenu(QString());
    for (int lvl = 0; lvl <= 6; ++lvl) {
        auto* actH = m_headingMenu->addAction(QString());
        actH->setShortcut(QKeySequence(QStringLiteral("Ctrl+%1").arg(lvl)));
        connect(actH, &QAction::triggered, this, [this, lvl]() {
            dispatchCommand(static_cast<uint32_t>(KMARK_ACT_HEADING_0 + lvl));
        });
        m_actHeadings[static_cast<size_t>(lvl)] = actH;
    }

    m_formatMenu->addSeparator();

    m_actBullet = m_formatMenu->addAction(icon("format-list-unordered"), QString());
    m_actBullet->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+U")));
    connect(m_actBullet, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_LIST_BULLET); });

    m_actOrdered = m_formatMenu->addAction(icon("format-list-ordered"), QString());
    m_actOrdered->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+O")));
    connect(m_actOrdered, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_LIST_ORDERED); });

    m_actTask = m_formatMenu->addAction(icon("view-task"), QString());
    m_actTask->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+T")));
    connect(m_actTask, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_LIST_TASK); });

    m_actQuote = m_formatMenu->addAction(icon("format-text-blockquote"), QString());
    m_actQuote->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+Q")));
    connect(m_actQuote, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_BLOCKQUOTE); });

    m_alertMenu = m_formatMenu->addMenu(QString());
    m_actAlertNote = m_alertMenu->addAction(QString());
    m_actAlertTip = m_alertMenu->addAction(QString());
    m_actAlertImportant = m_alertMenu->addAction(QString());
    m_actAlertWarning = m_alertMenu->addAction(QString());
    m_actAlertCaution = m_alertMenu->addAction(QString());
    connect(m_actAlertNote, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ALERT_NOTE); });
    connect(m_actAlertTip, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ALERT_TIP); });
    connect(m_actAlertImportant, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ALERT_IMPORTANT); });
    connect(m_actAlertWarning, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ALERT_WARNING); });
    connect(m_actAlertCaution, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_ALERT_CAUTION); });

    m_formatMenu->addSeparator();

    m_actLink = m_formatMenu->addAction(icon("insert-link"), QString());
    m_actLink->setShortcut(QKeySequence(QStringLiteral("Ctrl+K")));
    connect(m_actLink, &QAction::triggered, this, &MainWindow::promptInsertLink);

    m_actImage = m_formatMenu->addAction(icon("insert-image"), QString());
    m_actImage->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+I")));
    connect(m_actImage, &QAction::triggered, this, &MainWindow::promptInsertImage);

    m_actFence = m_formatMenu->addAction(icon("code-context"), QString());
    m_actFence->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+C")));
    connect(m_actFence, &QAction::triggered, this, &MainWindow::promptInsertCodeFence);

    m_actFenceLang = m_formatMenu->addAction(icon("text-x-script"), QString());
    m_actFenceLang->setShortcut(QKeySequence(QStringLiteral("Ctrl+Alt+L")));
    connect(m_actFenceLang, &QAction::triggered, this, [this]() {
        const KMarkToolbarState st = kmark_session_toolbar_state(m_session);
        if (!st.in_code_block) {
            promptInsertCodeFence();
            return;
        }
        const QString curLang = takeFfiString(kmark_session_active_code_language_alloc(m_session));
        promptSetCodeFenceLanguage(false, 0, curLang, QCursor::pos());
    });

    m_actHr = m_formatMenu->addAction(icon("insert-horizontal-rule"), QString());
    connect(m_actHr, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_HORIZONTAL_RULE); });

    /* ======================== 5. TABLE / TABELLE ======================== */
    m_tableMenu = menuBar()->addMenu(QString());
    m_actInsTable = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::InsertTable, palette()), QString());
    connect(m_actInsTable, &QAction::triggered, this, &MainWindow::promptInsertTable);

    m_tableMenu->addSeparator();
    m_actRowAbove = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::RowAbove, palette()), QString());
    m_actRowBelow = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::RowBelow, palette()), QString());
    m_actRowDel = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::RowDelete, palette()), QString());
    connect(m_actRowAbove, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ROW_ABOVE); });
    connect(m_actRowBelow, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ROW_BELOW); });
    connect(m_actRowDel, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ROW_DELETE); });

    m_tableMenu->addSeparator();
    m_actColLeft = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::ColLeft, palette()), QString());
    m_actColRight = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::ColRight, palette()), QString());
    m_actColDel = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::ColDelete, palette()), QString());
    connect(m_actColLeft, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_COL_LEFT); });
    connect(m_actColRight, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_COL_RIGHT); });
    connect(m_actColDel, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_COL_DELETE); });

    m_tableMenu->addSeparator();
    m_actAlignLeft = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::AlignLeft, palette()), QString());
    m_actAlignLeft->setCheckable(true);
    m_actAlignCenter = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::AlignCenter, palette()), QString());
    m_actAlignCenter->setCheckable(true);
    m_actAlignRight = m_tableMenu->addAction(makeTableActionIcon(TableIconKind::AlignRight, palette()), QString());
    m_actAlignRight->setCheckable(true);
    connect(m_actAlignLeft, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ALIGN_LEFT); });
    connect(m_actAlignCenter, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ALIGN_CENTER); });
    connect(m_actAlignRight, &QAction::triggered, this, [this]() { dispatchCommand(KMARK_ACT_TABLE_ALIGN_RIGHT); });

    /* ======================== 6. SETTINGS / EINSTELLUNGEN ======================== */
    m_settingsMenu = menuBar()->addMenu(QString());
    m_presetsMenu = m_settingsMenu->addMenu(icon("color-management"), QString());
    m_settingsMenu->addMenu(m_marksMenu);
    m_settingsMenu->addSeparator();
    m_settingsMenu->addAction(m_folderDock->toggleViewAction());
    m_settingsMenu->addAction(m_outlineDock->toggleViewAction());
    m_settingsMenu->addSeparator();
    m_actConfigure = m_settingsMenu->addAction(icon("configure"), QString());
    m_actConfigure->setShortcut(QKeySequence(QStringLiteral("Ctrl+Shift+,")));
    connect(m_actConfigure, &QAction::triggered, this, &MainWindow::openSettingsDialog);

    /* ======================== 7. HELP / HILFE ======================== */
    m_helpMenu = menuBar()->addMenu(QString());
    m_actGuide = m_helpMenu->addAction(icon("help-contents"), QString());
    m_actGuide->setShortcut(QKeySequence::HelpContents);
    connect(m_actGuide, &QAction::triggered, this, [this]() {
        kmark_session_open_guide_tab(m_session);
        syncTabBar();
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    });

    m_actReference = m_helpMenu->addAction(icon("documentation"), QString());
    m_actReference->setShortcut(QKeySequence(QStringLiteral("Shift+F1")));
    connect(m_actReference, &QAction::triggered, this, [this]() {
        kmark_session_open_reference_tab(m_session);
        syncTabBar();
        syncOutlineTree();
        m_viewport->notifyDocumentChanged();
    });

    m_helpMenu->addSeparator();
    m_actAbout = m_helpMenu->addAction(icon("help-about"), QString());
    connect(m_actAbout, &QAction::triggered, this, [this]() {
        QMessageBox::about(
            this,
            T("About KMark", "Über KMark"),
            T(
                "<h3>KMark 0.1.0</h3>"
                "<p>Native WYSIWYG Markdown and plain-text editor for KDE Plasma.</p>"
                "<p>Architecture: Safe Rust engine (<code>Rope</code> + 2D virtualized layout) with native Qt6/KF6 shell.</p>"
                "<p><a href=\"https://github.com/colorcube/kmark\">https://github.com/colorcube/kmark</a></p>"
                "<p>License: GPL-3.0-or-later</p>",
                "<h3>KMark 0.1.0</h3>"
                "<p>Nativer WYSIWYG-Markdown- und Texteditor für KDE Plasma.</p>"
                "<p>Architektur: Sichere Rust-Engine (<code>Rope</code> + virtualisiertes 2D-Layout) mit nativer Qt6/KF6-Oberfläche.</p>"
                "<p><a href=\"https://github.com/colorcube/kmark\">https://github.com/colorcube/kmark</a></p>"
                "<p>Lizenz: GPL-3.0-or-later</p>"
            )
        );
    });
    m_actAboutQt = m_helpMenu->addAction(QString());
    connect(m_actAboutQt, &QAction::triggered, qApp, &QApplication::aboutQt);

    /* ======================== MAIN & FORMATTING TOOLBAR ======================== */
    m_mainToolBar = addToolBar(QString());
    m_mainToolBar->setObjectName(QStringLiteral("MainFormatToolBar"));
    m_mainToolBar->setMovable(false);
    m_mainToolBar->setToolButtonStyle(Qt::ToolButtonIconOnly);

    m_mainToolBar->addAction(m_actNew);
    m_mainToolBar->addAction(m_actOpen);
    m_mainToolBar->addAction(m_actSave);
    m_mainToolBar->addSeparator();
    m_mainToolBar->addAction(m_actUndo);
    m_mainToolBar->addAction(m_actRedo);
    m_mainToolBar->addSeparator();

    m_headingCombo = new QComboBox(m_mainToolBar);
    m_headingCombo->setMinimumWidth(134);
    connect(m_headingCombo, &QComboBox::currentIndexChanged, this, [this](int idx) {
        if (!m_updatingUi && idx >= 0 && idx <= 6) {
            dispatchCommand(static_cast<uint32_t>(KMARK_ACT_HEADING_0 + idx));
        }
    });
    m_mainToolBar->addWidget(m_headingCombo);
    m_mainToolBar->addSeparator();

    m_mainToolBar->addAction(m_actBold);
    m_mainToolBar->addAction(m_actItalic);
    m_mainToolBar->addAction(m_actStrike);
    m_mainToolBar->addAction(m_actHighlight);
    m_mainToolBar->addAction(m_actInlineCode);
    m_mainToolBar->addSeparator();

    m_mainToolBar->addAction(m_actBullet);
    m_mainToolBar->addAction(m_actOrdered);
    m_mainToolBar->addAction(m_actTask);
    m_mainToolBar->addAction(m_actQuote);
    m_mainToolBar->addSeparator();

    m_mainToolBar->addAction(m_actLink);
    m_mainToolBar->addAction(m_actImage);
    m_mainToolBar->addAction(m_actInsTable);
    m_mainToolBar->addAction(m_actFence);

    /* Contextual Table Toolbar section (shown automatically whenever cursor is inside a table) */
    auto* tableTools = new QWidget(m_mainToolBar);
    auto* tableToolsLayout = new QHBoxLayout(tableTools);
    tableToolsLayout->setContentsMargins(4, 0, 2, 0);
    tableToolsLayout->setSpacing(2);

    auto addTableDivider = [&]() {
        auto* sep = new QFrame(tableTools);
        sep->setFrameShape(QFrame::VLine);
        sep->setFrameShadow(QFrame::Plain);
        sep->setFixedWidth(1);
        sep->setFixedHeight(18);
        sep->setStyleSheet(QStringLiteral("background-color: rgba(128, 128, 128, 95); border: none;"));
        tableToolsLayout->addSpacing(2);
        tableToolsLayout->addWidget(sep);
        tableToolsLayout->addSpacing(2);
    };

    auto addTableBtn = [&](QAction* action) {
        auto* btn = new QToolButton(tableTools);
        btn->setAutoRaise(true);
        btn->setDefaultAction(action);
        btn->setToolButtonStyle(Qt::ToolButtonIconOnly);
        btn->setIconSize(m_mainToolBar->iconSize());
        tableToolsLayout->addWidget(btn);
    };

    addTableDivider();
    addTableBtn(m_actRowAbove);
    addTableBtn(m_actRowBelow);
    addTableBtn(m_actRowDel);
    addTableDivider();
    addTableBtn(m_actColLeft);
    addTableBtn(m_actColRight);
    addTableBtn(m_actColDel);
    addTableDivider();
    addTableBtn(m_actAlignLeft);
    addTableBtn(m_actAlignCenter);
    addTableBtn(m_actAlignRight);

    m_actTableToolbarSection = m_mainToolBar->addWidget(tableTools);
    m_actTableToolbarSection->setVisible(false);

    auto* spacer = new QWidget(m_mainToolBar);
    spacer->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    m_mainToolBar->addWidget(spacer);

    auto makeModeBtn = [&](QAction* action) {
        auto* btn = new QToolButton(m_mainToolBar);
        btn->setDefaultAction(action);
        btn->setToolButtonStyle(Qt::ToolButtonTextOnly);
        m_mainToolBar->addWidget(btn);
        return btn;
    };
    m_btnModeWysiwyg = makeModeBtn(m_actModeWysiwyg);
    m_btnModeLive = makeModeBtn(m_actModeLive);
    m_btnModeSource = makeModeBtn(m_actModeSource);
    m_btnModePlain = makeModeBtn(m_actModePlain);
}

void MainWindow::setupStatusBar() {
    m_statusCursor = new QLabel(this);
    m_statusCounts = new QLabel(this);
    m_statusMode = new QLabel(this);

    statusBar()->addWidget(m_statusCursor);
    statusBar()->addPermanentWidget(m_statusCounts);
    statusBar()->addPermanentWidget(m_statusMode);
}

void MainWindow::retranslateUi() {
    m_updatingUi = true;

    m_folderDock->setWindowTitle(T("Folder Tree", "Ordnerbaum"));
    m_outlineDock->setWindowTitle(T("Outline", "Gliederung"));
    m_mainToolBar->setWindowTitle(T("Main Toolbar", "Hauptwerkzeugleiste"));

    m_btnFolderUp->setToolTip(T("Parent Folder", "Übergeordneter Ordner"));
    m_btnFolderHome->setToolTip(T("Home Folder (~)", "Persönlicher Ordner (~)"));
    m_btnRevealActiveFile->setToolTip(
        T("Select Opened File in Folder Tree", "Pfad der aktiven Datei im Ordnerbaum aufklappen")
    );
    updateFolderHeaderUi();

    m_findLabel->setText(T("Find:", "Suchen:"));
    m_findInput->setPlaceholderText(T("Enter search term…", "Suchbegriff eingeben…"));
    m_btnFindPrev->setText(T("Previous", "Aufwärts"));
    m_btnFindNext->setText(T("Next", "Abwärts"));
    m_caseCheck->setText(T("Match Case", "Groß/Klein"));
    m_wordCheck->setText(T("Whole Word", "Ganzes Wort"));
    m_regexCheck->setText(T("RegEx", "RegEx"));
    m_btnCloseFind->setToolTip(T("Close search bar (Esc)", "Suchleiste schließen (Esc)"));
    m_replaceLabel->setText(T("Replace:", "Ersetzen:"));
    m_replaceInput->setPlaceholderText(T("Replacement text…", "Ersetzungstext…"));
    m_btnReplaceOne->setText(T("Replace", "Ersetzen"));
    m_btnReplaceAll->setText(T("Replace All", "Alle ersetzen"));

    m_fileMenu->setTitle(T("&File", "&Datei"));
    m_actNew->setText(T("&New", "&Neu"));
    m_actOpen->setText(T("&Open…", "&Öffnen…"));
    m_actOpenFolder->setText(T("Open Folde&r…", "O&rdner öffnen…"));
    m_recentMenu->setTitle(T("Open &Recent", "Zuletzt geöffnete &Dateien"));
    rebuildRecentFilesMenu();
    m_actSave->setText(T("&Save", "&Speichern"));
    m_actSaveAs->setText(T("Save &As…", "Speichern &unter…"));
    m_actExportHtml->setText(T("Export as &HTML5…", "Als &HTML5 exportieren…"));
    m_actPrintPreview->setText(T("Print Pre&view…", "Druck&vorschau…"));
    m_actPrint->setText(T("&Print…", "&Drucken…"));
    m_actCloseTab->setText(T("&Close Tab", "Tab s&chließen"));
    m_actQuit->setText(T("&Quit", "B&eenden"));

    m_editMenu->setTitle(T("&Edit", "&Bearbeiten"));
    m_actUndo->setText(T("&Undo", "&Rückgängig"));
    m_actRedo->setText(T("&Redo", "&Wiederherstellen"));
    m_actCut->setText(T("Cu&t", "&Ausschneiden"));
    m_actCopy->setText(T("&Copy", "&Kopieren"));
    m_actPaste->setText(T("&Paste", "E&infügen"));
    m_actSelectAll->setText(T("Select &All", "Alles aus&wählen"));
    m_actFind->setText(T("&Find…", "&Suchen…"));
    m_actReplace->setText(T("&Replace…", "&Ersetzen…"));

    m_viewMenu->setTitle(T("&View", "&Ansicht"));
    m_actModeWysiwyg->setText(T("&Formatted (WYSIWYG)", "&Formatiert (WYSIWYG)"));
    m_actModeLive->setText(T("&Live Preview", "&Live-Vorschau"));
    m_actModeSource->setText(T("Markdown &Source", "Markdown-&Quelltext"));
    m_actModePlain->setText(T("&Plain Text (TXT)", "&Reintext (TXT)"));

    m_marksMenu->setTitle(T("Heading Indicators", "Überschriften-Markierung"));
    m_actMarksSlanted->setText(T("Slanted Bars (MDNote Style)", "Schräge Balken (MDNote-Stil)"));
    m_actMarksHash->setText(T("Hash Symbol (#)", "Raute-Symbol (#)"));
    m_actMarksNone->setText(T("No Indicators", "Keine Markierung"));

    m_actLineNumbers->setText(T("Show &Line Numbers", "&Zeilennummern anzeigen"));
    m_actWrap->setText(T("Dynamic &Word Wrap", "Dynamischer Zeilen&umbruch"));
    m_actFocus->setText(T("Focus Mode (Highlight Active Block)", "Fokus-Modus (aktiven Absatz hervorheben)"));
    m_actTypewriter->setText(T("Typewriter Mode (Center Caret)", "Schreibmaschinen-Modus (Cursor zentrieren)"));
    m_actZoomIn->setText(T("Zoom &In", "Schrift ver&größern"));
    m_actZoomOut->setText(T("Zoom &Out", "Schrift ver&kleinern"));
    m_actZoomReset->setText(T("&Reset Font Size", "Schriftgröße &zurücksetzen"));

    m_formatMenu->setTitle(T("F&ormat", "F&ormat"));
    m_actBold->setText(T("&Bold", "&Fett"));
    m_actItalic->setText(T("&Italic", "&Kursiv"));
    m_actStrike->setText(T("&Strikethrough", "&Durchgestrichen"));
    m_actHighlight->setText(T("&Highlight (==…==)", "&Textmarker (==…==)"));
    m_actInlineCode->setText(T("Inline &Code", "Inline-&Code"));

    m_headingMenu->setTitle(T("&Heading", "&Überschrift"));
    m_actHeadings[0]->setText(T("Paragraph (Body Text)", "Absatz (Normaltext)"));
    for (int lvl = 1; lvl <= 6; ++lvl) {
        m_actHeadings[static_cast<size_t>(lvl)]->setText(
            T("Heading %1", "Überschrift %1").arg(lvl)
        );
    }

    m_actBullet->setText(T("&Bullet List", "&Aufzählungsliste"));
    m_actOrdered->setText(T("&Numbered List", "&Nummerierte Liste"));
    m_actTask->setText(T("&Task List (- [ ])", "Auf&gabenliste (- [ ])"));
    m_actQuote->setText(T("Block&quote", "&Zitatblock"));

    m_alertMenu->setTitle(T("Admonition (&Callout)", "Hinweis-Block (&Callout)"));
    m_actAlertNote->setText(T("Note [!NOTE]", "Hinweis [!NOTE]"));
    m_actAlertTip->setText(T("Tip [!TIP]", "Tipp [!TIP]"));
    m_actAlertImportant->setText(T("Important [!IMPORTANT]", "Wichtig [!IMPORTANT]"));
    m_actAlertWarning->setText(T("Warning [!WARNING]", "Warnung [!WARNING]"));
    m_actAlertCaution->setText(T("Caution [!CAUTION]", "Vorsicht [!CAUTION]"));

    m_actLink->setText(T("Insert &Link…", "&Link einfügen…"));
    m_actImage->setText(T("Insert &Image…", "&Bild einfügen…"));
    m_actFence->setText(T("Insert C&ode Block…", "C&odeblock einfügen…"));
    m_actFenceLang->setText(T("Code Block &Language…", "Codeblock-&Sprache festlegen…"));
    m_actHr->setText(T("Horizontal &Rule", "Horizontale &Trennlinie"));

    m_tableMenu->setTitle(T("&Table", "&Tabelle"));
    m_actInsTable->setText(T("&Insert Table…", "&Tabelle einfügen…"));
    m_actRowAbove->setText(T("Insert Row &Above", "Zeile &oberhalb einfügen"));
    m_actRowBelow->setText(T("Insert Row &Below", "Zeile &unterhalb einfügen"));
    m_actRowDel->setText(T("&Delete Row", "Zeile &löschen"));
    m_actColLeft->setText(T("Insert Column &Left", "Spalte &links einfügen"));
    m_actColRight->setText(T("Insert Column &Right", "Spalte &rechts einfügen"));
    m_actColDel->setText(T("D&elete Column", "Spalte l&öschen"));
    m_actAlignLeft->setText(T("Align Column Left", "Spalte linksbündig"));
    m_actAlignCenter->setText(T("Align Column Center", "Spalte zentriert"));
    m_actAlignRight->setText(T("Align Column Right", "Spalte rechtsbündig"));

    m_settingsMenu->setTitle(T("&Settings", "&Einstellungen"));
    m_presetsMenu->setTitle(T("Appearance &Presets", "Design-&Presets"));
    m_actConfigure->setText(T("&Configure KMark…", "&KMark einrichten…"));
    rebuildPresetsMenu();

    m_helpMenu->setTitle(T("&Help", "&Hilfe"));
    m_actGuide->setText(T("Markdown &Quick Reference", "&Markdown-Kurzreferenz öffnen"));
    m_actReference->setText(T("Markdown &Complete Reference", "Markdown-&Langreferenz öffnen"));
    m_actAbout->setText(T("About &KMark", "Über &KMark"));
    m_actAboutQt->setText(T("About &Qt", "Über &Qt"));

    {
        const QSignalBlocker b(m_headingCombo);
        const int cur = std::max(0, m_headingCombo->currentIndex());
        m_headingCombo->clear();
        m_headingCombo->addItems({
            T("Paragraph", "Absatz"),
            T("Heading 1", "Überschrift 1"),
            T("Heading 2", "Überschrift 2"),
            T("Heading 3", "Überschrift 3"),
            T("Heading 4", "Überschrift 4"),
            T("Heading 5", "Überschrift 5"),
            T("Heading 6", "Überschrift 6"),
        });
        m_headingCombo->setCurrentIndex(cur);
    }

    m_btnModeWysiwyg->setText(T("Formatted", "Formatiert"));
    m_btnModeLive->setText(T("Live", "Live"));
    m_btnModeSource->setText(T("Source", "Quelltext"));
    m_btnModePlain->setText(T("TXT", "TXT"));

    m_updatingUi = false;
}

void MainWindow::dispatchCommand(uint32_t cmdId) {
    kmark_session_exec_command(m_session, cmdId);
    if (cmdId >= KMARK_ACT_VIEW_WYSIWYG) {
        syncSettingsFromRuntime(true);
    }
    syncOutlineTree();
    m_viewport->notifyDocumentChanged();
    m_viewport->setFocus();
}

void MainWindow::syncUiState() {
    if (m_session == nullptr) {
        return;
    }
    if (kmark_session_is_poisoned(m_session)) {
        if (!m_handlingPoisonedSession) {
            QTimer::singleShot(0, this, &MainWindow::handlePoisonedSession);
        }
        return;
    }
    m_updatingUi = true;
    const KMarkToolbarState st = kmark_session_toolbar_state(m_session);
    const KMarkLayoutSettings ls = kmark_session_get_layout_settings(m_session);

    m_actUndo->setEnabled(st.can_undo);
    m_actRedo->setEnabled(st.can_redo);
    m_actBold->setChecked(st.bold);
    m_actItalic->setChecked(st.italic);
    m_actStrike->setChecked(st.strike);
    m_actHighlight->setChecked(st.highlight);
    m_actInlineCode->setChecked(st.inline_code);

    {
        const QSignalBlocker b(m_headingCombo);
        m_headingCombo->setCurrentIndex(std::clamp(static_cast<int>(st.heading_level), 0, 6));
    }

    switch (st.view_mode) {
        case KMARK_VIEW_WYSIWYG:      m_actModeWysiwyg->setChecked(true); break;
        case KMARK_VIEW_LIVE_PREVIEW: m_actModeLive->setChecked(true); break;
        case KMARK_VIEW_SOURCE:       m_actModeSource->setChecked(true); break;
        case KMARK_VIEW_PLAIN_TEXT:   m_actModePlain->setChecked(true); break;
        default: break;
    }

    switch (ls.heading_marks) {
        case KMARK_MARKS_SLANTED: m_actMarksSlanted->setChecked(true); break;
        case KMARK_MARKS_HASH:    m_actMarksHash->setChecked(true); break;
        case KMARK_MARKS_NONE:    m_actMarksNone->setChecked(true); break;
        default: break;
    }
    m_actLineNumbers->setChecked(ls.show_line_numbers);
    m_actWrap->setChecked(ls.word_wrap);
    m_actFocus->setChecked(ls.focus_mode);
    m_actTypewriter->setChecked(ls.typewriter_mode);

    /* Contextual Table Toolbar & Table Menu actions */
    if (m_actTableToolbarSection != nullptr) {
        m_actTableToolbarSection->setVisible(st.in_table);
    }
    for (QAction* tblAct : {
             m_actRowAbove,
             m_actRowBelow,
             m_actRowDel,
             m_actColLeft,
             m_actColRight,
             m_actColDel,
             m_actAlignLeft,
             m_actAlignCenter,
             m_actAlignRight,
         }) {
        if (tblAct != nullptr) {
            tblAct->setEnabled(st.in_table);
        }
    }
    m_actAlignLeft->setChecked(st.in_table && st.table_col_align == KMARK_ALIGN_LEFT);
    m_actAlignCenter->setChecked(st.in_table && st.table_col_align == KMARK_ALIGN_CENTER);
    m_actAlignRight->setChecked(st.in_table && st.table_col_align == KMARK_ALIGN_RIGHT);

    /* Re-apply short labels on mode buttons because QAction::setChecked can sync QToolButton text */
    m_btnModeWysiwyg->setText(T("Formatted", "Formatiert"));
    m_btnModeLive->setText(T("Live", "Live"));
    m_btnModeSource->setText(T("Source", "Quelltext"));
    m_btnModePlain->setText(T("TXT", "TXT"));

    m_statusCursor->setText(
        T("Line %1, Col %2", "Zeile %1, Spalte %2").arg(st.line_1based).arg(st.col_1based)
    );
    m_statusCounts->setText(
        T("%1 words · %2 chars", "%1 Wörter · %2 Zeichen").arg(st.word_count).arg(st.char_count)
    );

    QString modeLabel = T("Formatted", "Formatiert");
    if (st.view_mode == KMARK_VIEW_LIVE_PREVIEW) {
        modeLabel = T("Live Preview", "Live-Vorschau");
    } else if (st.view_mode == KMARK_VIEW_SOURCE) {
        modeLabel = T("Source", "Quelltext");
    } else if (st.view_mode == KMARK_VIEW_PLAIN_TEXT) {
        modeLabel = T("Plain Text (TXT)", "Reintext (TXT)");
    }

    if (st.in_code_block) {
        const QString codeLang = takeFfiString(kmark_session_active_code_language_alloc(m_session));
        const QString badge = codeLang.isEmpty() ? QStringLiteral("Code") : codeLang;
        m_statusMode->setText(QStringLiteral("[%1] · %2 · UTF-8").arg(badge, modeLabel));
    } else {
        m_statusMode->setText(QStringLiteral("%1 · UTF-8").arg(modeLabel));
    }

    if (m_findBar->isVisible()) {
        if (st.search_match_count == 0) {
            m_matchCountLabel->setText(T("0 matches", "0 Treffer"));
        } else {
            m_matchCountLabel->setText(QStringLiteral("%1 / %2").arg(st.search_active_1based).arg(st.search_match_count));
        }
    }

    /* Update active tab title dirty marker (`*`) & window title */
    const size_t active = kmark_session_active_tab(m_session);
    if (static_cast<int>(active) < m_tabBar->count()) {
        QString title = takeFfiString(kmark_session_tab_title_alloc(m_session, active));
        if (st.is_dirty) {
            title.append(QStringLiteral(" *"));
        }
        m_tabBar->setTabText(static_cast<int>(active), title);
        setWindowTitle(QStringLiteral("%1 — KMark").arg(title));
    }

    if (m_btnRevealActiveFile != nullptr) {
        m_btnRevealActiveFile->setEnabled(kmark_session_tab_has_path(m_session, active));
    }
    refreshToolbarIcons();

    m_updatingUi = false;
}

void MainWindow::syncTabBar() {
    m_updatingUi = true;
    while (m_tabBar->count() > 0) {
        m_tabBar->removeTab(0);
    }
    const size_t count = kmark_session_tab_count(m_session);
    for (size_t i = 0; i < count; ++i) {
        QString title = takeFfiString(kmark_session_tab_title_alloc(m_session, i));
        if (kmark_session_tab_is_dirty(m_session, i)) {
            title.append(QStringLiteral(" *"));
        }
        const int idx = m_tabBar->addTab(title);
        const QString fullPath = takeFfiString(kmark_session_tab_path_alloc(m_session, i));
        if (!fullPath.isEmpty()) {
            m_tabBar->setTabToolTip(idx, fullPath);
        }
    }
    m_tabBar->setCurrentIndex(static_cast<int>(kmark_session_active_tab(m_session)));
    m_updatingUi = false;
}

void MainWindow::syncOutlineTree() {
    m_outlineTree->clear();
    const KMarkOutlineView view = kmark_session_outline(m_session);
    if (view.items_len == 0 || view.items_ptr == nullptr) {
        return;
    }

    QTreeWidgetItem* levelStack[7] = {nullptr};
    for (size_t i = 0; i < view.items_len; ++i) {
        const KMarkOutlineItem& it = view.items_ptr[i];
        QString title;
        if (it.title_len > 0 && view.text_pool_ptr != nullptr) {
            const auto* start = reinterpret_cast<const char*>(view.text_pool_ptr + it.title_offset);
            title = QString::fromUtf8(start, static_cast<qsizetype>(it.title_len));
        }

        const int lvl = std::clamp(static_cast<int>(it.level), 1, 6);
        QTreeWidgetItem* parentItem = nullptr;
        for (int p = lvl - 1; p >= 1; --p) {
            if (levelStack[p] != nullptr) {
                parentItem = levelStack[p];
                break;
            }
        }

        auto* node = (parentItem != nullptr)
            ? new QTreeWidgetItem(parentItem, {title})
            : new QTreeWidgetItem(m_outlineTree, {title});
        node->setData(0, Qt::UserRole, static_cast<qulonglong>(it.byte_offset));

        levelStack[lvl] = node;
        for (int clearLvl = lvl + 1; clearLvl <= 6; ++clearLvl) {
            levelStack[clearLvl] = nullptr;
        }
    }
    m_outlineTree->expandAll();
}

void MainWindow::actionNewTab() {
    kmark_session_new_tab(m_session);
    syncTabBar();
    syncOutlineTree();
    m_viewport->notifyDocumentChanged();
    m_viewport->setFocus();
}

void MainWindow::actionOpenFile() {
    const QString path = QFileDialog::getOpenFileName(
        this,
        T("Open Document", "Dokument öffnen"),
        QString(),
        T(
            "Markdown & Text Files (*.md *.markdown *.txt *.rst *.toml *.json *.yaml);;All Files (*)",
            "Markdown & Textdateien (*.md *.markdown *.txt *.rst *.toml *.json *.yaml);;Alle Dateien (*)"
        )
    );
    if (!path.isEmpty()) {
        openPathFromCli(path);
    }
}

void MainWindow::actionOpenFolder() {
    const QString dir = QFileDialog::getExistingDirectory(
        this,
        T("Open Workspace Folder", "Arbeitsordner öffnen"),
        m_folderRootPath
    );
    if (!dir.isEmpty()) {
        setFolderRoot(dir);
        m_folderDock->show();
    }
}

bool MainWindow::actionSave() {
    const size_t active = kmark_session_active_tab(m_session);
    if (!kmark_session_tab_has_path(m_session, active)) {
        return actionSaveAs();
    }
    if (!kmark_session_save_active(m_session)) {
        const QString err = takeFfiString(kmark_session_last_error_alloc(m_session));
        QMessageBox::critical(this, T("Save Failed", "Speichern fehlgeschlagen"), err);
        return false;
    }
    syncTabBar();
    syncUiState();
    statusBar()->showMessage(T("Document saved.", "Dokument gespeichert."), 3000);
    return true;
}

bool MainWindow::actionSaveAs() {
    const QString path = QFileDialog::getSaveFileName(
        this,
        T("Save Document As", "Dokument speichern unter"),
        T("Note.md", "Notiz.md"),
        T(
            "Markdown Document (*.md *.markdown);;Plain Text (*.txt);;All Files (*)",
            "Markdown-Dokument (*.md *.markdown);;Textdatei (*.txt);;Alle Dateien (*)"
        )
    );
    if (path.isEmpty()) {
        return false;
    }
    const QString absSaved = QFileInfo(path).absoluteFilePath();
    const QByteArray utf8 = absSaved.toUtf8();
    if (!kmark_session_save_active_as(m_session, utf8.constData())) {
        const QString err = takeFfiString(kmark_session_last_error_alloc(m_session));
        QMessageBox::critical(this, T("Save Failed", "Speichern fehlgeschlagen"), err);
        return false;
    }
    recordRecentFile(absSaved);
    m_viewport->clearImageCache();
    syncTabBar();
    syncUiState();
    statusBar()->showMessage(T("Saved as %1", "Gespeichert unter %1").arg(absSaved), 3000);
    return true;
}

void MainWindow::actionExportHtml() {
    const QString path = QFileDialog::getSaveFileName(
        this,
        T("Export as HTML5", "Als HTML5 exportieren"),
        QStringLiteral("Export.html"),
        T("HTML5 Document (*.html)", "HTML5-Dokument (*.html)")
    );
    if (path.isEmpty()) {
        return;
    }
    const QString html = takeFfiString(kmark_session_export_html_alloc(m_session));
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        QMessageBox::critical(this, T("Export Failed", "Export fehlgeschlagen"), f.errorString());
        return;
    }
    f.write(html.toUtf8());
    f.close();
    statusBar()->showMessage(T("HTML5 exported to %1", "HTML5 exportiert nach %1").arg(path), 4000);
}

void MainWindow::actionPrintPreview() {
    QPrinter printer(QPrinter::HighResolution);
    QPrintPreviewDialog preview(&printer, this);
    connect(&preview, &QPrintPreviewDialog::paintRequested, this, [this](QPrinter* p) {
        QTextDocument doc;
        doc.setHtml(takeFfiString(kmark_session_export_html_alloc(m_session)));
        doc.print(p);
    });
    preview.exec();
}

void MainWindow::actionPrint() {
    QPrinter printer(QPrinter::HighResolution);
    QPrintDialog dlg(&printer, this);
    if (dlg.exec() == QDialog::Accepted) {
        QTextDocument doc;
        doc.setHtml(takeFfiString(kmark_session_export_html_alloc(m_session)));
        doc.print(&printer);
    }
}

bool MainWindow::confirmCloseTab(size_t tabIdx) {
    if (!kmark_session_tab_is_dirty(m_session, tabIdx)) {
        return true;
    }
    const QString title = takeFfiString(kmark_session_tab_title_alloc(m_session, tabIdx));
    const auto choice = QMessageBox::question(
        this,
        T("Unsaved Changes", "Ungespeicherte Änderungen"),
        T(
            "The document “%1” has been modified. Do you want to save your changes?",
            "Das Dokument „%1“ wurde geändert. Möchten Sie die Änderungen speichern?"
        ).arg(title),
        QMessageBox::Save | QMessageBox::Discard | QMessageBox::Cancel,
        QMessageBox::Save
    );
    if (choice == QMessageBox::Cancel) {
        return false;
    }
    if (choice == QMessageBox::Save) {
        kmark_session_set_active_tab(m_session, tabIdx);
        return actionSave();
    }
    return true;
}

void MainWindow::handlePoisonedSession() {
    if (m_handlingPoisonedSession || m_session == nullptr || !kmark_session_is_poisoned(m_session)) {
        return;
    }
    m_handlingPoisonedSession = true;
    m_recoveryTimer.stop();
    if (m_viewport != nullptr) {
        m_viewport->setEnabled(false);
    }

    QMessageBox box(this);
    box.setIcon(QMessageBox::Critical);
    box.setWindowTitle(T("Internal Engine Error — Emergency Save", "Interner Engine-Fehler — Notfallspeicherung"));
    box.setText(T(
        "An unexpected internal error occurred in the editor engine.\n\n"
        "To protect your files on disk, further editing has been locked. "
        "Would you like to save emergency plain-text copies of your open documents before closing KMark?",
        "In der Editor-Engine ist ein unerwarteter interner Fehler aufgetreten.\n\n"
        "Zum Schutz Ihrer bestehenden Dateien wurde die weitere Bearbeitung gesperrt. "
        "Möchten Sie vor dem Schließen von KMark Notfall-Kopien Ihrer geöffneten Dokumente speichern?"
    ));
    QPushButton* saveBtn = box.addButton(
        T("Save Emergency Copies…", "Notfall-Kopien speichern…"),
        QMessageBox::AcceptRole
    );
    box.addButton(T("Close Without Saving", "Ohne Speichern beenden"), QMessageBox::RejectRole);
    box.setDefaultButton(saveBtn);
    box.exec();

    if (box.clickedButton() == saveBtn) {
        const size_t count = kmark_session_tab_count(m_session);
        for (size_t i = 0; i < count; ++i) {
            const QString content = takeFfiString(kmark_session_emergency_text_alloc(m_session, i));
            const bool dirty = kmark_session_tab_is_dirty(m_session, i);
            if (content.isEmpty() && !dirty) {
                continue;
            }
            const QString origPath = takeFfiString(kmark_session_tab_path_alloc(m_session, i));
            const QString title = takeFfiString(kmark_session_tab_title_alloc(m_session, i));

            QString suggested;
            if (!origPath.isEmpty()) {
                const QFileInfo fi(origPath);
                const QString ext = fi.suffix().isEmpty() ? QStringLiteral("md") : fi.suffix();
                suggested = fi.dir().filePath(
                    QStringLiteral("%1-recovered.%2").arg(fi.completeBaseName(), ext)
                );
            } else {
                QString safeTitle = title.isEmpty() ? QStringLiteral("document") : title;
                safeTitle.replace(QLatin1Char('/'), QLatin1Char('_'));
                suggested = QDir::home().filePath(QStringLiteral("%1-recovered.md").arg(safeTitle));
            }

            const QString dest = QFileDialog::getSaveFileName(
                this,
                T("Save Emergency Copy (%1)", "Notfall-Kopie speichern (%1)").arg(title),
                suggested,
                T(
                    "Markdown & Text Files (*.md *.markdown *.txt);;All Files (*)",
                    "Markdown- & Textdateien (*.md *.markdown *.txt);;Alle Dateien (*)"
                )
            );
            if (dest.isEmpty()) {
                continue;
            }
            QSaveFile out(dest);
            if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate)
                || out.write(content.toUtf8()) < 0
                || !out.commit()) {
                QMessageBox::warning(
                    this,
                    T("Emergency Save Failed", "Notfallspeicherung fehlgeschlagen"),
                    out.errorString()
                );
            }
        }
    }
    close();
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (m_session != nullptr && kmark_session_is_poisoned(m_session)) {
        if (!m_handlingPoisonedSession) {
            handlePoisonedSession();
        }
        m_recoveryTimer.stop();
        m_settings.lastFolderRoot = m_folderRootPath;
        m_settings.windowGeometry = saveGeometry();
        m_settings.windowState = saveState();
        m_settings.save();
        event->accept();
        return;
    }

    const size_t count = kmark_session_tab_count(m_session);
    for (size_t i = 0; i < count; ++i) {
        if (!confirmCloseTab(i)) {
            event->ignore();
            return;
        }
    }
    m_recoveryTimer.stop();
    if (m_session != nullptr) {
        kmark_session_clear_recovery(m_session);
    }
    syncSettingsFromRuntime(false);
    m_settings.lastFolderRoot = m_folderRootPath;
    m_settings.windowGeometry = saveGeometry();
    m_settings.windowState = saveState();
    m_settings.save();
    event->accept();
}

void MainWindow::promptInsertLink() {
    QDialog dlg(this);
    dlg.setWindowTitle(T("Insert Link", "Link einfügen"));
    auto* form = new QFormLayout(&dlg);
    auto* txtEdit = new QLineEdit(takeFfiString(kmark_session_selected_text_alloc(m_session)), &dlg);
    auto* urlEdit = new QLineEdit(QStringLiteral("https://"), &dlg);
    form->addRow(T("Link text:", "Anzeigetext:"), txtEdit);
    form->addRow(T("Target URL:", "Adresse (URL):"), urlEdit);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);
    form->addRow(buttons);

    if (dlg.exec() == QDialog::Accepted) {
        const QByteArray t = txtEdit->text().toUtf8();
        const QByteArray u = urlEdit->text().toUtf8();
        kmark_session_insert_link(m_session, t.constData(), u.constData());
        m_viewport->notifyDocumentChanged();
    }
}

void MainWindow::promptInsertImage() {
    QDialog dlg(this);
    dlg.setWindowTitle(T("Insert Image", "Bild einfügen"));
    auto* form = new QFormLayout(&dlg);
    auto* altEdit = new QLineEdit(T("Image description", "Bildbeschreibung"), &dlg);
    auto* pathRow = new QHBoxLayout();
    auto* urlEdit = new QLineEdit(&dlg);
    urlEdit->setMinimumWidth(320);
    auto* browseBtn = new QPushButton(T("Browse…", "Durchsuchen…"), &dlg);
    pathRow->addWidget(urlEdit, 1);
    pathRow->addWidget(browseBtn);

    auto* captionEdit = new QLineEdit(&dlg);
    captionEdit->setPlaceholderText(
        T("Optional (e.g. Figure 1: Overview)", "Optional (z. B. Abbildung 1: Übersicht)")
    );

    auto* sizeCombo = new QComboBox(&dlg);
    sizeCombo->addItem(T("Automatic (Original Size)", "Automatisch (Originalgröße)"), 0);
    for (int pct : {25, 33, 50, 66, 75, 100}) {
        const QString label = (pct == 100)
            ? T("100 % (Column Width)", "100 % (Spaltenbreite)")
            : QStringLiteral("%1 %").arg(pct);
        sizeCombo->addItem(label, pct);
    }

    connect(browseBtn, &QPushButton::clicked, &dlg, [this, &dlg, urlEdit, altEdit]() {
        const QString f = QFileDialog::getOpenFileName(
            &dlg,
            T("Choose Image File", "Bilddatei wählen"),
            m_viewport->activeDocumentDir(),
            T(
                "Images (*.png *.jpg *.jpeg *.svg *.webp *.gif *.bmp *.avif)",
                "Bilder (*.png *.jpg *.jpeg *.svg *.webp *.gif *.bmp *.avif)"
            )
        );
        if (!f.isEmpty()) {
            urlEdit->setText(m_viewport->makeFriendlyMarkdownPath(f));
            if (altEdit->text().isEmpty()
                || altEdit->text() == QStringLiteral("Image description")
                || altEdit->text() == QStringLiteral("Bildbeschreibung")) {
                altEdit->setText(QFileInfo(f).completeBaseName());
            }
        }
    });

    form->addRow(T("Image path / URL:", "Bildpfad / URL:"), pathRow);
    form->addRow(T("Alt text:", "Alternativtext:"), altEdit);
    form->addRow(T("Caption:", "Bildunterschrift:"), captionEdit);
    form->addRow(T("Size:", "Größe:"), sizeCombo);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);
    form->addRow(buttons);

    if (dlg.exec() == QDialog::Accepted && !urlEdit->text().trimmed().isEmpty()) {
        const QByteArray a = altEdit->text().toUtf8();
        const QByteArray u = urlEdit->text().trimmed().toUtf8();
        const QByteArray c = captionEdit->text().trimmed().toUtf8();
        const int pct = sizeCombo->currentData().toInt();
        const uint8_t sizeKind = (pct > 0) ? KMARK_IMG_SIZE_PERCENT : KMARK_IMG_SIZE_AUTO;
        const uint16_t sizeValue = (pct > 0) ? static_cast<uint16_t>(pct) : 0;
        kmark_session_insert_image_ex(
            m_session,
            a.constData(),
            u.constData(),
            c.isEmpty() ? nullptr : c.constData(),
            sizeKind,
            sizeValue
        );
        m_viewport->clearImageCache();
        m_viewport->notifyDocumentChanged();
    }
}

void MainWindow::promptInsertTable() {
    QDialog dlg(this);
    dlg.setWindowTitle(T("Insert Table", "Tabelle einfügen"));
    auto* form = new QFormLayout(&dlg);
    auto* colsSpin = new QSpinBox(&dlg);
    colsSpin->setRange(1, 16);
    colsSpin->setValue(3);
    auto* rowsSpin = new QSpinBox(&dlg);
    rowsSpin->setRange(1, 64);
    rowsSpin->setValue(3);
    form->addRow(T("Columns:", "Spaltenanzahl:"), colsSpin);
    form->addRow(T("Data rows:", "Daten-Zeilen:"), rowsSpin);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);
    form->addRow(buttons);

    if (dlg.exec() == QDialog::Accepted) {
        kmark_session_insert_table(
            m_session,
            static_cast<size_t>(colsSpin->value()),
            static_cast<size_t>(rowsSpin->value())
        );
        m_viewport->notifyDocumentChanged();
        m_viewport->setFocus();
    }
}

void MainWindow::promptInsertCodeFence() {
    QStringList items = commonCodeLanguages();
    items.prepend(QString()); /* allow plain code fence */
    bool ok = false;
    const QString lang = QInputDialog::getItem(
        this,
        T("Insert Code Block", "Codeblock einfügen"),
        T("Select or type programming language:", "Programmiersprache auswählen oder eingeben:"),
        items,
        1, /* default "rust" */
        true,
        &ok
    );
    if (ok) {
        const QByteArray utf8 = lang.trimmed().toUtf8();
        kmark_session_insert_code_fence(m_session, utf8.constData());
        m_viewport->notifyDocumentChanged();
        m_viewport->setFocus();
    }
}

void MainWindow::promptSetCodeFenceLanguage(
    bool hasOffset,
    size_t openLineStart,
    const QString& currentLang,
    const QPoint& popupPos
) {
    QMenu menu(this);
    menu.setTitle(T("Code Block Language", "Codeblock-Sprache"));

    QAction* actPlain = menu.addAction(T("None (Plain Code Block)", "Keine (Reiner Codeblock)"));
    actPlain->setCheckable(true);
    actPlain->setChecked(currentLang.trimmed().isEmpty());

    menu.addSeparator();

    QHash<QAction*, QString> actionMap;
    actionMap.insert(actPlain, QString());

    for (const QString& lang : commonCodeLanguages()) {
        QAction* act = menu.addAction(lang);
        act->setCheckable(true);
        if (currentLang.compare(lang, Qt::CaseInsensitive) == 0) {
            act->setChecked(true);
        }
        actionMap.insert(act, lang);
    }

    menu.addSeparator();
    QAction* actCustom = menu.addAction(T("Custom Language…", "Eigene Sprache eingeben…"));

    QAction* chosen = menu.exec(popupPos);
    if (chosen == nullptr) {
        return;
    }

    QString chosenLang;
    if (chosen == actCustom) {
        bool ok = false;
        const QString custom = QInputDialog::getText(
            this,
            T("Code Block Language", "Codeblock-Sprache"),
            T("Language identifier (e.g. rust, nix, lua, glsl):", "Sprachkennung (z. B. rust, nix, lua, glsl):"),
            QLineEdit::Normal,
            currentLang,
            &ok
        );
        if (!ok) {
            return;
        }
        chosenLang = custom.trimmed();
    } else if (actionMap.contains(chosen)) {
        chosenLang = actionMap.value(chosen);
    } else {
        return;
    }

    const QByteArray utf8 = chosenLang.toUtf8();
    kmark_session_set_code_fence_language(m_session, hasOffset, openLineStart, utf8.constData());
    m_viewport->notifyDocumentChanged();
    m_viewport->setFocus();
}

void MainWindow::showFindBar(bool withReplace) {
    m_replaceRow->setVisible(withReplace);
    m_findBar->show();
    const QString sel = takeFfiString(kmark_session_selected_text_alloc(m_session));
    if (!sel.isEmpty() && !sel.contains(QLatin1Char('\n'))) {
        m_findInput->setText(sel);
    }
    m_findInput->setFocus();
    m_findInput->selectAll();
    updateSearchQuery();
}

void MainWindow::hideFindBar() {
    m_findBar->hide();
    kmark_session_set_search(m_session, "", false, false, false);
    m_viewport->notifyDocumentChanged();
    m_viewport->setFocus();
}

void MainWindow::updateSearchQuery() {
    const QByteArray q = m_findInput->text().toUtf8();
    kmark_session_set_search(
        m_session,
        q.constData(),
        m_caseCheck->isChecked(),
        m_wordCheck->isChecked(),
        m_regexCheck->isChecked()
    );
    m_viewport->notifyDocumentChanged();
}

void MainWindow::promptSetCalloutKind(
    bool hasOffset,
    size_t openLineStart,
    const QString& currentTag,
    const QPoint& popupPos
) {
    QMenu menu(this);
    menu.setTitle(T("Callout Type", "Callout-Typ"));

    struct CalloutOption {
        const char* tag;
        uint8_t kind;
        QString label;
    };
    const std::array<CalloutOption, 5> options = {{
        {"NOTE",      KMARK_ALERT_NOTE,      T("Note [!NOTE]", "Hinweis [!NOTE]")},
        {"TIP",       KMARK_ALERT_TIP,       T("Tip [!TIP]", "Tipp [!TIP]")},
        {"IMPORTANT", KMARK_ALERT_IMPORTANT, T("Important [!IMPORTANT]", "Wichtig [!IMPORTANT]")},
        {"WARNING",   KMARK_ALERT_WARNING,   T("Warning [!WARNING]", "Warnung [!WARNING]")},
        {"CAUTION",   KMARK_ALERT_CAUTION,   T("Caution [!CAUTION]", "Vorsicht [!CAUTION]")},
    }};

    QHash<QAction*, uint8_t> actionMap;
    for (const auto& opt : options) {
        QAction* act = menu.addAction(opt.label);
        act->setCheckable(true);
        if (currentTag.compare(QLatin1String(opt.tag), Qt::CaseInsensitive) == 0) {
            act->setChecked(true);
        }
        actionMap.insert(act, opt.kind);
    }

    menu.addSeparator();
    QAction* actPlainQuote = menu.addAction(
        T("Convert to Plain Blockquote", "In normales Zitat umwandeln")
    );
    actionMap.insert(actPlainQuote, KMARK_ALERT_NONE);

    QAction* chosen = menu.exec(popupPos);
    if (chosen == nullptr || !actionMap.contains(chosen)) {
        return;
    }

    const uint8_t chosenKind = actionMap.value(chosen);
    kmark_session_set_callout_kind(m_session, hasOffset, openLineStart, chosenKind);
    syncOutlineTree();
    m_viewport->notifyDocumentChanged();
    m_viewport->setFocus();
}

void MainWindow::setFolderRoot(const QString& dirPath) {
    const QFileInfo fi(dirPath);
    const QString cleanDir = QDir::cleanPath(
        fi.exists() && fi.isDir() ? fi.absoluteFilePath() : QDir::rootPath()
    );
    m_folderRootPath = cleanDir;
    m_settings.lastFolderRoot = cleanDir;
    if (m_fsModel != nullptr && m_folderView != nullptr) {
        const QModelIndex rootIdx = m_fsModel->index(cleanDir);
        if (rootIdx.isValid()) {
            m_folderView->setRootIndex(rootIdx);
            (void)m_fsModel->rowCount(rootIdx);
        }
    }
    updateFolderHeaderUi();
}

void MainWindow::updateFolderHeaderUi() {
    if (m_folderPathLabel == nullptr) {
        return;
    }
    const QString display = compactHomePath(m_folderRootPath);
    static_cast<ElidedPathLabel*>(m_folderPathLabel)->setFullText(display);
    m_folderPathLabel->setToolTip(m_folderRootPath);
    if (m_btnFolderUp != nullptr) {
        m_btnFolderUp->setEnabled(!QDir(m_folderRootPath).isRoot());
    }
}

void MainWindow::revealActiveFileInFolderTree() {
    if (m_session == nullptr || m_fsModel == nullptr || m_folderView == nullptr) {
        return;
    }
    const size_t active = kmark_session_active_tab(m_session);
    if (!kmark_session_tab_has_path(m_session, active)) {
        statusBar()->showMessage(
            T("Active tab has not been saved to disk yet.", "Aktives Dokument wurde noch nicht gespeichert."),
            3000
        );
        return;
    }

    const QString rawPath = takeFfiString(kmark_session_tab_path_alloc(m_session, active));
    const QFileInfo fi(rawPath);
    if (!fi.exists()) {
        return;
    }
    const QString targetFile = QDir::cleanPath(fi.absoluteFilePath());

    const auto isInsideDir = [](const QString& file, const QString& dir) {
        if (dir.isEmpty()) {
            return false;
        }
        if (QDir(dir).isRoot()) {
            return true;
        }
        return file.startsWith(dir + QLatin1Char('/'));
    };

    if (!isInsideDir(targetFile, m_folderRootPath)) {
        const QString home = QDir::cleanPath(QDir::homePath());
        if (!home.isEmpty() && isInsideDir(targetFile, home)) {
            setFolderRoot(home);
        } else {
            setFolderRoot(QDir::rootPath());
        }
    }

    m_folderDock->show();
    m_folderDock->raise();

    m_pendingRevealPath = targetFile;
    performPendingRevealStep();

    QTimer::singleShot(300, this, [this, targetFile]() {
        if (m_pendingRevealPath == targetFile) {
            performPendingRevealStep();
            m_pendingRevealPath.clear();
        }
    });
}

void MainWindow::performPendingRevealStep() {
    if (m_pendingRevealPath.isEmpty() || m_fsModel == nullptr || m_folderView == nullptr) {
        return;
    }

    const QString parentDir = QDir::cleanPath(QFileInfo(m_pendingRevealPath).absolutePath());
    QStringList chain;
    QString cur = parentDir;
    while (!cur.isEmpty()) {
        chain.prepend(cur);
        if (cur == m_folderRootPath || QDir(cur).isRoot()) {
            break;
        }
        const QString next = QDir::cleanPath(QFileInfo(cur).absolutePath());
        if (next == cur) {
            break;
        }
        cur = next;
    }

    for (const QString& dirPath : chain) {
        const QModelIndex dirIdx = m_fsModel->index(dirPath);
        if (dirIdx.isValid()) {
            (void)m_fsModel->rowCount(dirIdx);
            if (m_fsModel->canFetchMore(dirIdx)) {
                m_fsModel->fetchMore(dirIdx);
            }
            if (dirPath != m_folderRootPath) {
                m_folderView->expand(dirIdx);
            }
        }
    }

    const QModelIndex fileIdx = m_fsModel->index(m_pendingRevealPath);
    if (fileIdx.isValid()) {
        m_folderView->setCurrentIndex(fileIdx);
        if (auto* sel = m_folderView->selectionModel()) {
            sel->select(fileIdx, QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
        }
        m_folderView->scrollTo(fileIdx, QAbstractItemView::PositionAtCenter);
    }
}

void MainWindow::loadRecentFiles() {
    QSettings settings(QStringLiteral("colorcube"), QStringLiteral("kmark"));
    const QStringList raw = settings.value(QStringLiteral("recentFiles")).toStringList();
    m_recentFiles.clear();
    for (const QString& p : raw) {
        const QString clean = QDir::cleanPath(p.trimmed());
        if (!clean.isEmpty() && !m_recentFiles.contains(clean)) {
            m_recentFiles.append(clean);
            if (m_recentFiles.size() >= kMaxRecentFiles) {
                break;
            }
        }
    }
}

void MainWindow::saveRecentFiles() const {
    QSettings settings(QStringLiteral("colorcube"), QStringLiteral("kmark"));
    settings.setValue(QStringLiteral("recentFiles"), m_recentFiles);
}

void MainWindow::recordRecentFile(const QString& filePath) {
    const QString clean = QDir::cleanPath(QFileInfo(filePath).absoluteFilePath());
    if (clean.isEmpty()) {
        return;
    }
    m_recentFiles.removeAll(clean);
    m_recentFiles.prepend(clean);
    while (m_recentFiles.size() > kMaxRecentFiles) {
        m_recentFiles.removeLast();
    }
    saveRecentFiles();
    rebuildRecentFilesMenu();
}

void MainWindow::rebuildRecentFilesMenu() {
    if (m_recentMenu == nullptr) {
        return;
    }
    m_recentMenu->clear();
    if (m_recentFiles.isEmpty()) {
        QAction* emptyAct = m_recentMenu->addAction(
            T("No Recent Files", "Keine zuletzt geöffneten Dateien")
        );
        emptyAct->setEnabled(false);
        return;
    }

    for (int i = 0; i < m_recentFiles.size(); ++i) {
        const QString path = m_recentFiles.at(i);
        const QFileInfo fi(path);
        QString baseName = fi.fileName();
        baseName.replace(QLatin1Char('&'), QStringLiteral("&&"));
        QString compact = compactHomePath(path);
        compact.replace(QLatin1Char('&'), QStringLiteral("&&"));

        const QString prefix = (i < 9)
            ? QStringLiteral("&%1. ").arg(i + 1)
            : QStringLiteral("%1. ").arg(i + 1);
        const QString label = QStringLiteral("%1%2  [%3]").arg(prefix, baseName, compact);

        QAction* act = m_recentMenu->addAction(
            QIcon::fromTheme(
                QStringLiteral("text-markdown"),
                QIcon::fromTheme(QStringLiteral("text-x-generic"))
            ),
            label
        );
        act->setToolTip(path);
        act->setStatusTip(path);
        connect(act, &QAction::triggered, this, [this, path]() {
            if (!QFileInfo::exists(path)) {
                QMessageBox::warning(
                    this,
                    T("File Not Found", "Datei nicht gefunden"),
                    T(
                        "The file “%1” no longer exists and will be removed from the recent files list.",
                        "Die Datei „%1“ existiert nicht mehr und wird aus der Liste entfernt."
                    ).arg(path)
                );
                m_recentFiles.removeAll(path);
                saveRecentFiles();
                rebuildRecentFilesMenu();
                return;
            }
            openPathFromCli(path);
        });
    }

    m_recentMenu->addSeparator();
    QAction* actClear = m_recentMenu->addAction(
        QIcon::fromTheme(
            QStringLiteral("edit-clear-history"),
            QIcon::fromTheme(QStringLiteral("edit-clear"))
        ),
        T("Clear List", "Liste leeren")
    );
    connect(actClear, &QAction::triggered, this, [this]() {
        m_recentFiles.clear();
        saveRecentFiles();
        rebuildRecentFilesMenu();
    });
}

void MainWindow::applySettingsToRuntime(bool applyDefaultViewMode) {
    if (m_session == nullptr || m_viewport == nullptr) {
        return;
    }
    const KMarkLayoutSettings cur = kmark_session_get_layout_settings(m_session);
    const uint8_t targetMode = applyDefaultViewMode ? m_settings.defaultViewMode : cur.view_mode;
    kmark_session_set_layout_settings(m_session, m_settings.toLayoutSettings(targetMode));
    m_viewport->applyAppearanceSettings(m_settings);
    rebuildPresetsMenu();
}

void MainWindow::syncSettingsFromRuntime(bool persistNow) {
    if (m_session == nullptr) {
        return;
    }
    const KMarkLayoutSettings ls = kmark_session_get_layout_settings(m_session);
    const KMarkToolbarState st = kmark_session_toolbar_state(m_session);
    if (st.view_mode != KMARK_VIEW_PLAIN_TEXT) {
        m_settings.defaultViewMode = st.view_mode;
    }
    m_settings.headingMarks = ls.heading_marks;
    m_settings.wordWrap = ls.word_wrap;
    m_settings.showLineNumbers = ls.show_line_numbers;
    m_settings.focusMode = ls.focus_mode;
    m_settings.typewriterMode = ls.typewriter_mode;
    m_settings.baseFontPx = ls.base_font_px;
    m_settings.lineSpacing = ls.line_spacing;
    m_settings.maxProseWidth = ls.max_prose_width;

    if (!m_settings.activePresetId.isEmpty()) {
        const AppearancePreset* curPreset = m_settings.findPresetById(m_settings.activePresetId);
        if (curPreset == nullptr || !m_settings.matchesAppearanceOf(*curPreset)) {
            m_settings.activePresetId.clear();
        }
    }
    if (m_settings.activePresetId.isEmpty()) {
        for (const AppearancePreset& candidate : m_settings.allPresets()) {
            if (m_settings.matchesAppearanceOf(candidate)) {
                m_settings.activePresetId = candidate.id;
                break;
            }
        }
    }

    rebuildPresetsMenu();
    if (persistNow) {
        m_settings.save();
    }
}

void MainWindow::rebuildPresetsMenu() {
    if (m_presetsMenu == nullptr) {
        return;
    }
    m_presetsMenu->clear();

    auto addPresetEntry = [this](const AppearancePreset& preset) {
        QString label = preset.displayName(m_lang);
        label.replace(QLatin1Char('&'), QStringLiteral("&&"));
        if (!preset.isBuiltin) {
            label = QStringLiteral("★ %1").arg(label);
        }
        QAction* act = m_presetsMenu->addAction(label);
        act->setCheckable(true);
        act->setChecked(m_settings.activePresetId == preset.id);
        const QString desc = preset.displayDescription(m_lang);
        if (!desc.isEmpty()) {
            act->setToolTip(desc);
            act->setStatusTip(desc);
        }
        const QString pid = preset.id;
        connect(act, &QAction::triggered, this, [this, pid]() {
            if (const AppearancePreset* p = m_settings.findPresetById(pid)) {
                m_settings.applyPreset(*p);
                applySettingsToRuntime(false);
                m_settings.save();
                statusBar()->showMessage(
                    T("Preset “%1” activated.", "Preset „%1“ aktiviert.").arg(p->displayName(m_lang)),
                    3000
                );
            }
        });
    };

    for (const AppearancePreset& bp : builtinPresets()) {
        addPresetEntry(bp);
    }

    if (!m_settings.userPresets.isEmpty()) {
        m_presetsMenu->addSeparator();
        for (const AppearancePreset& up : m_settings.userPresets) {
            addPresetEntry(up);
        }
    }

    if (m_settings.activePresetId.isEmpty()) {
        m_presetsMenu->addSeparator();
        QAction* customAct = m_presetsMenu->addAction(
            T("Custom (unsaved)", "Benutzerdefiniert (ungespeichert)")
        );
        customAct->setCheckable(true);
        customAct->setChecked(true);
        customAct->setEnabled(false);
    }

    m_presetsMenu->addSeparator();
    QAction* actSavePreset = m_presetsMenu->addAction(
        QIcon::fromTheme(QStringLiteral("document-save-as")),
        T("Save Current as Preset…", "Aktuelles Design als Preset speichern…")
    );
    connect(actSavePreset, &QAction::triggered, this, &MainWindow::promptSaveQuickPreset);

    QAction* actOpenDlg = m_presetsMenu->addAction(
        QIcon::fromTheme(QStringLiteral("configure")),
        T("Manage Presets && Appearance…", "Presets && Darstellung einrichten…")
    );
    connect(actOpenDlg, &QAction::triggered, this, &MainWindow::openSettingsDialog);
}

void MainWindow::promptSaveQuickPreset() {
    syncSettingsFromRuntime(false);

    bool ok = false;
    const QString rawName = QInputDialog::getText(
        this,
        T("Save Appearance Preset", "Design-Preset speichern"),
        T("Preset name:", "Name des Presets:"),
        QLineEdit::Normal,
        QString(),
        &ok
    );
    const QString name = rawName.trimmed();
    if (!ok || name.isEmpty()) {
        return;
    }

    for (AppearancePreset& existing : m_settings.userPresets) {
        if (existing.nameDe.compare(name, Qt::CaseInsensitive) == 0
            || existing.nameEn.compare(name, Qt::CaseInsensitive) == 0) {
            existing = m_settings.captureAsPreset(existing.id, name);
            m_settings.activePresetId = existing.id;
            rebuildPresetsMenu();
            m_settings.save();
            statusBar()->showMessage(
                T("Preset “%1” updated.", "Preset „%1“ aktualisiert.").arg(name),
                3500
            );
            return;
        }
    }

    const QString newId = QStringLiteral("user:%1").arg(
        QUuid::createUuid().toString(QUuid::WithoutBraces)
    );
    const AppearancePreset created = m_settings.captureAsPreset(newId, name);
    m_settings.userPresets.append(created);
    m_settings.activePresetId = created.id;
    rebuildPresetsMenu();
    m_settings.save();
    statusBar()->showMessage(
        T("Preset “%1” saved and added to menu.", "Preset „%1“ gespeichert und im Menü verfügbar.").arg(name),
        3500
    );
}

void MainWindow::openSettingsDialog() {
    syncSettingsFromRuntime(false);

    SettingsDialog dlg(m_settings, m_lang, this);
    connect(&dlg, &SettingsDialog::settingsApplied, this, [this](const AppSettings& updated) {
        m_settings = updated;
        applySettingsToRuntime(true);
        m_settings.save();
    });

    if (dlg.exec() == QDialog::Accepted) {
        m_settings = dlg.resultSettings();
        applySettingsToRuntime(true);
        m_settings.save();
        statusBar()->showMessage(T("Settings saved.", "Einstellungen gespeichert."), 3000);
    }
}

void MainWindow::refreshToolbarIcons() {
    const QPalette pal = palette();
    const int lightness = pal.color(QPalette::WindowText).lightness();
    if (m_lastIconLightness == lightness) {
        return;
    }
    m_lastIconLightness = lightness;

    if (m_btnFolderUp != nullptr) {
        m_btnFolderUp->setIcon(makeFolderUpIcon(pal));
    }
    if (m_btnFolderHome != nullptr) {
        m_btnFolderHome->setIcon(makeFolderHomeIcon(pal));
    }
    if (m_btnRevealActiveFile != nullptr) {
        m_btnRevealActiveFile->setIcon(makeSelectOpenedFileIcon(pal));
    }

    auto setCore = [&](QAction* act, CoreIconKind kind) {
        if (act != nullptr) {
            act->setIcon(makeCoreToolbarIcon(kind, pal));
        }
    };
    setCore(m_actNew, CoreIconKind::NewDoc);
    setCore(m_actOpen, CoreIconKind::OpenDoc);
    setCore(m_actSave, CoreIconKind::SaveDoc);
    setCore(m_actUndo, CoreIconKind::Undo);
    setCore(m_actRedo, CoreIconKind::Redo);
    setCore(m_actBold, CoreIconKind::Bold);
    setCore(m_actItalic, CoreIconKind::Italic);
    setCore(m_actStrike, CoreIconKind::Strike);
    setCore(m_actHighlight, CoreIconKind::Highlight);
    setCore(m_actInlineCode, CoreIconKind::InlineCode);
    setCore(m_actBullet, CoreIconKind::BulletList);
    setCore(m_actOrdered, CoreIconKind::OrderedList);
    setCore(m_actTask, CoreIconKind::TaskList);
    setCore(m_actQuote, CoreIconKind::Blockquote);
    setCore(m_actLink, CoreIconKind::Link);
    setCore(m_actImage, CoreIconKind::Image);
    setCore(m_actFence, CoreIconKind::CodeFence);

    if (m_actInsTable != nullptr) {
        m_actInsTable->setIcon(makeTableActionIcon(TableIconKind::InsertTable, pal));
    }
    if (m_actRowAbove != nullptr) {
        m_actRowAbove->setIcon(makeTableActionIcon(TableIconKind::RowAbove, pal));
    }
    if (m_actRowBelow != nullptr) {
        m_actRowBelow->setIcon(makeTableActionIcon(TableIconKind::RowBelow, pal));
    }
    if (m_actRowDel != nullptr) {
        m_actRowDel->setIcon(makeTableActionIcon(TableIconKind::RowDelete, pal));
    }
    if (m_actColLeft != nullptr) {
        m_actColLeft->setIcon(makeTableActionIcon(TableIconKind::ColLeft, pal));
    }
    if (m_actColRight != nullptr) {
        m_actColRight->setIcon(makeTableActionIcon(TableIconKind::ColRight, pal));
    }
    if (m_actColDel != nullptr) {
        m_actColDel->setIcon(makeTableActionIcon(TableIconKind::ColDelete, pal));
    }
    if (m_actAlignLeft != nullptr) {
        m_actAlignLeft->setIcon(makeTableActionIcon(TableIconKind::AlignLeft, pal));
    }
    if (m_actAlignCenter != nullptr) {
        m_actAlignCenter->setIcon(makeTableActionIcon(TableIconKind::AlignCenter, pal));
    }
    if (m_actAlignRight != nullptr) {
        m_actAlignRight->setIcon(makeTableActionIcon(TableIconKind::AlignRight, pal));
    }
}

} // namespace kmark
