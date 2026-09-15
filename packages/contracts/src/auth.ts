export interface User {
  id: string;
  email: string;
  fullName?: string | null;
  displayName?: string | null;
  avatarUrl?: string | null;
  createdAt: string;
  updatedAt?: string;
}

export interface CloudSessionView {
  userId: string;
  email: string;
  fullName?: string | null;
  organizationId?: string | null;
  organizationName?: string | null;
  role?: string;
}

export interface CloudLoginRequest {
  email: string;
  password?: string;
  totpCode?: string;
}

export interface CloudRegisterRequest {
  email: string;
  password?: string;
  fullName?: string;
  organizationName?: string;
}

export interface AuthTokens {
  accessToken: string;
  refreshToken: string;
  expiresIn?: number;
}

export interface AuthResponse {
  session: CloudSessionView;
  tokens: AuthTokens;
  mfaRequired?: boolean;
}

export interface TotpSetupResponse {
  secret: string;
  qrCodeUri: string;
  recoveryCodes?: string[];
}

export interface TotpVerifyRequest {
  code: string;
}

export interface TotpDisableRequest {
  code: string;
}

export interface UserProfilePatch {
  fullName?: string;
  displayName?: string;
  avatarUrl?: string;
}

export interface UserPreferences {
  theme: "light" | "dark" | "oled" | "system";
  accentColor?: string;
  fontSize?: number;
  soundEnabled?: boolean;
  hapticFeedbackEnabled?: boolean;
  voiceAutoPlay?: boolean;
  defaultModel?: string;
  streamResponses?: boolean;
}

export interface UserPreferencesPatch {
  theme?: "light" | "dark" | "oled" | "system";
  accentColor?: string;
  fontSize?: number;
  soundEnabled?: boolean;
  hapticFeedbackEnabled?: boolean;
  voiceAutoPlay?: boolean;
  defaultModel?: string;
  streamResponses?: boolean;
}
