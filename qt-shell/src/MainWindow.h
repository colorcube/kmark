#pragma once

#ifdef KMARK_HAVE_KF6
#include <KXmlGuiWindow>
using KMarkWindowBase = KXmlGuiWindow;
#else
#include <QMainWindow>
using KMarkWindowBase = QMainWindow;
#endif

#include <QAction>
#include <QCheckBox>
#include <QComboBox>
#include <QDockWidget>
#include <QFileSystemModel>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QPoint>
#include <QPushButton>
#include <QStringList>
#include <QTabBar>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QTreeView>
#include <QTreeWidget>
#include <array>

#include "AppSettings.h"
#include "EditorViewport.h"
#include "I18n.h"
#include "kmark_ffi.h"

namespace kmark {

class MainWindow final : public KMarkWindowBase {
    Q_OBJECT

public:
    explicit MainWindow(UiLang initialLang = detectSystemLanguage(), QWidget* parent = nullptr);
    ~MainWindow() override;

    void openPathFromCli(const QString& path);
    void openWelcomeGuideIfEmpty();
    size_t restoreCrashDrafts();
    UiLang uiLanguage() const { return m_lang; }

protected:
    void closeEvent(QCloseEvent* event) override;

private:
    QString T(const char* enUtf8, const char* deUtf8) const {
        return trBi(m_lang, enUtf8, deUtf8);
    }

    void setupMenusAndToolbars();
    void setupCentralArea();
    void setupDocks();
    void setupStatusBar();
    void retranslateUi();

    void dispatchCommand(uint32_t cmdId);
    void syncUiState();
    void syncTabBar();
    void syncOutlineTree();

    void applySettingsToRuntime(bool applyDefaultViewMode);
    void syncSettingsFromRuntime(bool persistNow);
    void rebuildPresetsMenu();
    void promptSaveQuickPreset();
    void openSettingsDialog();
    void refreshToolbarIcons();

    void actionNewTab();
    void actionOpenFile();
    void actionOpenFolder();
    bool actionSave();
    bool actionSaveAs();
    void actionExportHtml();
    void actionPrintPreview();
    void actionPrint();
    bool confirmCloseTab(size_t tabIdx);

    void promptInsertLink();
    void promptInsertImage();
    void promptInsertTable();
    void promptInsertCodeFence();
    void promptSetCodeFenceLanguage(bool hasOffset, size_t openLineStart, const QString& currentLang, const QPoint& popupPos);
    void promptSetCalloutKind(bool hasOffset, size_t openLineStart, const QString& currentTag, const QPoint& popupPos);

    void setFolderRoot(const QString& dirPath);
    void updateFolderHeaderUi();
    void revealActiveFileInFolderTree();
    void performPendingRevealStep();

    void loadRecentFiles();
    void saveRecentFiles() const;
    void recordRecentFile(const QString& filePath);
    void rebuildRecentFilesMenu();

    void showFindBar(bool withReplace);
    void hideFindBar();
    void updateSearchQuery();

    UiLang m_lang = UiLang::English;
    AppSettings m_settings;
    KMarkSession* m_session = nullptr;
    EditorViewport* m_viewport = nullptr;
    QTabBar* m_tabBar = nullptr;

    /* Kate-style bottom inline Find & Replace bar */
    QWidget* m_findBar = nullptr;
    QWidget* m_replaceRow = nullptr;
    QLabel* m_findLabel = nullptr;
    QLineEdit* m_findInput = nullptr;
    QPushButton* m_btnFindPrev = nullptr;
    QPushButton* m_btnFindNext = nullptr;
    QCheckBox* m_caseCheck = nullptr;
    QCheckBox* m_wordCheck = nullptr;
    QCheckBox* m_regexCheck = nullptr;
    QLabel* m_matchCountLabel = nullptr;
    QToolButton* m_btnCloseFind = nullptr;
    QLabel* m_replaceLabel = nullptr;
    QLineEdit* m_replaceInput = nullptr;
    QPushButton* m_btnReplaceOne = nullptr;
    QPushButton* m_btnReplaceAll = nullptr;

    /* Docks */
    QDockWidget* m_outlineDock = nullptr;
    QTreeWidget* m_outlineTree = nullptr;
    QDockWidget* m_folderDock = nullptr;
    QFileSystemModel* m_fsModel = nullptr;
    QTreeView* m_folderView = nullptr;
    QToolButton* m_btnFolderUp = nullptr;
    QToolButton* m_btnFolderHome = nullptr;
    QLabel* m_folderPathLabel = nullptr;
    QToolButton* m_btnRevealActiveFile = nullptr;
    QString m_folderRootPath;
    QString m_pendingRevealPath;

    /* Menus & Recent Files */
    QMenu* m_fileMenu = nullptr;
    QMenu* m_recentMenu = nullptr;
    QStringList m_recentFiles;
    QMenu* m_editMenu = nullptr;
    QMenu* m_viewMenu = nullptr;
    QMenu* m_marksMenu = nullptr;
    QMenu* m_formatMenu = nullptr;
    QMenu* m_headingMenu = nullptr;
    QMenu* m_alertMenu = nullptr;
    QMenu* m_tableMenu = nullptr;
    QMenu* m_settingsMenu = nullptr;
    QMenu* m_presetsMenu = nullptr;
    QMenu* m_helpMenu = nullptr;

    /* Actions for live retranslation */
    QAction* m_actNew = nullptr;
    QAction* m_actOpen = nullptr;
    QAction* m_actOpenFolder = nullptr;
    QAction* m_actSave = nullptr;
    QAction* m_actSaveAs = nullptr;
    QAction* m_actExportHtml = nullptr;
    QAction* m_actPrintPreview = nullptr;
    QAction* m_actPrint = nullptr;
    QAction* m_actCloseTab = nullptr;
    QAction* m_actQuit = nullptr;

    QAction* m_actUndo = nullptr;
    QAction* m_actRedo = nullptr;
    QAction* m_actCut = nullptr;
    QAction* m_actCopy = nullptr;
    QAction* m_actPaste = nullptr;
    QAction* m_actSelectAll = nullptr;
    QAction* m_actFind = nullptr;
    QAction* m_actReplace = nullptr;

    QAction* m_actModeWysiwyg = nullptr;
    QAction* m_actModeLive = nullptr;
    QAction* m_actModeSource = nullptr;
    QAction* m_actModePlain = nullptr;
    QAction* m_actMarksSlanted = nullptr;
    QAction* m_actMarksHash = nullptr;
    QAction* m_actMarksNone = nullptr;
    QAction* m_actLineNumbers = nullptr;
    QAction* m_actWrap = nullptr;
    QAction* m_actFocus = nullptr;
    QAction* m_actTypewriter = nullptr;
    QAction* m_actZoomIn = nullptr;
    QAction* m_actZoomOut = nullptr;
    QAction* m_actZoomReset = nullptr;

    QAction* m_actBold = nullptr;
    QAction* m_actItalic = nullptr;
    QAction* m_actStrike = nullptr;
    QAction* m_actHighlight = nullptr;
    QAction* m_actInlineCode = nullptr;
    std::array<QAction*, 7> m_actHeadings{};
    QAction* m_actBullet = nullptr;
    QAction* m_actOrdered = nullptr;
    QAction* m_actTask = nullptr;
    QAction* m_actQuote = nullptr;
    QAction* m_actAlertNote = nullptr;
    QAction* m_actAlertTip = nullptr;
    QAction* m_actAlertImportant = nullptr;
    QAction* m_actAlertWarning = nullptr;
    QAction* m_actAlertCaution = nullptr;
    QAction* m_actLink = nullptr;
    QAction* m_actImage = nullptr;
    QAction* m_actFence = nullptr;
    QAction* m_actFenceLang = nullptr;
    QAction* m_actHr = nullptr;

    QAction* m_actInsTable = nullptr;
    QAction* m_actRowAbove = nullptr;
    QAction* m_actRowBelow = nullptr;
    QAction* m_actRowDel = nullptr;
    QAction* m_actColLeft = nullptr;
    QAction* m_actColRight = nullptr;
    QAction* m_actColDel = nullptr;
    QAction* m_actAlignLeft = nullptr;
    QAction* m_actAlignCenter = nullptr;
    QAction* m_actAlignRight = nullptr;

    QAction* m_actConfigure = nullptr;

    QAction* m_actGuide = nullptr;
    QAction* m_actReference = nullptr;
    QAction* m_actAbout = nullptr;
    QAction* m_actAboutQt = nullptr;

    /* Formatting toolbar widgets */
    QToolBar* m_mainToolBar = nullptr;
    QComboBox* m_headingCombo = nullptr;
    QAction* m_actTableToolbarSection = nullptr;
    QToolButton* m_btnModeWysiwyg = nullptr;
    QToolButton* m_btnModeLive = nullptr;
    QToolButton* m_btnModeSource = nullptr;
    QToolButton* m_btnModePlain = nullptr;

    /* Status bar labels */
    QLabel* m_statusCursor = nullptr;
    QLabel* m_statusCounts = nullptr;
    QLabel* m_statusMode = nullptr;

    QTimer m_recoveryTimer;
    int m_lastIconLightness = -1;
    bool m_updatingUi = false;
    bool m_handlingPoisonedSession = false;

    void handlePoisonedSession();
};

} // namespace kmark
