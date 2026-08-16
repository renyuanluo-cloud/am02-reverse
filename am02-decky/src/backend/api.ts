import { call, callable } from "@decky/api";

export type TdpPresetId = "office" | "retro" | "classic" | "aaa";

export interface TdpPreset {
  id: TdpPresetId;
  label: string;
  stapm: number; // mW
  fast: number;
  slow: number;
}

export interface Settings {
  currentProfile: TdpPresetId | "manual";
  manualTdpMw: number;
  language: number; // 0 = Chinese, 1 = English
  theme: number; // 0 = black, 1 = white
  timeFormat: number; // 0 = 12h, 1 = 24h
  location: string; // weather city name; empty = weather off
  manualMinMw: number;
  manualMaxMw: number;
}

export const getSettings = callable<[], Settings>("get_settings");
export const getTdpProfiles = callable<[], TdpPreset[]>("get_tdp_profiles");

export const setTdpProfile = (id: string): Promise<boolean> =>
  call<[string], boolean>("set_tdp_profile", id);

export const setManualTdp = (mw: number): Promise<boolean> =>
  call<[number], boolean>("set_manual_tdp", mw);

export const setLanguage = (lang: number): Promise<boolean> =>
  call<[number], boolean>("set_language", lang);

export const setTheme = (theme: number): Promise<boolean> =>
  call<[number], boolean>("set_theme", theme);

export const setTimeFormat = (format: number): Promise<boolean> =>
  call<[number], boolean>("set_time_format", format);

export const setLocation = (city: string): Promise<boolean> =>
  call<[string], boolean>("set_location", city);

export interface CityHit {
  name: string;
  province: string;
  city: string; // 地级市（直辖市辖区/省直辖县为空）
  lat: number;
  lon: number;
}

export interface CitySearchResult {
  ok: boolean;
  cities: CityHit[];
}

export const searchCity = callable<[string], CitySearchResult>("search_city");
