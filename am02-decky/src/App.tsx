import { useEffect, useState } from "react";
import {
  getSettings,
  getTdpProfiles,
  setManualTdp,
  setTdpProfile,
  setLanguage,
  setTheme,
  setTimeFormat,
  setLocation,
  Settings,
  TdpPreset,
} from "./backend/api";
import { pickLang, translations } from "./i18n";
import ProfileList from "./components/ProfileList";
import ManualTdpSlider from "./components/ManualTdpSlider";
import DisplaySettings from "./components/DisplaySettings";
import LocationSettings from "./components/LocationSettings";

// SteamOS 的 steamwebhelper 跑在 pressure-vessel 容器里，Steam 全局样式
// (Motiva Sans) 的 CJK fallback 会走 JP 字形。插件元素用的是 Steam hash class
// （不含 "decky"），且 App 的 useEffect 里 document 不一定指向插件实际渲染的
// 页面 —— 所以此前 document.head 注入 + [class*="decky"] 选择器全部失效。
// 正确做法：把 <style> 作为 React 子节点渲染在插件根里（随插件 DOM 进真实
// document），并用全局 * 选择器 + !important 强制简体字形。
const FONT_STYLE = `*:not(i):not(svg):not(script):not(style):not(link):not(noscript) {
  font-family: "Noto Sans CJK SC", "Noto Sans SC", "Source Han Sans SC",
               "Microsoft YaHei", sans-serif !important;
}`;

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [presets, setPresets] = useState<TdpPreset[]>([]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const [s, p] = await Promise.all([getSettings(), getTdpProfiles()]);
      if (!cancelled) {
        setSettings(s);
        setPresets(p);
      }
    })();
    // 周期轮询刷新：反映副屏触摸切档后的档位/语言/主题等。
    // get_settings 后端会从 am02-service 拉真实状态（get_state 反向同步）。
    const timer = setInterval(async () => {
      try {
        const s = await getSettings();
        if (!cancelled) setSettings(s);
      } catch (e) {
        // 忽略瞬时错误，下个周期重试
      }
    }, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  const currentProfile = settings?.currentProfile ?? "manual";
  const manualTdpMw = settings?.manualTdpMw ?? 30000;
  const manualMinMw = settings?.manualMinMw ?? 5000;
  const manualMaxMw = settings?.manualMaxMw ?? 54000;
  const language = settings?.language ?? 0;
  const theme = settings?.theme ?? 0;
  const timeFormat = settings?.timeFormat ?? 1;
  const location = settings?.location ?? "";
  const t = translations[pickLang(language)];

  return (
    <div>
      {/* 随插件 DOM 注入字体覆盖（见文件头注释） */}
      <style>{FONT_STYLE}</style>
      <ProfileList
        presets={presets}
        currentProfile={currentProfile}
        t={t}
        onSelect={async (id) => {
          const ok = await setTdpProfile(id);
          if (ok) setSettings((s) => (s ? { ...s, currentProfile: id } : s));
        }}
      />
      <ManualTdpSlider
        valueMw={manualTdpMw}
        minMw={manualMinMw}
        maxMw={manualMaxMw}
        active={currentProfile === "manual"}
        t={t}
        onChange={async (mw) => {
          const ok = await setManualTdp(mw);
          if (ok)
            setSettings((s) =>
              s ? { ...s, currentProfile: "manual", manualTdpMw: mw } : s,
            );
        }}
      />
      <DisplaySettings
        language={language}
        theme={theme}
        timeFormat={timeFormat}
        t={t}
        onLanguageChange={async (lang) => {
          const ok = await setLanguage(lang);
          if (ok) setSettings((s) => (s ? { ...s, language: lang } : s));
        }}
        onThemeChange={async (t) => {
          const ok = await setTheme(t);
          if (ok) setSettings((s) => (s ? { ...s, theme: t } : s));
        }}
        onTimeFormatChange={async (fmt) => {
          const ok = await setTimeFormat(fmt);
          if (ok) setSettings((s) => (s ? { ...s, timeFormat: fmt } : s));
        }}
      />
      <LocationSettings
        location={location}
        t={t}
        onLocationChange={async (city) => {
          const ok = await setLocation(city);
          if (ok)
            setSettings((s) => (s ? { ...s, location: city.trim() } : s));
        }}
      />
    </div>
  );
}
