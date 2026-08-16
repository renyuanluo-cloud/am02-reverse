import { PanelSection, PanelSectionRow, SliderField } from "@decky/ui";
import type { Translations } from "../i18n";

interface Props {
  valueMw: number;
  minMw: number;
  maxMw: number;
  active: boolean;
  t: Translations;
  onChange: (mw: number) => void;
}

/**
 * Manual TDP slider (mW internally, displayed in Watts). Can exceed the 45W
 * preset cap up to ~54W (ryzenadj limit). Gamepad: left/right adjusts, A
 * confirms focus entry — standard SliderField behavior, no mouse hover.
 */
export default function ManualTdpSlider({
  valueMw,
  minMw,
  maxMw,
  active,
  t,
  onChange,
}: Props) {
  const valueW = Math.round(valueMw / 1000);
  const minW = Math.round(minMw / 1000);
  const maxW = Math.round(maxMw / 1000);

  return (
    <PanelSection title={t.manualTdp}>
      <PanelSectionRow>
        <SliderField
          label={active ? t.manualTdpActive : t.manualTdp}
          value={valueW}
          min={minW}
          max={maxW}
          step={1}
          valueSuffix=" W"
          showValue
          notchTicksVisible
          validValues="range"
          bottomSeparator="none"
          onChange={(w) => onChange(Math.round(w) * 1000)}
        />
      </PanelSectionRow>
    </PanelSection>
  );
}
