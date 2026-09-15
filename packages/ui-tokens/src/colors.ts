export const palette = {
  black: "#000000",
  oled: "#050507",
  graphite950: "#0b0c0e",
  graphite900: "#111115",
  graphite850: "#16161c",
  graphite800: "#1c1d24",
  graphite700: "#272832",
  graphite600: "#383a48",
  graphite500: "#505364",

  slate50: "#f8fafc",
  slate100: "#f1f5f9",
  slate200: "#e2e8f0",
  slate300: "#cbd5e1",
  slate400: "#94a3b8",
  slate500: "#64748b",
  slate600: "#475569",
  slate700: "#334155",
  slate800: "#1e293b",
  slate900: "#0f172a",

  appleBlueLight: "#0071e3",
  appleBlueDark: "#0a84ff",
  appleIndigoLight: "#5856d6",
  appleIndigoDark: "#5e5ce6",
  applePurpleLight: "#af52de",
  applePurpleDark: "#bf5af2",
  appleGreenLight: "#34c759",
  appleGreenDark: "#30d158",
  appleOrangeLight: "#ff9500",
  appleOrangeDark: "#ff9f0a",
  appleRedLight: "#ff3b30",
  appleRedDark: "#ff453a",
  appleTealLight: "#30b0c7",
  appleTealDark: "#40c8e0",

  white: "#ffffff",
  pureWhite: "#ffffff",
};

export interface ThemeColors {
  background: {
    primary: string;
    secondary: string;
    tertiary: string;
    grouped: string;
    card: string;
    cardElevated: string;
    glass: string;
    overlay: string;
    input: string;
    active: string;
  };
  text: {
    primary: string;
    secondary: string;
    tertiary: string;
    quaternary: string;
    inverse: string;
    accent: string;
    error: string;
    success: string;
    warning: string;
  };
  border: {
    hairline: string;
    subtle: string;
    standard: string;
    active: string;
    focus: string;
  };
  accent: {
    primary: string;
    secondary: string;
    highlight: string;
    muted: string;
  };
  status: {
    pending: { bg: string; text: string; border: string; icon: string };
    inProgress: { bg: string; text: string; border: string; icon: string };
    completed: { bg: string; text: string; border: string; icon: string };
    error: { bg: string; text: string; border: string; icon: string };
  };
  diff: {
    addedBg: string;
    addedText: string;
    addedBorder: string;
    removedBg: string;
    removedText: string;
    removedBorder: string;
    contextBg: string;
  };
  code: {
    bg: string;
    headerBg: string;
    border: string;
    text: string;
    lineNumber: string;
  };
}

export const lightColors: ThemeColors = {
  background: {
    primary: "#f7f8fa",
    secondary: "#ffffff",
    tertiary: "#f0f2f5",
    grouped: "#eef0f4",
    card: "rgba(255, 255, 255, 0.85)",
    cardElevated: "#ffffff",
    glass: "rgba(255, 255, 255, 0.72)",
    overlay: "rgba(0, 0, 0, 0.35)",
    input: "rgba(0, 0, 0, 0.04)",
    active: "rgba(0, 113, 227, 0.08)",
  },
  text: {
    primary: "#1d1d1f",
    secondary: "#6e6e73",
    tertiary: "#86868b",
    quaternary: "#c7c7cc",
    inverse: "#ffffff",
    accent: palette.appleBlueLight,
    error: palette.appleRedLight,
    success: palette.appleGreenLight,
    warning: palette.appleOrangeLight,
  },
  border: {
    hairline: "rgba(0, 0, 0, 0.06)",
    subtle: "rgba(0, 0, 0, 0.08)",
    standard: "#e5e7eb",
    active: palette.appleBlueLight,
    focus: "rgba(0, 113, 227, 0.35)",
  },
  accent: {
    primary: palette.appleBlueLight,
    secondary: palette.appleIndigoLight,
    highlight: "#38bdf8",
    muted: "rgba(0, 113, 227, 0.12)",
  },
  status: {
    pending: {
      bg: "rgba(110, 110, 115, 0.1)",
      text: "#6e6e73",
      border: "rgba(110, 110, 115, 0.2)",
      icon: "#86868b",
    },
    inProgress: {
      bg: "rgba(0, 113, 227, 0.12)",
      text: palette.appleBlueLight,
      border: "rgba(0, 113, 227, 0.3)",
      icon: palette.appleBlueLight,
    },
    completed: {
      bg: "rgba(52, 199, 89, 0.12)",
      text: "#248a3d",
      border: "rgba(52, 199, 89, 0.3)",
      icon: palette.appleGreenLight,
    },
    error: {
      bg: "rgba(255, 59, 48, 0.12)",
      text: "#d70015",
      border: "rgba(255, 59, 48, 0.3)",
      icon: palette.appleRedLight,
    },
  },
  diff: {
    addedBg: "rgba(52, 199, 89, 0.14)",
    addedText: "#1b5e20",
    addedBorder: "rgba(52, 199, 89, 0.35)",
    removedBg: "rgba(255, 59, 48, 0.14)",
    removedText: "#b71c1c",
    removedBorder: "rgba(255, 59, 48, 0.35)",
    contextBg: "transparent",
  },
  code: {
    bg: "#f3f4f6",
    headerBg: "#e5e7eb",
    border: "#d1d5db",
    text: "#1f2937",
    lineNumber: "#9ca3af",
  },
};

export const darkColors: ThemeColors = {
  background: {
    primary: "#111115",
    secondary: "#16161c",
    tertiary: "#1c1d24",
    grouped: "#0e0e12",
    card: "rgba(24, 25, 32, 0.85)",
    cardElevated: "#21222b",
    glass: "rgba(22, 22, 28, 0.78)",
    overlay: "rgba(0, 0, 0, 0.65)",
    input: "rgba(255, 255, 255, 0.06)",
    active: "rgba(10, 132, 255, 0.15)",
  },
  text: {
    primary: "#f5f5f7",
    secondary: "#98989f",
    tertiary: "#6e6e73",
    quaternary: "#3a3a3c",
    inverse: "#000000",
    accent: palette.appleBlueDark,
    error: palette.appleRedDark,
    success: palette.appleGreenDark,
    warning: palette.appleOrangeDark,
  },
  border: {
    hairline: "rgba(255, 255, 255, 0.08)",
    subtle: "rgba(255, 255, 255, 0.12)",
    standard: "rgba(255, 255, 255, 0.18)",
    active: palette.appleBlueDark,
    focus: "rgba(10, 132, 255, 0.4)",
  },
  accent: {
    primary: palette.appleBlueDark,
    secondary: palette.appleIndigoDark,
    highlight: "#38bdf8",
    muted: "rgba(10, 132, 255, 0.18)",
  },
  status: {
    pending: {
      bg: "rgba(142, 142, 147, 0.14)",
      text: "#aeaea7",
      border: "rgba(142, 142, 147, 0.25)",
      icon: "#8e8e93",
    },
    inProgress: {
      bg: "rgba(10, 132, 255, 0.18)",
      text: "#64b5f6",
      border: "rgba(10, 132, 255, 0.35)",
      icon: palette.appleBlueDark,
    },
    completed: {
      bg: "rgba(48, 209, 88, 0.18)",
      text: "#81c784",
      border: "rgba(48, 209, 88, 0.35)",
      icon: palette.appleGreenDark,
    },
    error: {
      bg: "rgba(255, 69, 58, 0.18)",
      text: "#e57373",
      border: "rgba(255, 69, 58, 0.35)",
      icon: palette.appleRedDark,
    },
  },
  diff: {
    addedBg: "rgba(48, 209, 88, 0.18)",
    addedText: "#81c784",
    addedBorder: "rgba(48, 209, 88, 0.35)",
    removedBg: "rgba(255, 69, 58, 0.18)",
    removedText: "#e57373",
    removedBorder: "rgba(255, 69, 58, 0.35)",
    contextBg: "transparent",
  },
  code: {
    bg: "#0d0e12",
    headerBg: "#16171d",
    border: "rgba(255, 255, 255, 0.08)",
    text: "#e2e8f0",
    lineNumber: "#64748b",
  },
};

export const oledColors: ThemeColors = {
  ...darkColors,
  background: {
    ...darkColors.background,
    primary: "#000000",
    secondary: "#08080a",
    tertiary: "#101014",
    grouped: "#000000",
    card: "rgba(12, 12, 15, 0.95)",
    cardElevated: "#141418",
    glass: "rgba(6, 6, 8, 0.85)",
  },
  border: {
    hairline: "rgba(255, 255, 255, 0.1)",
    subtle: "rgba(255, 255, 255, 0.14)",
    standard: "rgba(255, 255, 255, 0.2)",
    active: palette.appleBlueDark,
    focus: "rgba(10, 132, 255, 0.45)",
  },
};
