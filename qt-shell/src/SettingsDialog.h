#pragma once

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDoubleSpinBox>
#include <QFontComboBox>
#include <QLabel>
#include <QPushButton>
#include <QSpinBox>
#include <array>

#include "AppSettings.h"

namespace kmark {

class SettingsDialog final : public QDialog {
    Q_OBJECT

public:
    explicit SettingsDialog(const AppSettings& current, UiLang lang, QWidget* parent = nullptr);

    const AppSettings& resultSettings() const { return m_working; }

signals:
    void settingsApplied(const AppSettings& settings);

private:
    QString T(const char* enUtf8, const char* deUtf8) const {
        return trBi(m_lang, enUtf8, deUtf8);
    }

    void populatePresetCombo();
    void loadWorkingIntoControls();
    void onAnyAppearanceEdited();
    void updateColorButtonsUi();
    void pickColorSlot(int colorIndex);
    void saveCurrentAsPreset();
    void deleteSelectedUserPreset();
    void restoreDefaults();

    UiLang m_lang = UiLang::English;
    AppSettings m_working;
    bool m_syncing = false;

    /* Preset bar */
    QComboBox* m_presetCombo = nullptr;
    QPushButton* m_btnSavePreset = nullptr;
    QPushButton* m_btnDeletePreset = nullptr;
    QLabel* m_presetDescLabel = nullptr;

    /* Fonts & Heading Marks */
    QCheckBox* m_chkSysSans = nullptr;
    QFontComboBox* m_sansCombo = nullptr;
    QCheckBox* m_chkSysMono = nullptr;
    QFontComboBox* m_monoCombo = nullptr;
    QSpinBox* m_fontSizeSpin = nullptr;
    QDoubleSpinBox* m_lineSpacingSpin = nullptr;
    QComboBox* m_headingMarksCombo = nullptr;

    /* Colors */
    QCheckBox* m_chkSysColors = nullptr;
    std::array<QPushButton*, 8> m_colorBtns{};

    /* Editor & View */
    QComboBox* m_defaultModeCombo = nullptr;
    QSpinBox* m_maxWidthSpin = nullptr;
    QCheckBox* m_chkWordWrap = nullptr;
    QCheckBox* m_chkLineNumbers = nullptr;
    QCheckBox* m_chkFocusMode = nullptr;
    QCheckBox* m_chkTypewriter = nullptr;

    /* Live Preview Widget */
    QWidget* m_previewCanvas = nullptr;
};

} // namespace kmark
