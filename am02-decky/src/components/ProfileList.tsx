import {
  ButtonItem,
  PanelSection,
  PanelSectionRow,
} from "@decky/ui";
import { BsCheck2, BsCircle } from "react-icons/bs";
import { TdpPreset, TdpPresetId } from "../backend/api";
import type { Translations } from "../i18n";

interface Props {
  presets: TdpPreset[];
  currentProfile: TdpPresetId | "manual";
  t: Translations;
  onSelect: (id: TdpPresetId) => void;
}

/**
 * Four AYASpace-style TDP presets as a gamepad-navigable single-select list.
 * Selected row shows a check icon; the rest show an empty circle. Built on
 * ButtonItem (Steam focus ring + A/OK activation), no hover-only interaction.
 *
 * 模式名用简短符号（多语言友好），功耗显示成小号纯数字 + 单位，避免长句。
 */
export default function ProfileList({ presets, currentProfile, t, onSelect }: Props) {
  return (
    <PanelSection title={t.tdpPresets}>
      {presets.map((p) => {
        const selected = p.id === currentProfile;
        return (
          <PanelSectionRow key={p.id}>
            <ButtonItem
              layout="below"
              onClick={() => onSelect(p.id)}
              icon={selected ? <BsCheck2 /> : <BsCircle />}
              highlightOnFocus
            >
              {t.profileNames[p.id]}
              <span
                style={{
                  fontSize: "0.82em",
                  opacity: 0.6,
                  marginLeft: "0.6em",
                  fontWeight: 400,
                }}
              >
                {Math.round(p.stapm / 1000)}W
              </span>
            </ButtonItem>
          </PanelSectionRow>
        );
      })}
    </PanelSection>
  );
}
