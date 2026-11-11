#include "EditorViewport.h"

#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QClipboard>
#include <QContextMenuEvent>
#include <QDateTime>
#include <QDesktopServices>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QFileDialog>
#include <QFileInfo>
#include <QFontDatabase>
#include <QFontMetricsF>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QIcon>
#include <QImage>
#include <QImageReader>
#include <QInputDialog>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QLineEdit>
#include <QMenu>
#include <QMimeData>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPushButton>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QStandardPaths>
#include <QStyle>
#include <QStyleOptionButton>
#include <QUrl>
#include <QWheelEvent>
#include <algorithm>
#include <cmath>

#include "PaletteBridge.h"

namespace kmark {

namespace {

bool hasImageExtension(const QString& pathOrUrl) {
    const QString lower = pathOrUrl.trimmed().toLower();
    static const QStringList exts = {
        QStringLiteral(".png"),
        QStringLiteral(".jpg"),
        QStringLiteral(".jpeg"),
        QStringLiteral(".svg"),
        QStringLiteral(".webp"),
        QStringLiteral(".gif"),
        QStringLiteral(".bmp"),
        QStringLiteral(".avif"),
    };
    for (const QString& ext : exts) {
        if (lower.endsWith(ext)) {
            return true;
        }
    }
    return false;
}

bool hasDocumentExtension(const QString& path) {
    const QString lower = path.trimmed().toLower();
    static const QStringList exts = {
        QStringLiteral(".md"),
        QStringLiteral(".markdown"),
        QStringLiteral(".mdown"),
        QStringLiteral(".txt"),
        QStringLiteral(".rst"),
        QStringLiteral(".toml"),
        QStringLiteral(".json"),
        QStringLiteral(".yaml"),
        QStringLiteral(".yml"),
    };
    for (const QString& ext : exts) {
        if (lower.endsWith(ext)) {
            return true;
        }
    }
    return false;
}

} // namespace

EditorViewport::EditorViewport(KMarkSession* session, QWidget* parent)
    : QAbstractScrollArea(parent)
    , m_session(session) {
    setFocusPolicy(Qt::StrongFocus);
    setAttribute(Qt::WA_InputMethodEnabled, true);
    setFrameShape(QFrame::NoFrame);
    setAcceptDrops(true);
    viewport()->setAcceptDrops(true);
    viewport()->setMouseTracking(true);
    viewport()->setCursor(Qt::IBeamCursor);

    refreshSystemFonts();
    refreshPalette();

    kmark_session_set_measurer(
        m_session,
        this,
        &EditorViewport::metricsCallback,
        &EditorViewport::widthCallback
    );
    kmark_session_set_image_measurer(
        m_session,
        this,
        &EditorViewport::imageSizeCallback
    );

    connect(verticalScrollBar(), &QScrollBar::valueChanged, this, [this](int value) {
        if (m_session != nullptr) {
            kmark_session_set_scroll_y(m_session, static_cast<float>(value));
            viewport()->update();
        }
    });

    m_caretTimer.setInterval(530);
    connect(&m_caretTimer, &QTimer::timeout, this, [this]() {
        m_caretVisible = !m_caretVisible;
        viewport()->update(m_lastCaretRect.adjusted(-3.0, -3.0, 4.0, 4.0).toAlignedRect());
    });
    m_caretTimer.start();
}

EditorViewport::~EditorViewport() {
    if (m_session != nullptr) {
        kmark_session_set_measurer(m_session, nullptr, nullptr, nullptr);
        kmark_session_set_image_measurer(m_session, nullptr, nullptr);
    }
}

void EditorViewport::applyAppearanceSettings(const AppSettings& settings) {
    m_useSystemSans = settings.useSystemSans;
    m_useSystemMono = settings.useSystemMono;
    m_useSystemColors = settings.useSystemColors;
    m_sansBase = settings.effectiveSansFont();
    m_monoBase = settings.effectiveMonoFont();
    m_customPalette = settings.effectivePalette(QApplication::palette());

    if (m_session != nullptr) {
        kmark_session_set_palette(m_session, m_customPalette);
        kmark_session_clear_measure_cache(m_session);
    }
    viewport()->update();
}

void EditorViewport::refreshSystemFonts() {
    if (m_useSystemSans) {
        m_sansBase = QFontDatabase::systemFont(QFontDatabase::GeneralFont);
    }
    if (m_useSystemMono) {
        m_monoBase = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    }
    if (m_session != nullptr) {
        kmark_session_clear_measure_cache(m_session);
    }
    viewport()->update();
}

void EditorViewport::refreshPalette() {
    if (m_session != nullptr) {
        const KMarkPalette pal = m_useSystemColors
            ? paletteFromQt(QApplication::palette())
            : m_customPalette;
        kmark_session_set_palette(m_session, pal);
    }
    viewport()->update();
}

void EditorViewport::clearImageCache() {
    m_pixmapCache.clear();
    if (m_session != nullptr) {
        kmark_session_clear_measure_cache(m_session);
    }
    viewport()->update();
}

void EditorViewport::notifyDocumentChanged() {
    resetCaretBlink();
    viewport()->update();
    emit stateChanged();
}

QFont EditorViewport::makeFont(uint8_t fontRole, float sizePx, bool bold, bool italic) const {
    QFont f = (fontRole == KMARK_FONT_MONO) ? m_monoBase : m_sansBase;
    f.setPixelSize(std::max(8, static_cast<int>(std::lround(sizePx))));
    f.setBold(bold);
    f.setItalic(italic);
    return f;
}

QString EditorViewport::activeDocumentDir() const {
    if (m_session == nullptr) {
        return QDir::currentPath();
    }
    const size_t active = kmark_session_active_tab(m_session);
    char* raw = kmark_session_tab_path_alloc(m_session, active);
    if (raw != nullptr) {
        const QString fullPath = QString::fromUtf8(raw);
        kmark_string_free(raw);
        if (!fullPath.isEmpty()) {
            return QFileInfo(fullPath).absolutePath();
        }
    }
    return QDir::currentPath();
}

QString EditorViewport::resolveImagePath(const QString& rawSrc) const {
    QString src = rawSrc.trimmed();
    if (src.startsWith(QLatin1Char('<')) && src.endsWith(QLatin1Char('>')) && src.size() >= 2) {
        src = src.mid(1, src.size() - 2).trimmed();
    }
    /* Strip optional Markdown title (`path/img.png "Title"`) */
    const qsizetype quoteIdx = src.indexOf(QStringLiteral(" \""));
    if (quoteIdx > 0) {
        src = src.left(quoteIdx).trimmed();
    }
    if (src.startsWith(QStringLiteral("http://"), Qt::CaseInsensitive)
        || src.startsWith(QStringLiteral("https://"), Qt::CaseInsensitive)
        || src.startsWith(QStringLiteral("data:"), Qt::CaseInsensitive)) {
        return {};
    }
    if (src.startsWith(QStringLiteral("file://"), Qt::CaseInsensitive)) {
        return QUrl(src).toLocalFile();
    }
    const QFileInfo fi(src);
    if (fi.isAbsolute()) {
        return fi.exists() ? fi.absoluteFilePath() : QString();
    }
    const QDir docDir(activeDocumentDir());
    const QString candidate = docDir.filePath(src);
    if (QFileInfo::exists(candidate)) {
        return QFileInfo(candidate).absoluteFilePath();
    }
    const QString cwdCandidate = QDir::current().filePath(src);
    if (QFileInfo::exists(cwdCandidate)) {
        return QFileInfo(cwdCandidate).absoluteFilePath();
    }
    return {};
}

QString EditorViewport::makeFriendlyMarkdownPath(const QString& absPath) const {
    if (m_session != nullptr) {
        const size_t active = kmark_session_active_tab(m_session);
        if (kmark_session_tab_has_path(m_session, active)) {
            const QDir baseDir(activeDocumentDir());
            const QString rel = QDir::cleanPath(baseDir.relativeFilePath(absPath));
            if (!rel.startsWith(QStringLiteral("../../../"))) {
                return rel;
            }
        }
    }
    return QDir::fromNativeSeparators(absPath);
}

void EditorViewport::metricsCallback(
    void* userData,
    uint8_t fontRole,
    float sizePx,
    bool bold,
    bool italic,
    KMarkFontMetrics* outMetrics
) {
    if (userData == nullptr || outMetrics == nullptr) {
        return;
    }
    const auto* self = static_cast<const EditorViewport*>(userData);
    const QFont font = self->makeFont(fontRole, sizePx, bold, italic);
    const QFontMetricsF fm(font);
    outMetrics->ascent = static_cast<float>(fm.ascent());
    outMetrics->descent = static_cast<float>(fm.descent());
    outMetrics->line_height = static_cast<float>(fm.height());
    outMetrics->space_width = static_cast<float>(fm.horizontalAdvance(QLatin1Char(' ')));
}

float EditorViewport::widthCallback(
    void* userData,
    const char* utf8Ptr,
    size_t utf8Len,
    uint8_t fontRole,
    float sizePx,
    bool bold,
    bool italic
) {
    if (userData == nullptr || utf8Ptr == nullptr || utf8Len == 0) {
        return 0.0f;
    }
    const auto* self = static_cast<const EditorViewport*>(userData);
    const QFont font = self->makeFont(fontRole, sizePx, bold, italic);
    const QFontMetricsF fm(font);
    const QString text = QString::fromUtf8(utf8Ptr, static_cast<qsizetype>(utf8Len));
    return static_cast<float>(fm.horizontalAdvance(text));
}

bool EditorViewport::imageSizeCallback(
    void* userData,
    const char* srcUtf8,
    size_t srcLen,
    float* outW,
    float* outH
) {
    if (userData == nullptr || srcUtf8 == nullptr || srcLen == 0 || outW == nullptr || outH == nullptr) {
        return false;
    }
    const auto* self = static_cast<const EditorViewport*>(userData);
    const QString rawSrc = QString::fromUtf8(srcUtf8, static_cast<qsizetype>(srcLen));
    const QString resolved = self->resolveImagePath(rawSrc);
    if (resolved.isEmpty()) {
        return false;
    }
    QImageReader reader(resolved);
    reader.setAutoTransform(true);
    const QSize sz = reader.size();
    if (!sz.isValid() || sz.width() <= 0 || sz.height() <= 0) {
        const QPixmap pm(resolved);
        if (pm.isNull()) {
            return false;
        }
        *outW = static_cast<float>(pm.width());
        *outH = static_cast<float>(pm.height());
        return true;
    }
    *outW = static_cast<float>(sz.width());
    *outH = static_cast<float>(sz.height());
    return true;
}

void EditorViewport::resetCaretBlink() {
    m_caretVisible = true;
    if (hasFocus()) {
        m_caretTimer.start();
    }
}

void EditorViewport::paintEvent(QPaintEvent* /*event*/) {
    if (m_session == nullptr) {
        return;
    }

    const float vw = static_cast<float>(viewport()->width());
    const float vh = static_cast<float>(viewport()->height());
    const KMarkFrameView frame = kmark_session_render_frame(m_session, vw, vh);

    {
        const QSignalBlocker blocker(verticalScrollBar());
        const int maxScroll = std::max(0, static_cast<int>(std::ceil(frame.total_height - vh * 0.6f)));
        verticalScrollBar()->setRange(0, maxScroll);
        verticalScrollBar()->setPageStep(static_cast<int>(vh));
        verticalScrollBar()->setSingleStep(24);
        verticalScrollBar()->setValue(static_cast<int>(std::lround(frame.scroll_y)));
    }

    m_lastCaretRect = QRectF(
        frame.caret_rect.x,
        frame.caret_rect.y,
        std::max(2.0f, frame.caret_rect.w),
        std::max(14.0f, frame.caret_rect.h)
    );

    QPainter painter(viewport());
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::TextAntialiasing, true);

    for (size_t i = 0; i < frame.cmds_len; ++i) {
        const KMarkDrawCmd& cmd = frame.cmds_ptr[i];
        switch (cmd.tag) {
            case KMARK_CMD_FILL_RECT: {
                const QRectF r(cmd.rect.x, cmd.rect.y, cmd.rect.w, cmd.rect.h);
                painter.setPen(Qt::NoPen);
                painter.setBrush(toQColor(cmd.color));
                if (cmd.aux > 0.2f) {
                    painter.drawRoundedRect(r, cmd.aux, cmd.aux);
                } else {
                    painter.fillRect(r, toQColor(cmd.color));
                }
                break;
            }
            case KMARK_CMD_STROKE_RECT: {
                const QRectF r(cmd.rect.x, cmd.rect.y, cmd.rect.w, cmd.rect.h);
                QPen pen(toQColor(cmd.color));
                pen.setWidthF(std::max(1.0f, cmd.aux));
                painter.setPen(pen);
                painter.setBrush(Qt::NoBrush);
                const qreal rad = cmd.level > 0 ? static_cast<qreal>(cmd.level) * 0.5 : 3.0;
                painter.drawRoundedRect(r, rad, rad);
                break;
            }
            case KMARK_CMD_LINE: {
                QPen pen(toQColor(cmd.color));
                pen.setWidthF(std::max(1.0f, cmd.aux));
                painter.setPen(pen);
                painter.drawLine(QPointF(cmd.rect.x, cmd.rect.y), QPointF(cmd.rect.w, cmd.rect.h));
                break;
            }
            case KMARK_CMD_HEADING_BARS: {
                /* Signature slanted H1..H6 bars inspired by MDNote */
                const int count = std::clamp(static_cast<int>(cmd.level), 1, 6);
                const qreal barW = 2.6;
                const qreal gap = 2.4;
                const qreal slant = 3.2;
                const qreal totalW = count * barW + (count - 1) * gap;
                qreal curX = static_cast<qreal>(cmd.rect.x) - totalW;
                const qreal topY = static_cast<qreal>(cmd.rect.y);
                const qreal botY = topY + static_cast<qreal>(cmd.rect.h);

                painter.setPen(Qt::NoPen);
                painter.setBrush(toQColor(cmd.color));
                for (int b = 0; b < count; ++b) {
                    QPainterPath path;
                    path.moveTo(curX + slant, topY);
                    path.lineTo(curX + slant + barW, topY);
                    path.lineTo(curX + barW, botY);
                    path.lineTo(curX, botY);
                    path.closeSubpath();
                    painter.drawPath(path);
                    curX += barW + gap;
                }
                break;
            }
            case KMARK_CMD_CHECKBOX: {
                QStyleOptionButton opt;
                opt.initFrom(this);
                opt.rect = QRectF(cmd.rect.x, cmd.rect.y, cmd.rect.w, cmd.rect.h).toAlignedRect();
                opt.state = QStyle::State_Enabled | ((cmd.flags & 4u) != 0 ? QStyle::State_On : QStyle::State_Off);
                style()->drawPrimitive(QStyle::PE_IndicatorCheckBox, &opt, &painter, this);
                break;
            }
            case KMARK_CMD_TEXT: {
                if (cmd.text_len == 0 || frame.text_pool_ptr == nullptr) {
                    break;
                }
                const auto* start = reinterpret_cast<const char*>(frame.text_pool_ptr + cmd.text_offset);
                const QString str = QString::fromUtf8(start, static_cast<qsizetype>(cmd.text_len));
                const bool bold = (cmd.flags & 1u) != 0;
                const bool italic = (cmd.flags & 2u) != 0;
                m_lastFontPx = cmd.aux;
                painter.setFont(makeFont(cmd.font_role, cmd.aux, bold, italic));
                painter.setPen(toQColor(cmd.color));
                painter.drawText(QPointF(cmd.rect.x, cmd.rect.y), str);
                break;
            }
            case KMARK_CMD_IMAGE: {
                if (cmd.text_len == 0 || frame.text_pool_ptr == nullptr) {
                    break;
                }
                const auto* start = reinterpret_cast<const char*>(frame.text_pool_ptr + cmd.text_offset);
                const QString rawSrc = QString::fromUtf8(start, static_cast<qsizetype>(cmd.text_len));
                const QString resolved = resolveImagePath(rawSrc);
                if (resolved.isEmpty()) {
                    break;
                }
                auto it = m_pixmapCache.find(resolved);
                if (it == m_pixmapCache.end()) {
                    if (m_pixmapCache.size() >= 64) {
                        m_pixmapCache.clear();
                    }
                    QImageReader reader(resolved);
                    reader.setAutoTransform(true);
                    const QImage img = reader.read();
                    it = m_pixmapCache.insert(resolved, img.isNull() ? QPixmap(resolved) : QPixmap::fromImage(img));
                }
                if (!it->isNull()) {
                    const QRectF dest(cmd.rect.x, cmd.rect.y, cmd.rect.w, cmd.rect.h);
                    painter.save();
                    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
                    QPainterPath clip;
                    clip.addRoundedRect(dest, 4.0, 4.0);
                    painter.setClipPath(clip);
                    painter.drawPixmap(dest, *it, QRectF(0, 0, it->width(), it->height()));
                    painter.restore();
                }
                break;
            }
            default:
                break;
        }
    }

    if (hasFocus() && m_caretVisible) {
        painter.fillRect(m_lastCaretRect, QApplication::palette().color(QPalette::Text));
    }
}

void EditorViewport::resizeEvent(QResizeEvent* event) {
    QAbstractScrollArea::resizeEvent(event);
    viewport()->update();
}

void EditorViewport::copySelectionToClipboard() {
    if (m_session == nullptr) {
        return;
    }
    char* raw = kmark_session_selected_text_alloc(m_session);
    if (raw != nullptr) {
        const QString sel = QString::fromUtf8(raw);
        kmark_string_free(raw);
        if (!sel.isEmpty()) {
            QApplication::clipboard()->setText(sel);
        }
    }
}

void EditorViewport::cutSelectionToClipboard() {
    copySelectionToClipboard();
    if (m_session != nullptr) {
        kmark_session_key_press(m_session, KMARK_KEY_BACKSPACE, false, false, false);
        notifyDocumentChanged();
    }
}

void EditorViewport::pasteFromClipboard() {
    if (m_session == nullptr) {
        return;
    }
    const QMimeData* mime = QApplication::clipboard()->mimeData();
    if (mime != nullptr && mime->hasImage() && !mime->hasText()) {
        const QImage img = qvariant_cast<QImage>(mime->imageData());
        if (!img.isNull()) {
            QString outDir = activeDocumentDir();
            const size_t active = kmark_session_active_tab(m_session);
            if (!kmark_session_tab_has_path(m_session, active)) {
                outDir = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
                if (outDir.isEmpty()) {
                    outDir = QDir::tempPath();
                }
                outDir += QStringLiteral("/kmark");
            }
            QDir().mkpath(outDir);
            const QString fileName = QStringLiteral("pasted-%1.png")
                .arg(QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss")));
            const QString fullPath = QDir(outDir).filePath(fileName);
            if (img.save(fullPath, "PNG")) {
                const QString mdPath = makeFriendlyMarkdownPath(fullPath);
                const QByteArray altUtf8 = QByteArrayLiteral("Screenshot");
                const QByteArray urlUtf8 = mdPath.toUtf8();
                kmark_session_insert_image(m_session, altUtf8.constData(), urlUtf8.constData());
                clearImageCache();
                notifyDocumentChanged();
                return;
            }
        }
    }
    const QString text = QApplication::clipboard()->text();
    if (!text.isEmpty()) {
        const QByteArray utf8 = text.toUtf8();
        kmark_session_insert_text(m_session, utf8.constData());
        notifyDocumentChanged();
    }
}

bool EditorViewport::event(QEvent* ev) {
    /* Intercept Tab / Backtab inside the editor so focus doesn't jump out of tables/lists! */
    if (ev->type() == QEvent::KeyPress) {
        auto* ke = static_cast<QKeyEvent*>(ev);
        if (ke->key() == Qt::Key_Tab || ke->key() == Qt::Key_Backtab) {
            keyPressEvent(ke);
            return true;
        }
    }
    if (ev->type() == QEvent::ApplicationPaletteChange || ev->type() == QEvent::PaletteChange) {
        refreshPalette();
    }
    if (ev->type() == QEvent::ApplicationFontChange) {
        refreshSystemFonts();
    }
    return QAbstractScrollArea::event(ev);
}

void EditorViewport::keyPressEvent(QKeyEvent* event) {
    if (m_session == nullptr) {
        return;
    }

    const Qt::KeyboardModifiers mods = event->modifiers();
    const bool shift = mods.testFlag(Qt::ShiftModifier);
    const bool ctrl = mods.testFlag(Qt::ControlModifier);
    const bool alt = mods.testFlag(Qt::AltModifier);

    if (event->matches(QKeySequence::Copy)) {
        copySelectionToClipboard();
        return;
    }
    if (event->matches(QKeySequence::Cut)) {
        cutSelectionToClipboard();
        return;
    }
    if (event->matches(QKeySequence::Paste)) {
        pasteFromClipboard();
        return;
    }
    if (ctrl && (event->key() == Qt::Key_H)) {
        /* Support Ctrl+H as alias for KDE's Ctrl+R Replace bar */
        emit findRequested(true);
        return;
    }

    int mappedKey = -1;
    switch (event->key()) {
        case Qt::Key_Left:      mappedKey = KMARK_KEY_LEFT; break;
        case Qt::Key_Right:     mappedKey = KMARK_KEY_RIGHT; break;
        case Qt::Key_Up:        mappedKey = KMARK_KEY_UP; break;
        case Qt::Key_Down:      mappedKey = KMARK_KEY_DOWN; break;
        case Qt::Key_Home:      mappedKey = KMARK_KEY_HOME; break;
        case Qt::Key_End:       mappedKey = KMARK_KEY_END; break;
        case Qt::Key_PageUp:    mappedKey = KMARK_KEY_PAGE_UP; break;
        case Qt::Key_PageDown:  mappedKey = KMARK_KEY_PAGE_DOWN; break;
        case Qt::Key_Backspace: mappedKey = KMARK_KEY_BACKSPACE; break;
        case Qt::Key_Delete:    mappedKey = KMARK_KEY_DELETE; break;
        case Qt::Key_Return:
        case Qt::Key_Enter:     mappedKey = KMARK_KEY_ENTER; break;
        case Qt::Key_Tab:       mappedKey = KMARK_KEY_TAB; break;
        case Qt::Key_Backtab:   mappedKey = KMARK_KEY_BACKTAB; break;
        case Qt::Key_Escape:    mappedKey = KMARK_KEY_ESCAPE; break;
        default: break;
    }

    if (mappedKey >= 0) {
        kmark_session_key_press(m_session, static_cast<uint32_t>(mappedKey), shift, ctrl, alt);
        notifyDocumentChanged();
        return;
    }

    const QString text = event->text();
    if (!ctrl && !alt && !text.isEmpty() && text.at(0).isPrint()) {
        const QByteArray utf8 = text.toUtf8();
        kmark_session_insert_text(m_session, utf8.constData());
        notifyDocumentChanged();
        return;
    }

    QAbstractScrollArea::keyPressEvent(event);
}

void EditorViewport::inputMethodEvent(QInputMethodEvent* event) {
    if (m_session == nullptr) {
        return;
    }

    const QString commit = event->commitString();
    const QString preedit = event->preeditString();

    if (!commit.isEmpty()) {
        const QByteArray utf8 = commit.toUtf8();
        kmark_session_insert_text(m_session, utf8.constData());
    }

    if (preedit.isEmpty()) {
        kmark_session_set_ime_preedit(m_session, nullptr);
    } else {
        const QByteArray preUtf8 = preedit.toUtf8();
        kmark_session_set_ime_preedit(m_session, preUtf8.constData());
    }

    notifyDocumentChanged();
    event->accept();
}

QVariant EditorViewport::inputMethodQuery(Qt::InputMethodQuery query) const {
    switch (query) {
        case Qt::ImCursorRectangle:
            return m_lastCaretRect.toAlignedRect();
        case Qt::ImFont:
            return makeFont(KMARK_FONT_SANS, m_lastFontPx, false, false);
        default:
            return QAbstractScrollArea::inputMethodQuery(query);
    }
}

void EditorViewport::mousePressEvent(QMouseEvent* event) {
    if (m_session == nullptr || event->button() != Qt::LeftButton) {
        QAbstractScrollArea::mousePressEvent(event);
        return;
    }
    setFocus(Qt::MouseFocusReason);
    const QPointF pos = event->position();
    const Qt::KeyboardModifiers mods = event->modifiers();
    const KMarkPointerResult res = kmark_session_pointer_press(
        m_session,
        static_cast<float>(pos.x()),
        static_cast<float>(pos.y()),
        1,
        mods.testFlag(Qt::ShiftModifier),
        mods.testFlag(Qt::ControlModifier),
        mods.testFlag(Qt::AltModifier)
    );

    QString payload;
    if (res.str_alloc != nullptr) {
        payload = QString::fromUtf8(res.str_alloc);
        kmark_string_free(res.str_alloc);
    }

    if (res.tag == KMARK_PTR_OPEN_URL && !payload.isEmpty()) {
        const QString localResolved = resolveImagePath(payload);
        if (!localResolved.isEmpty()) {
            QDesktopServices::openUrl(QUrl::fromLocalFile(localResolved));
        } else {
            QDesktopServices::openUrl(QUrl::fromUserInput(payload));
        }
    } else if (res.tag == KMARK_PTR_EDIT_CODE_LANGUAGE) {
        emit codeLanguageBadgeClicked(res.byte_offset, payload, event->globalPosition().toPoint());
        return;
    } else if (res.tag == KMARK_PTR_EDIT_CALLOUT_KIND) {
        emit calloutBadgeClicked(res.byte_offset, payload, event->globalPosition().toPoint());
        return;
    }
    notifyDocumentChanged();
}

void EditorViewport::mouseDoubleClickEvent(QMouseEvent* event) {
    if (m_session == nullptr || event->button() != Qt::LeftButton) {
        QAbstractScrollArea::mouseDoubleClickEvent(event);
        return;
    }
    const QPointF pos = event->position();
    const Qt::KeyboardModifiers mods = event->modifiers();
    const KMarkPointerResult res = kmark_session_pointer_press(
        m_session,
        static_cast<float>(pos.x()),
        static_cast<float>(pos.y()),
        2,
        mods.testFlag(Qt::ShiftModifier),
        mods.testFlag(Qt::ControlModifier),
        mods.testFlag(Qt::AltModifier)
    );
    if (res.str_alloc != nullptr) {
        kmark_string_free(res.str_alloc);
    }
    notifyDocumentChanged();
}

void EditorViewport::mouseMoveEvent(QMouseEvent* event) {
    if (m_session != nullptr) {
        const QPointF pos = event->position();
        if (event->buttons().testFlag(Qt::LeftButton)) {
            kmark_session_pointer_drag(m_session, static_cast<float>(pos.x()), static_cast<float>(pos.y()));
            notifyDocumentChanged();
        } else {
            const bool ctrl = event->modifiers().testFlag(Qt::ControlModifier);
            const bool interactive = kmark_session_hover_interactive(
                m_session,
                static_cast<float>(pos.x()),
                static_cast<float>(pos.y()),
                ctrl
            );
            viewport()->setCursor(interactive ? Qt::PointingHandCursor : Qt::IBeamCursor);
        }
    }
    QAbstractScrollArea::mouseMoveEvent(event);
}

UiLang EditorViewport::currentUiLang() const {
    if (m_session != nullptr && kmark_session_ui_language(m_session) == KMARK_LANG_GERMAN) {
        return UiLang::German;
    }
    return detectSystemLanguage();
}

void EditorViewport::applyImageUpdate(
    size_t startByte,
    size_t endByte,
    const QString& alt,
    const QString& src,
    const QString& caption,
    uint8_t sizeKind,
    uint16_t sizeValue
) {
    if (m_session == nullptr) {
        return;
    }
    const QByteArray altUtf8 = alt.toUtf8();
    const QByteArray srcUtf8 = src.toUtf8();
    const QByteArray capUtf8 = caption.trimmed().toUtf8();
    kmark_session_update_image(
        m_session,
        startByte,
        endByte,
        altUtf8.constData(),
        srcUtf8.constData(),
        capUtf8.isEmpty() ? nullptr : capUtf8.constData(),
        sizeKind,
        sizeValue
    );
    clearImageCache();
    notifyDocumentChanged();
}

void EditorViewport::promptImageCaption(
    size_t startByte,
    size_t endByte,
    const QString& alt,
    const QString& src,
    const QString& currentCaption,
    uint8_t sizeKind,
    uint16_t sizeValue
) {
    const UiLang lang = currentUiLang();
    QDialog dlg(this);
    dlg.setWindowTitle(
        currentCaption.isEmpty()
            ? trBi(lang, "Add Image Caption", "Bildunterschrift hinzufügen")
            : trBi(lang, "Edit Image Caption", "Bildunterschrift bearbeiten")
    );
    auto* form = new QFormLayout(&dlg);
    auto* capEdit = new QLineEdit(currentCaption, &dlg);
    capEdit->setMinimumWidth(360);
    capEdit->setClearButtonEnabled(true);
    capEdit->setPlaceholderText(
        trBi(lang, "e.g. Figure 1: Architecture overview", "z. B. Abbildung 1: Architekturübersicht")
    );
    capEdit->selectAll();
    form->addRow(trBi(lang, "Caption:", "Bildunterschrift:"), capEdit);

    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);
    form->addRow(buttons);

    if (dlg.exec() == QDialog::Accepted) {
        applyImageUpdate(startByte, endByte, alt, src, capEdit->text(), sizeKind, sizeValue);
    }
}

void EditorViewport::promptReplaceImage(
    size_t startByte,
    size_t endByte,
    const QString& currentAlt,
    const QString& currentSrc,
    const QString& caption,
    uint8_t sizeKind,
    uint16_t sizeValue
) {
    const UiLang lang = currentUiLang();
    QDialog dlg(this);
    dlg.setWindowTitle(trBi(lang, "Replace Image", "Bild ersetzen"));
    auto* form = new QFormLayout(&dlg);

    auto* pathRow = new QHBoxLayout();
    auto* urlEdit = new QLineEdit(currentSrc, &dlg);
    urlEdit->setMinimumWidth(320);
    auto* browseBtn = new QPushButton(trBi(lang, "Browse…", "Durchsuchen…"), &dlg);
    pathRow->addWidget(urlEdit, 1);
    pathRow->addWidget(browseBtn);

    auto* altEdit = new QLineEdit(currentAlt, &dlg);
    const QString prevStem = QFileInfo(currentSrc).completeBaseName();

    connect(browseBtn, &QPushButton::clicked, &dlg, [this, &dlg, lang, urlEdit, altEdit, prevStem]() {
        const QString f = QFileDialog::getOpenFileName(
            &dlg,
            trBi(lang, "Choose Replacement Image", "Neues Bild wählen"),
            activeDocumentDir(),
            trBi(
                lang,
                "Images (*.png *.jpg *.jpeg *.svg *.webp *.gif *.bmp *.avif)",
                "Bilder (*.png *.jpg *.jpeg *.svg *.webp *.gif *.bmp *.avif)"
            )
        );
        if (!f.isEmpty()) {
            urlEdit->setText(makeFriendlyMarkdownPath(f));
            if (altEdit->text().trimmed().isEmpty() || altEdit->text() == prevStem) {
                altEdit->setText(QFileInfo(f).completeBaseName());
            }
        }
    });

    form->addRow(trBi(lang, "Image path / URL:", "Bildpfad / URL:"), pathRow);
    form->addRow(trBi(lang, "Alt text:", "Alternativtext:"), altEdit);

    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);
    form->addRow(buttons);

    if (dlg.exec() == QDialog::Accepted && !urlEdit->text().trimmed().isEmpty()) {
        applyImageUpdate(
            startByte,
            endByte,
            altEdit->text(),
            urlEdit->text().trimmed(),
            caption,
            sizeKind,
            sizeValue
        );
    }
}

void EditorViewport::contextMenuEvent(QContextMenuEvent* event) {
    if (m_session == nullptr) {
        return;
    }

    const UiLang lang = currentUiLang();
    QPointF hitPos;
    QPoint popupGlobalPos = event->globalPos();
    if (event->reason() == QContextMenuEvent::Keyboard) {
        hitPos = m_lastCaretRect.center();
        popupGlobalPos = viewport()->mapToGlobal(m_lastCaretRect.bottomLeft().toPoint());
    } else {
        hitPos = QPointF(viewport()->mapFromGlobal(event->globalPos()));
    }

    KMarkImageInfo imgInfo{};
    if (kmark_session_image_at(
            m_session,
            static_cast<float>(hitPos.x()),
            static_cast<float>(hitPos.y()),
            &imgInfo
        )) {
        const size_t startByte = imgInfo.start_byte;
        const size_t endByte = imgInfo.end_byte;
        const uint8_t sizeKind = imgInfo.size_kind;
        const uint16_t sizeValue = imgInfo.size_value;
        const QString alt = imgInfo.alt_alloc != nullptr ? QString::fromUtf8(imgInfo.alt_alloc) : QString();
        const QString src = imgInfo.src_alloc != nullptr ? QString::fromUtf8(imgInfo.src_alloc) : QString();
        const QString caption = imgInfo.caption_alloc != nullptr ? QString::fromUtf8(imgInfo.caption_alloc) : QString();
        kmark_image_info_free(&imgInfo);

        QMenu menu(this);

        /* 1. Add / Edit / Remove Caption */
        if (caption.isEmpty()) {
            QAction* actAddCap = menu.addAction(
                QIcon::fromTheme(QStringLiteral("insert-text")),
                trBi(lang, "Add &Caption…", "&Bildunterschrift hinzufügen…")
            );
            connect(actAddCap, &QAction::triggered, this, [=, this]() {
                promptImageCaption(startByte, endByte, alt, src, caption, sizeKind, sizeValue);
            });
        } else {
            QAction* actEditCap = menu.addAction(
                QIcon::fromTheme(QStringLiteral("document-edit")),
                trBi(lang, "Edit &Caption…", "&Bildunterschrift bearbeiten…")
            );
            connect(actEditCap, &QAction::triggered, this, [=, this]() {
                promptImageCaption(startByte, endByte, alt, src, caption, sizeKind, sizeValue);
            });

            QAction* actRemCap = menu.addAction(
                QIcon::fromTheme(QStringLiteral("edit-clear")),
                trBi(lang, "Re&move Caption", "Bildunterschrift &entfernen")
            );
            connect(actRemCap, &QAction::triggered, this, [=, this]() {
                applyImageUpdate(startByte, endByte, alt, src, QString(), sizeKind, sizeValue);
            });
        }

        /* 2. Replace Image */
        QAction* actReplace = menu.addAction(
            QIcon::fromTheme(QStringLiteral("insert-image")),
            trBi(lang, "&Replace Image…", "Bild &ersetzen…")
        );
        connect(actReplace, &QAction::triggered, this, [=, this]() {
            promptReplaceImage(startByte, endByte, alt, src, caption, sizeKind, sizeValue);
        });

        /* 3. Relative Size Submenu */
        QMenu* sizeMenu = menu.addMenu(
            QIcon::fromTheme(QStringLiteral("transform-scale")),
            trBi(lang, "&Size", "&Größe")
        );
        auto* sizeGroup = new QActionGroup(sizeMenu);
        sizeGroup->setExclusive(true);

        QAction* actAuto = sizeMenu->addAction(
            trBi(lang, "&Automatic (Original Size)", "&Automatisch (Originalgröße)")
        );
        actAuto->setCheckable(true);
        actAuto->setChecked(sizeKind == KMARK_IMG_SIZE_AUTO);
        sizeGroup->addAction(actAuto);
        connect(actAuto, &QAction::triggered, this, [=, this]() {
            applyImageUpdate(startByte, endByte, alt, src, caption, KMARK_IMG_SIZE_AUTO, 0);
        });

        sizeMenu->addSeparator();

        bool matchedPreset = (sizeKind == KMARK_IMG_SIZE_AUTO);
        static constexpr uint16_t presets[] = {25, 33, 50, 66, 75, 100};
        for (uint16_t pct : presets) {
            const QString label = (pct == 100)
                ? trBi(lang, "100 % (Column Width)", "100 % (Spaltenbreite)")
                : QStringLiteral("%1 %").arg(pct);
            QAction* actPct = sizeMenu->addAction(label);
            actPct->setCheckable(true);
            if (sizeKind == KMARK_IMG_SIZE_PERCENT && sizeValue == pct) {
                actPct->setChecked(true);
                matchedPreset = true;
            }
            sizeGroup->addAction(actPct);
            connect(actPct, &QAction::triggered, this, [=, this]() {
                applyImageUpdate(startByte, endByte, alt, src, caption, KMARK_IMG_SIZE_PERCENT, pct);
            });
        }

        sizeMenu->addSeparator();

        QString customLabel = trBi(lang, "&Custom Relative Size (%)…", "&Benutzerdefinierte Größe (%)…");
        if (!matchedPreset) {
            if (sizeKind == KMARK_IMG_SIZE_PERCENT) {
                customLabel = trBi(
                    lang,
                    "&Custom Relative Size (%1 %)…",
                    "&Benutzerdefinierte Größe (%1 %)…"
                ).arg(sizeValue);
            } else if (sizeKind == KMARK_IMG_SIZE_PIXELS) {
                customLabel = trBi(
                    lang,
                    "&Custom Size (%1 px)…",
                    "&Benutzerdefinierte Größe (%1 px)…"
                ).arg(sizeValue);
            }
        }
        QAction* actCustom = sizeMenu->addAction(customLabel);
        actCustom->setCheckable(true);
        actCustom->setChecked(!matchedPreset);
        sizeGroup->addAction(actCustom);
        connect(actCustom, &QAction::triggered, this, [=, this]() {
            const int initialPct = (sizeKind == KMARK_IMG_SIZE_PERCENT && sizeValue >= 5 && sizeValue <= 100)
                ? static_cast<int>(sizeValue)
                : 50;
            bool ok = false;
            const int chosen = QInputDialog::getInt(
                this,
                trBi(lang, "Image Size", "Bildgröße"),
                trBi(
                    lang,
                    "Relative image width (% of text column):",
                    "Relative Bildgröße (% der Spaltenbreite):"
                ),
                initialPct,
                5,
                100,
                5,
                &ok
            );
            if (ok) {
                applyImageUpdate(
                    startByte,
                    endByte,
                    alt,
                    src,
                    caption,
                    KMARK_IMG_SIZE_PERCENT,
                    static_cast<uint16_t>(chosen)
                );
            }
        });

        menu.addSeparator();
        QAction* actCopyPath = menu.addAction(
            QIcon::fromTheme(QStringLiteral("edit-copy")),
            trBi(lang, "Copy Image &Path", "Bild&pfad kopieren")
        );
        connect(actCopyPath, &QAction::triggered, this, [src]() {
            QApplication::clipboard()->setText(src);
        });

        menu.exec(popupGlobalPos);
        event->accept();
        return;
    }

    /* Standard KDE editor context menu when right-clicking non-image text */
    const KMarkToolbarState st = kmark_session_toolbar_state(m_session);
    char* rawSel = kmark_session_selected_text_alloc(m_session);
    const bool hasSelection = (rawSel != nullptr && rawSel[0] != '\0');
    kmark_string_free(rawSel);

    QMenu menu(this);
    QAction* actUndo = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-undo")),
        trBi(lang, "&Undo", "&Rückgängig")
    );
    actUndo->setEnabled(st.can_undo);
    connect(actUndo, &QAction::triggered, this, [this]() {
        kmark_session_exec_command(m_session, KMARK_ACT_UNDO);
        notifyDocumentChanged();
    });

    QAction* actRedo = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-redo")),
        trBi(lang, "Re&do", "Wieder&herstellen")
    );
    actRedo->setEnabled(st.can_redo);
    connect(actRedo, &QAction::triggered, this, [this]() {
        kmark_session_exec_command(m_session, KMARK_ACT_REDO);
        notifyDocumentChanged();
    });

    menu.addSeparator();

    QAction* actCut = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-cut")),
        trBi(lang, "Cu&t", "&Ausschneiden")
    );
    actCut->setEnabled(hasSelection);
    connect(actCut, &QAction::triggered, this, &EditorViewport::cutSelectionToClipboard);

    QAction* actCopy = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-copy")),
        trBi(lang, "&Copy", "&Kopieren")
    );
    actCopy->setEnabled(hasSelection);
    connect(actCopy, &QAction::triggered, this, &EditorViewport::copySelectionToClipboard);

    QAction* actPaste = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-paste")),
        trBi(lang, "&Paste", "&Einfügen")
    );
    connect(actPaste, &QAction::triggered, this, &EditorViewport::pasteFromClipboard);

    menu.addSeparator();

    QAction* actSelAll = menu.addAction(
        QIcon::fromTheme(QStringLiteral("edit-select-all")),
        trBi(lang, "Select &All", "Alles &auswählen")
    );
    connect(actSelAll, &QAction::triggered, this, [this]() {
        kmark_session_exec_command(m_session, KMARK_ACT_SELECT_ALL);
        notifyDocumentChanged();
    });

    menu.exec(popupGlobalPos);
    event->accept();
}

void EditorViewport::wheelEvent(QWheelEvent* event) {
    if (m_session == nullptr) {
        return;
    }
    if (event->modifiers().testFlag(Qt::ControlModifier)) {
        const int dy = event->angleDelta().y();
        if (dy > 0) {
            kmark_session_exec_command(m_session, KMARK_ACT_ZOOM_IN);
        } else if (dy < 0) {
            kmark_session_exec_command(m_session, KMARK_ACT_ZOOM_OUT);
        }
        notifyDocumentChanged();
        event->accept();
        return;
    }

    const QPoint pixelDelta = event->pixelDelta();
    float dyPx = 0.0f;
    if (!pixelDelta.isNull()) {
        dyPx = static_cast<float>(-pixelDelta.y());
    } else {
        dyPx = static_cast<float>(-event->angleDelta().y()) * 0.45f;
    }
    kmark_session_scroll_pixels(m_session, dyPx);
    viewport()->update();
    event->accept();
}

void EditorViewport::dragEnterEvent(QDragEnterEvent* event) {
    const QMimeData* mime = event->mimeData();
    if (mime != nullptr && (mime->hasUrls() || mime->hasImage() || mime->hasText())) {
        event->acceptProposedAction();
        return;
    }
    QAbstractScrollArea::dragEnterEvent(event);
}

void EditorViewport::dragMoveEvent(QDragMoveEvent* event) {
    const QMimeData* mime = event->mimeData();
    if (mime != nullptr && (mime->hasUrls() || mime->hasImage() || mime->hasText())) {
        if (m_session != nullptr) {
            const QPointF pos = event->position();
            kmark_session_drop_cursor_at(m_session, static_cast<float>(pos.x()), static_cast<float>(pos.y()));
            viewport()->update();
        }
        event->acceptProposedAction();
        return;
    }
    QAbstractScrollArea::dragMoveEvent(event);
}

void EditorViewport::dropEvent(QDropEvent* event) {
    if (m_session == nullptr) {
        return;
    }
    const QMimeData* mime = event->mimeData();
    if (mime == nullptr) {
        return;
    }

    setFocus(Qt::OtherFocusReason);
    const QPointF pos = event->position();
    kmark_session_drop_cursor_at(m_session, static_cast<float>(pos.x()), static_cast<float>(pos.y()));

    bool insertedContent = false;
    if (mime->hasUrls()) {
        for (const QUrl& url : mime->urls()) {
            if (url.isLocalFile()) {
                const QString localPath = url.toLocalFile();
                const QFileInfo fi(localPath);
                if (hasImageExtension(localPath)) {
                    const QString mdPath = makeFriendlyMarkdownPath(fi.absoluteFilePath());
                    QString alt = fi.completeBaseName();
                    if (alt.isEmpty()) {
                        alt = QStringLiteral("Image");
                    }
                    const QByteArray altUtf8 = alt.toUtf8();
                    const QByteArray urlUtf8 = mdPath.toUtf8();
                    kmark_session_insert_image(m_session, altUtf8.constData(), urlUtf8.constData());
                    insertedContent = true;
                } else if (fi.isDir() || hasDocumentExtension(localPath)) {
                    emit fileDropped(fi.absoluteFilePath());
                } else {
                    const QString mdPath = makeFriendlyMarkdownPath(fi.absoluteFilePath());
                    const QByteArray txtUtf8 = fi.fileName().toUtf8();
                    const QByteArray urlUtf8 = mdPath.toUtf8();
                    kmark_session_insert_link(m_session, txtUtf8.constData(), urlUtf8.constData());
                    insertedContent = true;
                }
            } else {
                const QString urlStr = url.toString();
                if (hasImageExtension(url.path())) {
                    QString alt = QFileInfo(url.path()).completeBaseName();
                    if (alt.isEmpty()) {
                        alt = QStringLiteral("Image");
                    }
                    const QByteArray altUtf8 = alt.toUtf8();
                    const QByteArray urlUtf8 = urlStr.toUtf8();
                    kmark_session_insert_image(m_session, altUtf8.constData(), urlUtf8.constData());
                } else {
                    const QByteArray u = urlStr.toUtf8();
                    kmark_session_insert_link(m_session, u.constData(), u.constData());
                }
                insertedContent = true;
            }
        }
    } else if (mime->hasImage()) {
        const QImage img = qvariant_cast<QImage>(mime->imageData());
        if (!img.isNull()) {
            QString outDir = activeDocumentDir();
            const size_t active = kmark_session_active_tab(m_session);
            if (!kmark_session_tab_has_path(m_session, active)) {
                outDir = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
                if (outDir.isEmpty()) {
                    outDir = QDir::tempPath();
                }
                outDir += QStringLiteral("/kmark");
            }
            QDir().mkpath(outDir);
            const QString fileName = QStringLiteral("dropped-%1.png")
                .arg(QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss")));
            const QString fullPath = QDir(outDir).filePath(fileName);
            if (img.save(fullPath, "PNG")) {
                const QString mdPath = makeFriendlyMarkdownPath(fullPath);
                const QByteArray altUtf8 = QByteArrayLiteral("Image");
                const QByteArray urlUtf8 = mdPath.toUtf8();
                kmark_session_insert_image(m_session, altUtf8.constData(), urlUtf8.constData());
                insertedContent = true;
            }
        }
    } else if (mime->hasText()) {
        const QByteArray txtUtf8 = mime->text().toUtf8();
        kmark_session_insert_text(m_session, txtUtf8.constData());
        insertedContent = true;
    }

    if (insertedContent) {
        clearImageCache();
        notifyDocumentChanged();
    }
    event->acceptProposedAction();
}

void EditorViewport::focusInEvent(QFocusEvent* event) {
    QAbstractScrollArea::focusInEvent(event);
    resetCaretBlink();
    viewport()->update();
}

void EditorViewport::focusOutEvent(QFocusEvent* event) {
    QAbstractScrollArea::focusOutEvent(event);
    m_caretTimer.stop();
    viewport()->update();
}

} // namespace kmark
