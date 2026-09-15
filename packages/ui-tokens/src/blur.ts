export type BlurIntensity = "ultraThin" | "thin" | "regular" | "thick" | "chrome";

export const blurTokens = {
  ultraThin: {
    intensity: 20,
    tint: "default" as const,
  },
  thin: {
    intensity: 40,
    tint: "default" as const,
  },
  regular: {
    intensity: 65,
    tint: "default" as const,
  },
  thick: {
    intensity: 85,
    tint: "default" as const,
  },
  chrome: {
    intensity: 95,
    tint: "default" as const,
  },
};
