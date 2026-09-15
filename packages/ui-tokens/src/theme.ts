import { ThemeColors, lightColors, darkColors, oledColors } from "./colors";
import { typography, TextStyleToken } from "./typography";
import { radii } from "./radii";
import { spacing } from "./spacing";
import { shadows } from "./shadows";
import { blurTokens } from "./blur";
import { springs, durations } from "./animations";

export type ThemeMode = "light" | "dark" | "oled" | "system";

export interface AroTheme {
  isDark: boolean;
  isOled: boolean;
  colors: ThemeColors;
  typography: Record<string, TextStyleToken>;
  radii: typeof radii;
  spacing: typeof spacing;
  shadows: typeof shadows;
  blur: typeof blurTokens;
  springs: typeof springs;
  durations: typeof durations;
}

export function getAroTheme(mode: ThemeMode = "dark", systemIsDark = true): AroTheme {
  const resolvedMode: "light" | "dark" | "oled" =
    mode === "system" ? (systemIsDark ? "dark" : "light") : mode;

  const isDark = resolvedMode === "dark" || resolvedMode === "oled";
  const isOled = resolvedMode === "oled";

  const colors = resolvedMode === "oled"
    ? oledColors
    : resolvedMode === "dark"
    ? darkColors
    : lightColors;

  return {
    isDark,
    isOled,
    colors,
    typography,
    radii,
    spacing,
    shadows,
    blur: blurTokens,
    springs,
    durations,
  };
}
