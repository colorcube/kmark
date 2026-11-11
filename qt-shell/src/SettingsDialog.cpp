#include "SettingsDialog.h"

#include <QApplication>
#include <QColorDialog>
#include <QDialogButtonBox>
#include <QFontMetricsF>
#include <QFormLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QIcon>
#include <QInputDialog>
#include <QMessageBox>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QRegularExpression>
#include <QSignalBlocker>
#include <QUuid>
#include <QVBoxLayout>
#include <algorithm>
#include <cmath>

#include "PaletteBridge.h"

namespace kmark {

namespace {

QIcon makeColorSwatchIcon(const QColor& c) {
    QPixmap pm(36, 20);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);

    /* Subtle checkerboard for semi-transparent colors like highlightBg */
    const QRectF r(1.0, 1.0, 34.0, 18.0);
    QPainterPath clip;
    clip.addRoundedRect(r, 4.0, 4.0);
    p.save();
    p.setClipPath(clip);
    p.fillRect(r, QColor(235, 235, 235));
    p.fillRect(QRectF(1.0, 1.0, 17.0, 9.0), QColor(200, 200, 200));
    p.fillRect(QRectF(18.0, 10.0, 17.0, 9.0), QColor(200, 200, 200));
    p.fillRect(r, c);
    p.restore();

    p.setPen(QPen(QColor(120, 128, 138, 180), 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(r, 4.0, 4.0);
    return QIcon(pm);
}

class MiniAppearancePreview final : public QWidget {
public:
    MiniAppearancePreview(const AppSettings* settings, UiLang lang, QWidget* parent = nullptr)
        : QWidget(parent)
        , m_settings(settings)
        , m_lang(lang) {
        setMinimumSize(320, 155);
        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    }

protected:
    void paintEvent(QPaintEvent*) override {
        if (m_settings == nullptr) {
            return;
        }
        QPainter p(this);
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setRenderHint(QPainter::TextAntialiasing, true);

        const KMarkPalette pal = m_settings->effectivePalette(QApplication::palette());
        const QColor bg = toQColor(pal.canvas_bg);
        const QColor fg = toQColor(pal.text_fg);
        const QColor accent = toQColor(pal.accent);
        const QColor link = toQColor(pal.link_fg);
        const QColor muted = toQColor(pal.muted_fg);
        const QColor codeBg = toQColor(pal.code_bg);
        const QColor inlineCodeBg = toQColor(pal.inline_code_bg);
        const QColor hlBg = toQColor(pal.highlight_bg);
        const QColor quoteBar = toQColor(pal.quote_bar);
        const QColor quoteFg = toQColor(pal.quote_fg);
        const QColor border = toQColor(pal.table_border);

        const QRectF card = QRectF(rect()).adjusted(1.0, 1.0, -1.0, -1.0);
        p.setPen(QPen(border, 1.0));
        p.setBrush(bg);
        p.drawRoundedRect(card, 6.0, 6.0);

        p.save();
        p.setClipRect(card.adjusted(2.0, 2.0, -2.0, -2.0));

        const int basePx = std::clamp(static_cast<int>(std::lround(m_settings->baseFontPx * 0.92f)), 11, 24);
        QFont hFont = m_settings->effectiveSansFont();
        hFont.setPixelSize(std::clamp(static_cast<int>(std::lround(basePx * 1.32)), 13, 30));
        hFont.setBold(true);

        QFont bodyFont = m_settings->effectiveSansFont();
        bodyFont.setPixelSize(basePx);

        QFont monoFont = m_settings->effectiveMonoFont();
        monoFont.setPixelSize(std::max(10, basePx - 1));

        const qreal leftX = 34.0;
        qreal y = 16.0;

        /* 1. Heading with active HeadingMarkStyle */
        const QFontMetricsF hFm(hFont);
        const qreal hBase = y + hFm.ascent();
        if (m_settings->headingMarks == KMARK_MARKS_SLANTED) {
            p.setPen(QPen(accent, 2.8, Qt::SolidLine, Qt::RoundCap));
            for (int i = 0; i < 2; ++i) {
                const qreal bx = 14.0 + i * 5.5;
                p.drawLine(QPointF(bx + 2.4, y + 2.0), QPointF(bx, y + hFm.height() - 3.0));
            }
        } else if (m_settings->headingMarks == KMARK_MARKS_HASH) {
            p.setFont(bodyFont);
            p.setPen(muted);
            p.drawText(QPointF(12.0, hBase), QStringLiteral("##"));
        }
        p.setFont(hFont);
        p.setPen(fg);
        p.drawText(
            QPointF(leftX, hBase),
            trBi(m_lang, "Heading & Typography", "Überschrift & Typografie")
        );

        y += hFm.height() * m_settings->lineSpacing * 0.85;

        /* 2. Body line with Link, ==Highlight==, and `code` */
        const QFontMetricsF bFm(bodyFont);
        const QFontMetricsF mFm(monoFont);
        const qreal bBase = y + bFm.ascent();
        qreal x = leftX;

        p.setFont(bodyFont);
        p.setPen(fg);
        const QString s1 = trBi(m_lang, "Body text with ", "Fließtext mit ");
        p.drawText(QPointF(x, bBase), s1);
        x += bFm.horizontalAdvance(s1);

        const QString sLink = QStringLiteral("Link");
        p.setPen(link);
        p.drawText(QPointF(x, bBase), sLink);
        const qreal lw = bFm.horizontalAdvance(sLink);
        p.drawLine(QPointF(x, bBase + 2.0), QPointF(x + lw, bBase + 2.0));
        x += lw;

        p.setPen(fg);
        const QString s2 = QStringLiteral(", ");
        p.drawText(QPointF(x, bBase), s2);
        x += bFm.horizontalAdvance(s2);

        const QString sHl = trBi(m_lang, "highlight", "Marker");
        const qreal hlw = bFm.horizontalAdvance(sHl);
        p.setPen(Qt::NoPen);
        p.setBrush(hlBg);
        p.drawRoundedRect(QRectF(x - 3.0, y + 1.0, hlw + 6.0, bFm.height()), 3.0, 3.0);
        p.setPen(fg);
        p.drawText(QPointF(x, bBase), sHl);
        x += hlw + 6.0;

        const QString s3 = trBi(m_lang, " & ", " & ");
        p.drawText(QPointF(x, bBase), s3);
        x += bFm.horizontalAdvance(s3);

        const QString sCode = QStringLiteral("fn main()");
        const qreal cw = mFm.horizontalAdvance(sCode);
        p.setPen(Qt::NoPen);
        p.setBrush(inlineCodeBg);
        p.drawRoundedRect(QRectF(x - 3.0, y + 1.0, cw + 8.0, bFm.height()), 4.0, 4.0);
        p.setFont(monoFont);
        p.setPen(fg);
        p.drawText(QPointF(x + 1.0, bBase), sCode);

        y += bFm.height() * m_settings->lineSpacing + 4.0;

        /* 3. Blockquote + Code block strip */
        p.setPen(Qt::NoPen);
        p.setBrush(quoteBar);
        p.drawRoundedRect(QRectF(leftX, y, 3.5, bFm.height() + 2.0), 1.5, 1.5);
        p.setFont(bodyFont);
        p.setPen(quoteFg);
        p.drawText(
            QPointF(leftX + 11.0, y + bFm.ascent()),
            trBi(m_lang, "Calm, focused writing in KDE Plasma.", "Ruhiges, konzentriertes Schreiben in KDE.")
        );

        y += bFm.height() * m_settings->lineSpacing + 4.0;
        const QRectF codeBox(leftX, y, std::max(140.0, card.width() - leftX - 16.0), mFm.height() + 10.0);
        p.setPen(QPen(border, 0.8));
        p.setBrush(codeBg);
        p.drawRoundedRect(codeBox, 4.0, 4.0);
        p.setFont(monoFont);
        p.setPen(fg);
        p.drawText(QPointF(codeBox.left() + 8.0, codeBox.top() + 5.0 + mFm.ascent()), QStringLiteral("let answer = 42;"));

        p.restore();
    }

private:
    const AppSettings* m_settings = nullptr;
    UiLang m_lang = UiLang::English;
};

QString slugifyPresetName(const QString& name) {
    QString s = name.trimmed().toLower();
    s.replace(QRegularExpression(QStringLiteral("[^a-z0-9_-]+")), QStringLiteral("-"));
    while (s.startsWith(QLatin1Char('-'))) {
        s.remove(0, 1);
    }
    while (s.endsWith(QLatin1Char('-'))) {
        s.chop(1);
    }
    if (s.isEmpty()) {
        s = QUuid::createUuid().toString(QUuid::Id128).left(8);
    }
    return QStringLiteral("user:") + s;
}

} // namespace

SettingsDialog::SettingsDialog(const AppSettings& current, UiLang lang, QWidget* parent)
    : QDialog(parent)
    , m_lang(lang)
    , m_working(current) {
    setWindowTitle(T("Configure KMark — Settings", "KMark einrichten — Einstellungen"));
    resize(820, 580);

    auto* rootLayout = new QVBoxLayout(this);
    rootLayout->setSpacing(10);

    /* ======================== 1. TOP PRESET BAR ======================== */
    auto* presetGroup = new QGroupBox(T("Appearance Presets", "Design-Presets"), this);
    auto* presetVBox = new QVBoxLayout(presetGroup);
    auto* presetRow = new QHBoxLayout();
    auto* presetLbl = new QLabel(T("Active Preset:", "Aktives Preset:"), presetGroup);
    m_presetCombo = new QComboBox(presetGroup);
    m_presetCombo->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);

    m_btnSavePreset = new QPushButton(
        QIcon::fromTheme(QStringLiteral("document-save-as")),
        T("Save as Preset…", "Als Preset speichern…"),
        presetGroup
    );
    m_btnDeletePreset = new QPushButton(
        QIcon::fromTheme(QStringLiteral("edit-delete")),
        T("Delete Preset", "Preset löschen"),
        presetGroup
    );

    presetRow->addWidget(presetLbl);
    presetRow->addWidget(m_presetCombo, 1);
    presetRow->addWidget(m_btnSavePreset);
    presetRow->addWidget(m_btnDeletePreset);
    presetVBox->addLayout(presetRow);

    m_presetDescLabel = new QLabel(presetGroup);
    m_presetDescLabel->setWordWrap(true);
    QFont descFont = m_presetDescLabel->font();
    descFont.setItalic(true);
    m_presetDescLabel->setFont(descFont);
    presetVBox->addWidget(m_presetDescLabel);

    rootLayout->addWidget(presetGroup);

    /* ======================== 2. TWO-COLUMN BODY ======================== */
    auto* columnsLayout = new QHBoxLayout();
    columnsLayout->setSpacing(12);

    /* ---- Left Column: Fonts & Headings + Editor Behavior ---- */
    auto* leftCol = new QVBoxLayout();
    leftCol->setSpacing(10);

    auto* fontGroup = new QGroupBox(
        T("Fonts && Heading Indicators", "Schriften && Überschriften-Markierung"),
        this
    );
    auto* fontForm = new QFormLayout(fontGroup);

    auto* sansRow = new QHBoxLayout();
    m_chkSysSans = new QCheckBox(T("System font", "Systemschrift"), fontGroup);
    m_sansCombo = new QFontComboBox(fontGroup);
    sansRow->addWidget(m_chkSysSans);
    sansRow->addWidget(m_sansCombo, 1);
    fontForm->addRow(T("Body font:", "Fließtext-Schrift:"), sansRow);

    auto* monoRow = new QHBoxLayout();
    m_chkSysMono = new QCheckBox(T("System font", "Systemschrift"), fontGroup);
    m_monoCombo = new QFontComboBox(fontGroup);
    m_monoCombo->setFontFilters(QFontComboBox::MonospacedFonts);
    monoRow->addWidget(m_chkSysMono);
    monoRow->addWidget(m_monoCombo, 1);
    fontForm->addRow(T("Code font:", "Code-Schrift:"), monoRow);

    m_fontSizeSpin = new QSpinBox(fontGroup);
    m_fontSizeSpin->setRange(10, 40);
    m_fontSizeSpin->setSuffix(QStringLiteral(" px"));
    fontForm->addRow(T("Base font size:", "Schriftgröße:"), m_fontSizeSpin);

    m_lineSpacingSpin = new QDoubleSpinBox(fontGroup);
    m_lineSpacingSpin->setRange(1.10, 2.20);
    m_lineSpacingSpin->setSingleStep(0.04);
    m_lineSpacingSpin->setDecimals(2);
    fontForm->addRow(T("Line spacing:", "Zeilenabstand:"), m_lineSpacingSpin);

    m_headingMarksCombo = new QComboBox(fontGroup);
    m_headingMarksCombo->addItem(
        T("Slanted Bars (MDNote Style)", "Schräge Balken (MDNote-Stil)"),
        static_cast<int>(KMARK_MARKS_SLANTED)
    );
    m_headingMarksCombo->addItem(
        T("Hash Symbol (#)", "Raute-Symbol (#)"),
        static_cast<int>(KMARK_MARKS_HASH)
    );
    m_headingMarksCombo->addItem(
        T("No Indicators", "Keine Markierung"),
        static_cast<int>(KMARK_MARKS_NONE)
    );
    fontForm->addRow(T("Heading indicators:", "Überschriften-Markierung:"), m_headingMarksCombo);

    leftCol->addWidget(fontGroup);

    auto* editorGroup = new QGroupBox(T("Editor && View", "Editor && Ansicht"), this);
    auto* editorVBox = new QVBoxLayout(editorGroup);
    auto* editorForm = new QFormLayout();

    m_defaultModeCombo = new QComboBox(editorGroup);
    m_defaultModeCombo->addItem(T("Formatted (WYSIWYG)", "Formatiert (WYSIWYG)"), static_cast<int>(KMARK_VIEW_WYSIWYG));
    m_defaultModeCombo->addItem(T("Live Preview", "Live-Vorschau"), static_cast<int>(KMARK_VIEW_LIVE_PREVIEW));
    m_defaultModeCombo->addItem(T("Markdown Source", "Markdown-Quelltext"), static_cast<int>(KMARK_VIEW_SOURCE));
    m_defaultModeCombo->addItem(T("Plain Text (TXT)", "Reintext (TXT)"), static_cast<int>(KMARK_VIEW_PLAIN_TEXT));
    editorForm->addRow(T("Default view mode:", "Standard-Ansicht:"), m_defaultModeCombo);

    m_maxWidthSpin = new QSpinBox(editorGroup);
    m_maxWidthSpin->setRange(520, 2400);
    m_maxWidthSpin->setSingleStep(20);
    m_maxWidthSpin->setSuffix(QStringLiteral(" px"));
    editorForm->addRow(T("Max column width:", "Max. Textspaltenbreite:"), m_maxWidthSpin);
    editorVBox->addLayout(editorForm);

    m_chkWordWrap = new QCheckBox(T("Dynamic word wrap (F10)", "Dynamischer Zeilenumbruch (F10)"), editorGroup);
    m_chkLineNumbers = new QCheckBox(T("Show line numbers in Formatted mode (F11)", "Zeilennummern im Formatierten Modus (F11)"), editorGroup);
    m_chkFocusMode = new QCheckBox(T("Focus mode (highlight active paragraph)", "Fokus-Modus (aktiven Absatz hervorheben)"), editorGroup);
    m_chkTypewriter = new QCheckBox(T("Typewriter mode (keep cursor centered)", "Schreibmaschinen-Modus (Cursor zentrieren)"), editorGroup);
    m_chkSingleInstance = new QCheckBox(
        T("Open files in existing window as tabs", "Dateien im bestehenden Fenster als Tab öffnen"),
        editorGroup
    );
    m_chkSingleInstance->setToolTip(
        T(
            "When opening files from Dolphin or the command line, reuse the running KMark window instead of starting a new window (override with kmark --new-window).",
            "Öffnet Dateien aus Dolphin oder der Kommandozeile als neuen Tab im bereits laufenden KMark-Fenster statt in einer neuen Instanz (mit kmark --new-window übersteuerbar)."
        )
    );

    editorVBox->addWidget(m_chkWordWrap);
    editorVBox->addWidget(m_chkLineNumbers);
    editorVBox->addWidget(m_chkFocusMode);
    editorVBox->addWidget(m_chkTypewriter);
    editorVBox->addWidget(m_chkSingleInstance);
    editorVBox->addStretch(1);

    leftCol->addWidget(editorGroup, 1);

    /* ---- Right Column: Colors + Live Preview ---- */
    auto* rightCol = new QVBoxLayout();
    rightCol->setSpacing(10);

    auto* colorGroup = new QGroupBox(T("Colors", "Farben"), this);
    auto* colorVBox = new QVBoxLayout(colorGroup);

    m_chkSysColors = new QCheckBox(
        T("Follow KDE Plasma system color scheme automatically", "KDE-Plasma-Systemfarbschema automatisch verwenden"),
        colorGroup
    );
    colorVBox->addWidget(m_chkSysColors);

    const std::array<QString, 8> colorLabels = {{
        T("Background:", "Hintergrund:"),
        T("Body text:", "Textfarbe:"),
        T("Accent / H1–H6:", "Akzent / H1–H6:"),
        T("Links:", "Links:"),
        T("Code block:", "Codeblock:"),
        T("Quote bar:", "Zitatbalken:"),
        T("Highlight (==…==):", "Textmarker (==…==):"),
        T("Table header:", "Tabellenkopf:"),
    }};

    auto* colorGrid = new QGridLayout();
    colorGrid->setHorizontalSpacing(10);
    colorGrid->setVerticalSpacing(6);
    for (int i = 0; i < 8; ++i) {
        const int row = i / 2;
        const int col = (i % 2) * 2;
        auto* lbl = new QLabel(colorLabels[static_cast<size_t>(i)], colorGroup);
        auto* btn = new QPushButton(colorGroup);
        btn->setMinimumWidth(118);
        connect(btn, &QPushButton::clicked, this, [this, i]() { pickColorSlot(i); });
        m_colorBtns[static_cast<size_t>(i)] = btn;
        colorGrid->addWidget(lbl, row, col);
        colorGrid->addWidget(btn, row, col + 1);
    }
    colorVBox->addLayout(colorGrid);
    rightCol->addWidget(colorGroup);

    auto* previewGroup = new QGroupBox(T("Live Preview", "Vorschau"), this);
    auto* previewVBox = new QVBoxLayout(previewGroup);
    m_previewCanvas = new MiniAppearancePreview(&m_working, m_lang, previewGroup);
    previewVBox->addWidget(m_previewCanvas, 1);
    rightCol->addWidget(previewGroup, 1);

    columnsLayout->addLayout(leftCol, 1);
    columnsLayout->addLayout(rightCol, 1);
    rootLayout->addLayout(columnsLayout, 1);

    /* ======================== 3. DIALOG BUTTON BOX ======================== */
    auto* buttons = new QDialogButtonBox(
        QDialogButtonBox::Ok | QDialogButtonBox::Apply | QDialogButtonBox::Cancel | QDialogButtonBox::RestoreDefaults,
        this
    );
    if (auto* btnApply = buttons->button(QDialogButtonBox::Apply)) {
        btnApply->setText(T("Apply", "Anwenden"));
        connect(btnApply, &QPushButton::clicked, this, [this]() {
            emit settingsApplied(m_working);
        });
    }
    if (auto* btnCancel = buttons->button(QDialogButtonBox::Cancel)) {
        btnCancel->setText(T("Cancel", "Abbrechen"));
    }
    if (auto* btnDefaults = buttons->button(QDialogButtonBox::RestoreDefaults)) {
        btnDefaults->setText(T("Defaults", "Standardwerte"));
        connect(btnDefaults, &QPushButton::clicked, this, &SettingsDialog::restoreDefaults);
    }
    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    rootLayout->addWidget(buttons);

    /* ======================== 4. SIGNAL WIRING ======================== */
    connect(m_presetCombo, &QComboBox::currentIndexChanged, this, [this](int idx) {
        if (m_syncing || idx < 0) {
            return;
        }
        const QString id = m_presetCombo->itemData(idx).toString();
        if (id.isEmpty()) {
            return;
        }
        if (const AppearancePreset* preset = m_working.findPresetById(id)) {
            m_working.applyPreset(*preset);
            loadWorkingIntoControls();
        }
    });

    connect(m_btnSavePreset, &QPushButton::clicked, this, &SettingsDialog::saveCurrentAsPreset);
    connect(m_btnDeletePreset, &QPushButton::clicked, this, &SettingsDialog::deleteSelectedUserPreset);

    connect(m_chkSysSans, &QCheckBox::toggled, this, [this](bool checked) {
        if (m_syncing) {
            return;
        }
        m_working.useSystemSans = checked;
        if (!checked && m_working.sansFamily.isEmpty()) {
            m_working.sansFamily = m_sansCombo->currentFont().family();
        }
        m_sansCombo->setEnabled(!checked);
        onAnyAppearanceEdited();
    });

    connect(m_sansCombo, &QFontComboBox::currentFontChanged, this, [this](const QFont& f) {
        if (m_syncing) {
            return;
        }
        m_working.sansFamily = f.family();
        m_working.useSystemSans = false;
        onAnyAppearanceEdited();
    });

    connect(m_chkSysMono, &QCheckBox::toggled, this, [this](bool checked) {
        if (m_syncing) {
            return;
        }
        m_working.useSystemMono = checked;
        if (!checked && m_working.monoFamily.isEmpty()) {
            m_working.monoFamily = m_monoCombo->currentFont().family();
        }
        m_monoCombo->setEnabled(!checked);
        onAnyAppearanceEdited();
    });

    connect(m_monoCombo, &QFontComboBox::currentFontChanged, this, [this](const QFont& f) {
        if (m_syncing) {
            return;
        }
        m_working.monoFamily = f.family();
        m_working.useSystemMono = false;
        onAnyAppearanceEdited();
    });

    connect(m_fontSizeSpin, &QSpinBox::valueChanged, this, [this](int px) {
        if (m_syncing) {
            return;
        }
        m_working.baseFontPx = static_cast<float>(px);
        onAnyAppearanceEdited();
    });

    connect(m_lineSpacingSpin, &QDoubleSpinBox::valueChanged, this, [this](double val) {
        if (m_syncing) {
            return;
        }
        m_working.lineSpacing = static_cast<float>(val);
        onAnyAppearanceEdited();
    });

    connect(m_headingMarksCombo, &QComboBox::currentIndexChanged, this, [this](int) {
        if (m_syncing) {
            return;
        }
        m_working.headingMarks = static_cast<uint8_t>(m_headingMarksCombo->currentData().toInt());
        onAnyAppearanceEdited();
    });

    connect(m_chkSysColors, &QCheckBox::toggled, this, [this](bool checked) {
        if (m_syncing) {
            return;
        }
        m_working.useSystemColors = checked;
        if (checked) {
            m_working.customColors = AppearanceColors::fromQtPalette(QApplication::palette());
        }
        updateColorButtonsUi();
        onAnyAppearanceEdited();
    });

    connect(m_defaultModeCombo, &QComboBox::currentIndexChanged, this, [this](int) {
        if (!m_syncing) {
            m_working.defaultViewMode = static_cast<uint8_t>(m_defaultModeCombo->currentData().toInt());
        }
    });

    connect(m_maxWidthSpin, &QSpinBox::valueChanged, this, [this](int w) {
        if (m_syncing) {
            return;
        }
        m_working.maxProseWidth = static_cast<float>(w);
        onAnyAppearanceEdited();
    });

    connect(m_chkWordWrap, &QCheckBox::toggled, this, [this](bool v) {
        if (!m_syncing) {
            m_working.wordWrap = v;
        }
    });
    connect(m_chkLineNumbers, &QCheckBox::toggled, this, [this](bool v) {
        if (!m_syncing) {
            m_working.showLineNumbers = v;
        }
    });
    connect(m_chkFocusMode, &QCheckBox::toggled, this, [this](bool v) {
        if (!m_syncing) {
            m_working.focusMode = v;
        }
    });
    connect(m_chkTypewriter, &QCheckBox::toggled, this, [this](bool v) {
        if (!m_syncing) {
            m_working.typewriterMode = v;
        }
    });
    connect(m_chkSingleInstance, &QCheckBox::toggled, this, [this](bool v) {
        if (!m_syncing) {
            m_working.singleInstanceTabs = v;
        }
    });

    populatePresetCombo();
    loadWorkingIntoControls();
}

void SettingsDialog::populatePresetCombo() {
    const QSignalBlocker blocker(m_presetCombo);
    m_presetCombo->clear();

    for (const AppearancePreset& p : builtinPresets()) {
        m_presetCombo->addItem(p.displayName(m_lang), p.id);
    }

    if (!m_working.userPresets.isEmpty()) {
        m_presetCombo->insertSeparator(m_presetCombo->count());
        for (const AppearancePreset& up : m_working.userPresets) {
            m_presetCombo->addItem(
                QStringLiteral("★ %1").arg(up.displayName(m_lang)),
                up.id
            );
        }
    }

    int matchIdx = -1;
    if (!m_working.activePresetId.isEmpty()) {
        matchIdx = m_presetCombo->findData(m_working.activePresetId);
    }
    if (matchIdx < 0) {
        m_presetCombo->insertSeparator(m_presetCombo->count());
        m_presetCombo->addItem(T("(Custom / Modified)", "(Benutzerdefiniert / Angepasst)"), QString());
        matchIdx = m_presetCombo->count() - 1;
    }
    m_presetCombo->setCurrentIndex(matchIdx);

    const AppearancePreset* activeP = m_working.findPresetById(m_working.activePresetId);
    if (activeP != nullptr) {
        m_presetDescLabel->setText(activeP->displayDescription(m_lang));
        m_btnDeletePreset->setEnabled(!activeP->isBuiltin);
    } else {
        m_presetDescLabel->setText(
            T(
                "Customized settings — click “Save as Preset…” to add them to the menu.",
                "Angepasste Einstellungen — über „Als Preset speichern…“ im Menü verfügbar machen."
            )
        );
        m_btnDeletePreset->setEnabled(false);
    }
}

void SettingsDialog::loadWorkingIntoControls() {
    m_syncing = true;

    m_chkSysSans->setChecked(m_working.useSystemSans);
    m_sansCombo->setEnabled(!m_working.useSystemSans);
    m_sansCombo->setCurrentFont(m_working.effectiveSansFont());

    m_chkSysMono->setChecked(m_working.useSystemMono);
    m_monoCombo->setEnabled(!m_working.useSystemMono);
    m_monoCombo->setCurrentFont(m_working.effectiveMonoFont());

    m_fontSizeSpin->setValue(static_cast<int>(std::lround(m_working.baseFontPx)));
    m_lineSpacingSpin->setValue(static_cast<double>(m_working.lineSpacing));

    const int hmIdx = m_headingMarksCombo->findData(static_cast<int>(m_working.headingMarks));
    m_headingMarksCombo->setCurrentIndex(std::max(0, hmIdx));

    m_chkSysColors->setChecked(m_working.useSystemColors);
    updateColorButtonsUi();

    const int vmIdx = m_defaultModeCombo->findData(static_cast<int>(m_working.defaultViewMode));
    m_defaultModeCombo->setCurrentIndex(std::max(0, vmIdx));
    m_maxWidthSpin->setValue(static_cast<int>(std::lround(m_working.maxProseWidth)));

    m_chkWordWrap->setChecked(m_working.wordWrap);
    m_chkLineNumbers->setChecked(m_working.showLineNumbers);
    m_chkFocusMode->setChecked(m_working.focusMode);
    m_chkTypewriter->setChecked(m_working.typewriterMode);
    m_chkSingleInstance->setChecked(m_working.singleInstanceTabs);

    populatePresetCombo();
    m_previewCanvas->update();

    m_syncing = false;
}

void SettingsDialog::onAnyAppearanceEdited() {
    /* Check if current working values still match the active preset (or any preset) */
    const AppearancePreset* cur = m_working.findPresetById(m_working.activePresetId);
    if (cur == nullptr || !m_working.matchesAppearanceOf(*cur)) {
        m_working.activePresetId.clear();
        for (const AppearancePreset& p : m_working.allPresets()) {
            if (m_working.matchesAppearanceOf(p)) {
                m_working.activePresetId = p.id;
                break;
            }
        }
    }
    populatePresetCombo();
    m_previewCanvas->update();
}

void SettingsDialog::updateColorButtonsUi() {
    const AppearanceColors shown = m_working.useSystemColors
        ? AppearanceColors::fromQtPalette(QApplication::palette())
        : m_working.customColors;

    const std::array<QColor, 8> cols = {{
        shown.canvasBg,
        shown.textFg,
        shown.accent,
        shown.linkFg,
        shown.codeBg,
        shown.quoteBar,
        shown.highlightBg,
        shown.tableHeaderBg,
    }};

    for (size_t i = 0; i < cols.size(); ++i) {
        QPushButton* btn = m_colorBtns[i];
        btn->setIcon(makeColorSwatchIcon(cols[i]));
        btn->setText(cols[i].name(QColor::HexRgb).toUpper());
        btn->setEnabled(!m_working.useSystemColors);
    }
}

void SettingsDialog::pickColorSlot(int colorIndex) {
    AppearanceColors& c = m_working.customColors;
    QColor* target = nullptr;
    bool allowAlpha = false;
    switch (colorIndex) {
        case 0: target = &c.canvasBg; break;
        case 1: target = &c.textFg; break;
        case 2: target = &c.accent; break;
        case 3: target = &c.linkFg; break;
        case 4: target = &c.codeBg; break;
        case 5: target = &c.quoteBar; break;
        case 6: target = &c.highlightBg; allowAlpha = true; break;
        case 7: target = &c.tableHeaderBg; break;
        default: return;
    }

    QColorDialog::ColorDialogOptions opts;
    if (allowAlpha) {
        opts |= QColorDialog::ShowAlphaChannel;
    }
    const QColor chosen = QColorDialog::getColor(
        *target,
        this,
        T("Choose Color", "Farbe auswählen"),
        opts
    );
    if (!chosen.isValid()) {
        return;
    }
    *target = chosen;
    m_working.useSystemColors = false;
    m_syncing = true;
    m_chkSysColors->setChecked(false);
    m_syncing = false;
    updateColorButtonsUi();
    onAnyAppearanceEdited();
}

void SettingsDialog::saveCurrentAsPreset() {
    QString initialName;
    if (const AppearancePreset* cur = m_working.findPresetById(m_working.activePresetId)) {
        if (!cur->isBuiltin) {
            initialName = cur->displayName(m_lang);
        }
    }
    bool ok = false;
    const QString rawName = QInputDialog::getText(
        this,
        T("Save Appearance Preset", "Design-Preset speichern"),
        T("Preset name:", "Name des Presets:"),
        QLineEdit::Normal,
        initialName,
        &ok
    );
    const QString name = rawName.trimmed();
    if (!ok || name.isEmpty()) {
        return;
    }

    /* Update existing user preset if name matches case-insensitively */
    for (AppearancePreset& existing : m_working.userPresets) {
        if (existing.nameDe.compare(name, Qt::CaseInsensitive) == 0
            || existing.nameEn.compare(name, Qt::CaseInsensitive) == 0) {
            existing = m_working.captureAsPreset(existing.id, name);
            m_working.activePresetId = existing.id;
            populatePresetCombo();
            return;
        }
    }

    QString newId = slugifyPresetName(name);
    int suffix = 2;
    while (m_working.findPresetById(newId) != nullptr) {
        newId = QStringLiteral("%1-%2").arg(slugifyPresetName(name)).arg(suffix++);
    }

    const AppearancePreset created = m_working.captureAsPreset(newId, name);
    m_working.userPresets.append(created);
    m_working.activePresetId = created.id;
    populatePresetCombo();
}

void SettingsDialog::deleteSelectedUserPreset() {
    const AppearancePreset* cur = m_working.findPresetById(m_working.activePresetId);
    if (cur == nullptr || cur->isBuiltin) {
        return;
    }
    const QString presetName = cur->displayName(m_lang);
    const auto ans = QMessageBox::question(
        this,
        T("Delete Preset", "Preset löschen"),
        T("Delete user preset “%1”?", "Eigenes Preset „%1“ wirklich löschen?").arg(presetName),
        QMessageBox::Yes | QMessageBox::No,
        QMessageBox::No
    );
    if (ans != QMessageBox::Yes) {
        return;
    }

    const QString removedId = cur->id;
    for (qsizetype i = 0; i < m_working.userPresets.size(); ++i) {
        if (m_working.userPresets.at(i).id == removedId) {
            m_working.userPresets.removeAt(i);
            break;
        }
    }
    m_working.activePresetId.clear();
    populatePresetCombo();
}

void SettingsDialog::restoreDefaults() {
    const QList<AppearancePreset> savedUserPresets = m_working.userPresets;
    const QString savedFolder = m_working.lastFolderRoot;
    const QByteArray savedGeom = m_working.windowGeometry;
    const QByteArray savedState = m_working.windowState;

    m_working = AppSettings{};
    m_working.customColors = AppearanceColors::fromQtPalette(QApplication::palette());
    m_working.userPresets = savedUserPresets;
    m_working.lastFolderRoot = savedFolder;
    m_working.windowGeometry = savedGeom;
    m_working.windowState = savedState;

    loadWorkingIntoControls();
}

} // namespace kmark
