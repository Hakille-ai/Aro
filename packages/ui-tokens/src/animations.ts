export interface SpringConfig {
  damping: number;
  mass: number;
  stiffness: number;
  overshootClamping?: boolean;
}

export const springs: Record<string, SpringConfig> = {
  /** Snappy and precise for buttons, switches, micro-interactions */
  snappy: {
    damping: 24,
    mass: 0.8,
    stiffness: 280,
    overshootClamping: true,
  },
  /** Standard fluid Apple spring for card expansion, navigation transitions */
  fluid: {
    damping: 20,
    mass: 1,
    stiffness: 180,
    overshootClamping: false,
  },
  /** Gentle spring for subtle fade-in, tooltip, or badge bounce */
  gentle: {
    damping: 28,
    mass: 1.2,
    stiffness: 140,
    overshootClamping: false,
  },
  /** Bouncy spring for celebrate, success icon checkmark */
  bouncy: {
    damping: 14,
    mass: 0.9,
    stiffness: 220,
    overshootClamping: false,
  },
  /** Sheet modal swipe-up and slide-down */
  sheet: {
    damping: 26,
    mass: 0.95,
    stiffness: 200,
    overshootClamping: true,
  },
};

export const durations = {
  instant: 100,
  fast: 200,
  normal: 300,
  deliberate: 450,
  modal: 350,
};
