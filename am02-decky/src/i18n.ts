// 简单的双语 i18n：zh / en 两套文案。
// 语言源是后端 get_settings 返回的 `language` 字段（0=中文 / 1=英文），
// set_language 成功后 App 更新 state，前端随之后切换语言。

export type Lang = "zh" | "en";

export type TdpPresetId = "office" | "retro" | "classic" | "aaa";

export interface Translations {
  tdpPresets: string;
  manualTdp: string;
  manualTdpActive: string;
  display: string;
  language: string;
  theme: string;
  themeBlack: string;
  themeWhite: string;
  timeFormat: string;
  timeFormat12: string;
  timeFormat24: string;
  weather: string;
  location: string;
  locationHint: string;
  locationApply: string;
  locationOff: string;
  locationNoMatch: string;
  locationPick: string;
  locationNotSet: string;
  locationClear: string;
  profileNames: Record<TdpPresetId, string>;
}

export const translations: Record<Lang, Translations> = {
  zh: {
    tdpPresets: "TDP 档位",
    manualTdp: "手动 TDP",
    manualTdpActive: "手动 TDP（生效中）",
    display: "显示",
    language: "语言",
    theme: "主题",
    themeBlack: "深色",
    themeWhite: "浅色",
    timeFormat: "时间格式",
    timeFormat12: "12 小时制",
    timeFormat24: "24 小时制",
    weather: "天气",
    location: "地区",
    locationHint: "输入城市名（如 深圳 / 南山区），留空关闭天气",
    locationApply: "应用",
    locationOff: "天气已关闭",
    locationNoMatch: "无匹配城市",
    locationPick: "选择城市",
    locationNotSet: "未设置",
    locationClear: "关闭天气",
    profileNames: {
      office: "办公",
      retro: "复古",
      classic: "经典",
      aaa: "3A",
    },
  },
  en: {
    tdpPresets: "TDP Presets",
    manualTdp: "Manual TDP",
    manualTdpActive: "Manual TDP (active)",
    display: "Display",
    language: "Language",
    theme: "Theme",
    themeBlack: "Black",
    themeWhite: "White",
    timeFormat: "Time Format",
    timeFormat12: "12-hour",
    timeFormat24: "24-hour",
    weather: "Weather",
    location: "Location",
    locationHint: "Enter a city name (e.g. Shenzhen / Nanshan); empty turns weather off",
    locationApply: "Apply",
    locationOff: "Weather off",
    locationNoMatch: "No matching city",
    locationPick: "Pick city",
    locationNotSet: "Not set",
    locationClear: "Clear weather",
    profileNames: {
      office: "Office",
      retro: "Retro",
      classic: "Classic",
      aaa: "AAA",
    },
  },
};

export function pickLang(language: number): Lang {
  return language === 1 ? "en" : "zh";
}
