import {
  ButtonItem,
  PanelSection,
  PanelSectionRow,
} from "@decky/ui";
import type { Translations } from "../i18n";

interface Props {
  language: number; // 0 = Chinese, 1 = English
  theme: number; // 0 = black, 1 = white
  timeFormat: number; // 0 = 12h, 1 = 24h
  t: Translations;
  onLanguageChange: (lang: number) => void;
  onThemeChange: (theme: number) => void;
  onTimeFormatChange: (format: number) => void;
}

/**
 * Side-screen display language / theme / time-format toggles.
 *
 * These go through `am02-service` (Rust, systemd, root) over IPC, so IT
 * writes the side-screen protocol pack fields:
 *   pack[0x79] = language   (0 = Chinese, 1 = English)
 *   pack[0x7a] = theme      (0 = black, 1 = white)
 *   pack[0x84] = time format (0 = 12h, 1 = 24h)
 * This plugin must NOT touch the serial port directly.
 */
export default function DisplaySettings({
  language,
  theme,
  timeFormat,
  t,
  onLanguageChange,
  onThemeChange,
  onTimeFormatChange,
}: Props) {
  return (
    <PanelSection title={t.display}>
      <PanelSectionRow>
        <ButtonItem
          layout="below"
          highlightOnFocus
          onClick={() => onLanguageChange(language === 0 ? 1 : 0)}
        >
          {t.language}: {language === 0 ? "中文" : "English"}
        </ButtonItem>
      </PanelSectionRow>
      <PanelSectionRow>
        <ButtonItem
          layout="below"
          highlightOnFocus
          onClick={() => onThemeChange(theme === 0 ? 1 : 0)}
        >
          {t.theme}: {theme === 0 ? t.themeBlack : t.themeWhite}
        </ButtonItem>
      </PanelSectionRow>
      <PanelSectionRow>
        <ButtonItem
          layout="below"
          highlightOnFocus
          onClick={() => onTimeFormatChange(timeFormat === 0 ? 1 : 0)}
        >
          {t.timeFormat}: {timeFormat === 0 ? t.timeFormat12 : t.timeFormat24}
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
}
