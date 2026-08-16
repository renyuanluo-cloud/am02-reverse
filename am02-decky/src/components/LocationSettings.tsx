import { useEffect, useRef, useState } from "react";
import {
  ButtonItem,
  PanelSection,
  PanelSectionRow,
  TextField,
  showModal,
} from "@decky/ui";
import { searchCity, CityHit } from "../backend/api";
import type { Translations } from "../i18n";

interface Props {
  location: string;
  t: Translations;
  onLocationChange: (city: string) => void;
}

/** 「区县名 · 省 · 地级市」三级展示；缺地级市则退两级。 */
function labelOf(h: CityHit): string {
  if (h.city && h.city !== h.name) return `${h.name} · ${h.province} · ${h.city}`;
  if (h.province && h.province !== h.name) return `${h.name} · ${h.province}`;
  return h.name;
}

/** 弹出式搜索面板：输入框 + 模糊匹配候选列表 + 关闭天气入口。 */
function SearchModal({
  t,
  initial,
  onPick,
  onClear,
}: {
  t: Translations;
  initial: string;
  onPick: (h: CityHit) => void;
  onClear: () => void;
}) {
  const [q, setQ] = useState(initial);
  const [hits, setHits] = useState<CityHit[]>([]);
  const seq = useRef(0);

  useEffect(() => {
    const query = q.trim();
    if (!query) {
      setHits([]);
      return;
    }
    const s = ++seq.current;
    const timer = setTimeout(() => {
      searchCity(query)
        .then((r) => {
          if (seq.current === s) setHits(r?.cities ?? []);
        })
        .catch(() => {
          if (seq.current === s) setHits([]);
        });
    }, 200);
    return () => clearTimeout(timer);
  }, [q]);

  return (
    <div style={{ padding: "8px 0" }}>
      <TextField
        label={t.location}
        value={q}
        bAlwaysShowClearAction
        onChange={(e) => setQ(e.target.value)}
      />
      <div style={{ maxHeight: "300px", overflowY: "auto", marginTop: "8px" }}>
        {hits.map((h) => (
          <ButtonItem
            key={`${h.name}-${h.lat},${h.lon}`}
            layout="below"
            onClick={() => onPick(h)}
          >
            {labelOf(h)}
          </ButtonItem>
        ))}
        {q.trim() !== "" && hits.length === 0 && (
          <div style={{ padding: "8px 16px" }}>{t.locationNoMatch}</div>
        )}
      </div>
      <PanelSectionRow>
        <ButtonItem layout="below" onClick={onClear}>
          {t.locationClear}
        </ButtonItem>
      </PanelSectionRow>
    </div>
  );
}

export default function LocationSettings({
  location,
  t,
  onLocationChange,
}: Props) {
  const [meta, setMeta] = useState<CityHit | null>(null);
  const weatherOn = location.trim() !== "";

  // 反查已保存城市的三级区划信息
  useEffect(() => {
    const q = location.trim();
    if (q) {
      searchCity(q)
        .then((r) => {
          const exact = (r?.cities ?? []).find((c) => c.name === q);
          setMeta(exact ?? null);
        })
        .catch(() => setMeta(null));
    } else {
      setMeta(null);
    }
  }, [location]);

  const openPicker = () => {
    let result: ReturnType<typeof showModal>;
    result = showModal(
      <SearchModal
        t={t}
        initial={location}
        onPick={(h) => {
          setMeta(h);
          onLocationChange(h.name);
          result?.Close();
        }}
        onClear={() => {
          setMeta(null);
          onLocationChange("");
          result?.Close();
        }}
      />,
      undefined,
      { strTitle: t.weather, popupWidth: 560 }
    );
  };

  return (
    <PanelSection title={t.weather}>
      <PanelSectionRow>
        <ButtonItem
          layout="below"
          description={weatherOn ? (meta ? labelOf(meta) : t.locationHint) : t.locationHint}
          onClick={openPicker}
        >
          {weatherOn ? location : t.locationNotSet}
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
}
