const manifest = {"name":"AM02 Decky","author":"am02-service team","flags":["root"],"api_version":1,"publish":{"tags":["tdp","root","ayaneo","am02"],"description":"AYANEO AM02 mini PC reverse-engineered side-screen service UI: TDP presets, manual TDP, display language/theme","image":""}};
const API_VERSION = 2;
const internalAPIConnection = window.__DECKY_SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED_deckyLoaderAPIInit;
if (!internalAPIConnection) {
    throw new Error('[@decky/api]: Failed to connect to the loader as as the loader API was not initialized. This is likely a bug in Decky Loader.');
}
let api;
try {
    api = internalAPIConnection.connect(API_VERSION, manifest.name);
}
catch {
    api = internalAPIConnection.connect(1, manifest.name);
    console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version 1. Some features may not work.`);
}
if (api._version != API_VERSION) {
    console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version ${api._version}. Some features may not work.`);
}
const call = api.call;
const callable = api.callable;
api.addEventListener;
api.removeEventListener;
api.routerHook;
api.toaster;
api.openFilePicker;
api.executeInTab;
api.injectCssIntoTab;
api.removeCssFromTab;
api.fetchNoCors;
api.getExternalResourceURL;
api.useQuickAccessVisible;
const definePlugin = (fn) => {
    return (...args) => {
        return fn(...args);
    };
};

var DefaultContext = {
  color: undefined,
  size: undefined,
  className: undefined,
  style: undefined,
  attr: undefined
};
var IconContext = SP_REACT.createContext && SP_REACT.createContext(DefaultContext);

var __assign = window && window.__assign || function () {
  __assign = Object.assign || function (t) {
    for (var s, i = 1, n = arguments.length; i < n; i++) {
      s = arguments[i];
      for (var p in s) if (Object.prototype.hasOwnProperty.call(s, p)) t[p] = s[p];
    }
    return t;
  };
  return __assign.apply(this, arguments);
};
var __rest = window && window.__rest || function (s, e) {
  var t = {};
  for (var p in s) if (Object.prototype.hasOwnProperty.call(s, p) && e.indexOf(p) < 0) t[p] = s[p];
  if (s != null && typeof Object.getOwnPropertySymbols === "function") for (var i = 0, p = Object.getOwnPropertySymbols(s); i < p.length; i++) {
    if (e.indexOf(p[i]) < 0 && Object.prototype.propertyIsEnumerable.call(s, p[i])) t[p[i]] = s[p[i]];
  }
  return t;
};
function Tree2Element(tree) {
  return tree && tree.map(function (node, i) {
    return SP_REACT.createElement(node.tag, __assign({
      key: i
    }, node.attr), Tree2Element(node.child));
  });
}
function GenIcon(data) {
  // eslint-disable-next-line react/display-name
  return function (props) {
    return SP_REACT.createElement(IconBase, __assign({
      attr: __assign({}, data.attr)
    }, props), Tree2Element(data.child));
  };
}
function IconBase(props) {
  var elem = function (conf) {
    var attr = props.attr,
      size = props.size,
      title = props.title,
      svgProps = __rest(props, ["attr", "size", "title"]);
    var computedSize = size || conf.size || "1em";
    var className;
    if (conf.className) className = conf.className;
    if (props.className) className = (className ? className + " " : "") + props.className;
    return SP_REACT.createElement("svg", __assign({
      stroke: "currentColor",
      fill: "currentColor",
      strokeWidth: "0"
    }, conf.attr, attr, svgProps, {
      className: className,
      style: __assign(__assign({
        color: props.color || conf.color
      }, conf.style), props.style),
      height: computedSize,
      width: computedSize,
      xmlns: "http://www.w3.org/2000/svg"
    }), title && SP_REACT.createElement("title", null, title), props.children);
  };
  return IconContext !== undefined ? SP_REACT.createElement(IconContext.Consumer, null, function (conf) {
    return elem(conf);
  }) : elem(DefaultContext);
}

// THIS FILE IS AUTO GENERATED
function BsCheck2 (props) {
  return GenIcon({"attr":{"fill":"currentColor","viewBox":"0 0 16 16"},"child":[{"tag":"path","attr":{"d":"M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z"}}]})(props);
}function BsCircle (props) {
  return GenIcon({"attr":{"fill":"currentColor","viewBox":"0 0 16 16"},"child":[{"tag":"path","attr":{"d":"M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z"}}]})(props);
}function BsCpuFill (props) {
  return GenIcon({"attr":{"fill":"currentColor","viewBox":"0 0 16 16"},"child":[{"tag":"path","attr":{"d":"M6.5 6a.5.5 0 0 0-.5.5v3a.5.5 0 0 0 .5.5h3a.5.5 0 0 0 .5-.5v-3a.5.5 0 0 0-.5-.5h-3z"}},{"tag":"path","attr":{"d":"M5.5.5a.5.5 0 0 0-1 0V2A2.5 2.5 0 0 0 2 4.5H.5a.5.5 0 0 0 0 1H2v1H.5a.5.5 0 0 0 0 1H2v1H.5a.5.5 0 0 0 0 1H2v1H.5a.5.5 0 0 0 0 1H2A2.5 2.5 0 0 0 4.5 14v1.5a.5.5 0 0 0 1 0V14h1v1.5a.5.5 0 0 0 1 0V14h1v1.5a.5.5 0 0 0 1 0V14h1v1.5a.5.5 0 0 0 1 0V14a2.5 2.5 0 0 0 2.5-2.5h1.5a.5.5 0 0 0 0-1H14v-1h1.5a.5.5 0 0 0 0-1H14v-1h1.5a.5.5 0 0 0 0-1H14v-1h1.5a.5.5 0 0 0 0-1H14A2.5 2.5 0 0 0 11.5 2V.5a.5.5 0 0 0-1 0V2h-1V.5a.5.5 0 0 0-1 0V2h-1V.5a.5.5 0 0 0-1 0V2h-1V.5zm1 4.5h3A1.5 1.5 0 0 1 11 6.5v3A1.5 1.5 0 0 1 9.5 11h-3A1.5 1.5 0 0 1 5 9.5v-3A1.5 1.5 0 0 1 6.5 5z"}}]})(props);
}

const getSettings = callable("get_settings");
const getTdpProfiles = callable("get_tdp_profiles");
const setTdpProfile = (id) => call("set_tdp_profile", id);
const setManualTdp = (mw) => call("set_manual_tdp", mw);
const setLanguage = (lang) => call("set_language", lang);
const setTheme = (theme) => call("set_theme", theme);
const setTimeFormat = (format) => call("set_time_format", format);
const setLocation = (city) => call("set_location", city);
const searchCity = callable("search_city");

// 简单的双语 i18n：zh / en 两套文案。
// 语言源是后端 get_settings 返回的 `language` 字段（0=中文 / 1=英文），
// set_language 成功后 App 更新 state，前端随之后切换语言。
const translations = {
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
function pickLang(language) {
    return language === 1 ? "en" : "zh";
}

/**
 * Four AYASpace-style TDP presets as a gamepad-navigable single-select list.
 * Selected row shows a check icon; the rest show an empty circle. Built on
 * ButtonItem (Steam focus ring + A/OK activation), no hover-only interaction.
 *
 * 模式名用简短符号（多语言友好），功耗显示成小号纯数字 + 单位，避免长句。
 */
function ProfileList({ presets, currentProfile, t, onSelect }) {
    return (window.SP_REACT.createElement(DFL.PanelSection, { title: t.tdpPresets }, presets.map((p) => {
        const selected = p.id === currentProfile;
        return (window.SP_REACT.createElement(DFL.PanelSectionRow, { key: p.id },
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", onClick: () => onSelect(p.id), icon: selected ? window.SP_REACT.createElement(BsCheck2, null) : window.SP_REACT.createElement(BsCircle, null), highlightOnFocus: true },
                t.profileNames[p.id],
                window.SP_REACT.createElement("span", { style: {
                        fontSize: "0.82em",
                        opacity: 0.6,
                        marginLeft: "0.6em",
                        fontWeight: 400,
                    } },
                    Math.round(p.stapm / 1000),
                    "W"))));
    })));
}

/**
 * Manual TDP slider (mW internally, displayed in Watts). Can exceed the 45W
 * preset cap up to ~54W (ryzenadj limit). Gamepad: left/right adjusts, A
 * confirms focus entry — standard SliderField behavior, no mouse hover.
 */
function ManualTdpSlider({ valueMw, minMw, maxMw, active, t, onChange, }) {
    const valueW = Math.round(valueMw / 1000);
    const minW = Math.round(minMw / 1000);
    const maxW = Math.round(maxMw / 1000);
    return (window.SP_REACT.createElement(DFL.PanelSection, { title: t.manualTdp },
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.SliderField, { label: active ? t.manualTdpActive : t.manualTdp, value: valueW, min: minW, max: maxW, step: 1, valueSuffix: " W", showValue: true, notchTicksVisible: true, validValues: "range", bottomSeparator: "none", onChange: (w) => onChange(Math.round(w) * 1000) }))));
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
function DisplaySettings({ language, theme, timeFormat, t, onLanguageChange, onThemeChange, onTimeFormatChange, }) {
    return (window.SP_REACT.createElement(DFL.PanelSection, { title: t.display },
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", highlightOnFocus: true, onClick: () => onLanguageChange(language === 0 ? 1 : 0) },
                t.language,
                ": ",
                language === 0 ? "中文" : "English")),
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", highlightOnFocus: true, onClick: () => onThemeChange(theme === 0 ? 1 : 0) },
                t.theme,
                ": ",
                theme === 0 ? t.themeBlack : t.themeWhite)),
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", highlightOnFocus: true, onClick: () => onTimeFormatChange(timeFormat === 0 ? 1 : 0) },
                t.timeFormat,
                ": ",
                timeFormat === 0 ? t.timeFormat12 : t.timeFormat24))));
}

/** 「区县名 · 省 · 地级市」三级展示；缺地级市则退两级。 */
function labelOf(h) {
    if (h.city && h.city !== h.name)
        return `${h.name} · ${h.province} · ${h.city}`;
    if (h.province && h.province !== h.name)
        return `${h.name} · ${h.province}`;
    return h.name;
}
/** 弹出式搜索面板：输入框 + 模糊匹配候选列表 + 关闭天气入口。 */
function SearchModal({ t, initial, onPick, onClear, }) {
    const [q, setQ] = SP_REACT.useState(initial);
    const [hits, setHits] = SP_REACT.useState([]);
    const seq = SP_REACT.useRef(0);
    SP_REACT.useEffect(() => {
        const query = q.trim();
        if (!query) {
            setHits([]);
            return;
        }
        const s = ++seq.current;
        const timer = setTimeout(() => {
            searchCity(query)
                .then((r) => {
                if (seq.current === s)
                    setHits(r?.cities ?? []);
            })
                .catch(() => {
                if (seq.current === s)
                    setHits([]);
            });
        }, 200);
        return () => clearTimeout(timer);
    }, [q]);
    return (window.SP_REACT.createElement("div", { style: { padding: "8px 0" } },
        window.SP_REACT.createElement(DFL.TextField, { label: t.location, value: q, bAlwaysShowClearAction: true, onChange: (e) => setQ(e.target.value) }),
        window.SP_REACT.createElement("div", { style: { maxHeight: "300px", overflowY: "auto", marginTop: "8px" } },
            hits.map((h) => (window.SP_REACT.createElement(DFL.ButtonItem, { key: `${h.name}-${h.lat},${h.lon}`, layout: "below", onClick: () => onPick(h) }, labelOf(h)))),
            q.trim() !== "" && hits.length === 0 && (window.SP_REACT.createElement("div", { style: { padding: "8px 16px" } }, t.locationNoMatch))),
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", onClick: onClear }, t.locationClear))));
}
function LocationSettings({ location, t, onLocationChange, }) {
    const [meta, setMeta] = SP_REACT.useState(null);
    const weatherOn = location.trim() !== "";
    // 反查已保存城市的三级区划信息
    SP_REACT.useEffect(() => {
        const q = location.trim();
        if (q) {
            searchCity(q)
                .then((r) => {
                const exact = (r?.cities ?? []).find((c) => c.name === q);
                setMeta(exact ?? null);
            })
                .catch(() => setMeta(null));
        }
        else {
            setMeta(null);
        }
    }, [location]);
    const openPicker = () => {
        let result;
        result = DFL.showModal(window.SP_REACT.createElement(SearchModal, { t: t, initial: location, onPick: (h) => {
                setMeta(h);
                onLocationChange(h.name);
                result?.Close();
            }, onClear: () => {
                setMeta(null);
                onLocationChange("");
                result?.Close();
            } }), undefined, { strTitle: t.weather, popupWidth: 560 });
    };
    return (window.SP_REACT.createElement(DFL.PanelSection, { title: t.weather },
        window.SP_REACT.createElement(DFL.PanelSectionRow, null,
            window.SP_REACT.createElement(DFL.ButtonItem, { layout: "below", description: weatherOn ? (meta ? labelOf(meta) : t.locationHint) : t.locationHint, onClick: openPicker }, weatherOn ? location : t.locationNotSet))));
}

// SteamOS 的 steamwebhelper 跑在 pressure-vessel 容器里，CEF 的 CJK
// fallback 不读主机 fontconfig（JP 字形优先于 SC）。这里直接在插件
// 挂载的 DOM 根上强制简体字形，绕开容器的字体 fallback 问题。
const FONT_STYLE_ID = "am02-decky-font-cjk-sc";
function injectFontStyle() {
    if (document.getElementById(FONT_STYLE_ID))
        return;
    const style = document.createElement("style");
    style.id = FONT_STYLE_ID;
    style.textContent = `
    #quickAccessMenu *, ._decky_quick_access_menu *,
    [class*="decky"] *:not(i):not(svg) {
      font-family: "Noto Sans CJK SC", "Noto Sans SC", "Source Han Sans SC",
                   "Microsoft YaHei", sans-serif !important;
    }
  `;
    document.head.appendChild(style);
}
function App() {
    const [settings, setSettings] = SP_REACT.useState(null);
    const [presets, setPresets] = SP_REACT.useState([]);
    SP_REACT.useEffect(() => {
        // 注入简体字形 CSS（每次挂载确保存在）
        injectFontStyle();
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
                if (!cancelled)
                    setSettings(s);
            }
            catch (e) {
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
    return (window.SP_REACT.createElement("div", null,
        window.SP_REACT.createElement(ProfileList, { presets: presets, currentProfile: currentProfile, t: t, onSelect: async (id) => {
                const ok = await setTdpProfile(id);
                if (ok)
                    setSettings((s) => (s ? { ...s, currentProfile: id } : s));
            } }),
        window.SP_REACT.createElement(ManualTdpSlider, { valueMw: manualTdpMw, minMw: manualMinMw, maxMw: manualMaxMw, active: currentProfile === "manual", t: t, onChange: async (mw) => {
                const ok = await setManualTdp(mw);
                if (ok)
                    setSettings((s) => s ? { ...s, currentProfile: "manual", manualTdpMw: mw } : s);
            } }),
        window.SP_REACT.createElement(DisplaySettings, { language: language, theme: theme, timeFormat: timeFormat, t: t, onLanguageChange: async (lang) => {
                const ok = await setLanguage(lang);
                if (ok)
                    setSettings((s) => (s ? { ...s, language: lang } : s));
            }, onThemeChange: async (t) => {
                const ok = await setTheme(t);
                if (ok)
                    setSettings((s) => (s ? { ...s, theme: t } : s));
            }, onTimeFormatChange: async (fmt) => {
                const ok = await setTimeFormat(fmt);
                if (ok)
                    setSettings((s) => (s ? { ...s, timeFormat: fmt } : s));
            } }),
        window.SP_REACT.createElement(LocationSettings, { location: location, t: t, onLocationChange: async (city) => {
                const ok = await setLocation(city);
                if (ok)
                    setSettings((s) => (s ? { ...s, location: city.trim() } : s));
            } })));
}

var index = definePlugin(() => {
    return {
        name: "AM02 Decky",
        content: window.SP_REACT.createElement(App, null),
        icon: window.SP_REACT.createElement(BsCpuFill, null),
        onDismount: () => { },
    };
});

export { index as default };
//# sourceMappingURL=index.js.map
