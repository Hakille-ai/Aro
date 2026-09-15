export type MonitoringLanguage = "fr" | "en";

export interface PerformanceRecord {
  id: string;
  timestamp: string;
  conversationTitle: string;
  responseTime: number;
  tokens: number;
  speed: number;
}

export interface ActivityDay {
  dateStr: string;
  count: number;
  level: number;
  dayName: string;
  dateFormatted: string;
  monthLabel: string;
}

export interface MonitoringInitialState {
  cpu: number;
  ram: number;
  ramMax: number;
  gpu: number;
  gpuVram: number;
  gpuVramMax: number;
  cpuHistory: number[];
  ramHistory: number[];
  gpuHistory: number[];
}

export const MONITORING_HISTORY_LENGTH = 20;
export const PERFORMANCE_HISTORY_LIMIT = 50;

export function createInitialMonitoringState(): MonitoringInitialState {
  return {
    cpu: 4,
    ram: 4.2,
    ramMax: 16.0,
    gpu: 2,
    gpuVram: 2.4,
    gpuVramMax: 8.0,
    cpuHistory: Array(MONITORING_HISTORY_LENGTH).fill(4),
    ramHistory: Array(MONITORING_HISTORY_LENGTH).fill(4.2),
    gpuHistory: Array(MONITORING_HISTORY_LENGTH).fill(2),
  };
}

export function activityLevel(count: number): number {
  if (count <= 0) return 0;
  if (count <= 2) return 1;
  if (count <= 4) return 2;
  if (count <= 7) return 3;
  return 4;
}

export function populateMockActivity(
  now: Date = new Date(),
  random: () => number = Math.random,
): Record<string, number> {
  const log: Record<string, number> = {};
  for (let index = 0; index < 365; index += 1) {
    const date = new Date(now);
    date.setDate(now.getDate() - index);
    const dateStr = date.toISOString().split("T")[0];
    const dayOfWeek = date.getDay();
    const isWeekend = dayOfWeek === 0 || dayOfWeek === 6;
    const probability = isWeekend ? 0.2 : 0.65;
    if (random() < probability) {
      log[dateStr] = Math.floor(random() * 8) + 1;
    }
  }
  return log;
}

export function incrementActivityLog(
  activityLog: Record<string, number>,
  dateStr: string,
): Record<string, number> {
  return { ...activityLog, [dateStr]: (activityLog[dateStr] || 0) + 1 };
}

export function buildActivityWeeks(
  activityLog: Record<string, number>,
  language: MonitoringLanguage,
  now: Date = new Date(),
): ActivityDay[][] {
  const locale = language === "fr" ? "fr-FR" : "en-US";
  const currentDayOfWeek = now.getDay();
  const startDate = new Date(now);
  startDate.setDate(now.getDate() - 364 - currentDayOfWeek);

  const weeks: ActivityDay[][] = [];
  let currentWeek: ActivityDay[] = [];
  for (let index = 0; index < 371; index += 1) {
    const date = new Date(startDate);
    date.setDate(startDate.getDate() + index);
    const dateStr = date.toISOString().split("T")[0];
    const count = activityLog[dateStr] || 0;
    currentWeek.push({
      dateStr,
      count,
      level: activityLevel(count),
      dayName: date.toLocaleDateString(locale, { weekday: "short" }),
      dateFormatted: date.toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric" }),
      monthLabel: index % 7 === 0 && date.getDate() <= 7
        ? date.toLocaleDateString(locale, { month: "short" })
        : "",
    });
    if (currentWeek.length === 7) {
      weeks.push(currentWeek);
      currentWeek = [];
    }
  }
  return weeks;
}

export function createPerformanceRecord(
  conversationTitle: string,
  timeSec: number,
  tokens: number,
  language: MonitoringLanguage,
  now: Date = new Date(),
  id: string = globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).substring(2, 11),
): PerformanceRecord {
  const speed = timeSec > 0 ? Number((tokens / timeSec).toFixed(1)) : 0.0;
  return {
    id,
    timestamp: now.toLocaleTimeString(language === "fr" ? "fr-FR" : "en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
    conversationTitle: conversationTitle || (language === "fr" ? "Conversation sans titre" : "Untitled chat"),
    responseTime: Number(timeSec.toFixed(2)),
    tokens,
    speed,
  };
}

export function prependPerformanceRecord(
  history: PerformanceRecord[],
  record: PerformanceRecord,
): PerformanceRecord[] {
  return [record, ...history].slice(0, PERFORMANCE_HISTORY_LIMIT);
}

export function summarizePerformance(history: PerformanceRecord[]): {
  averageResponseTime: number;
  totalTokensGenerated: number;
} {
  const sumTime = history.reduce((total, record) => total + record.responseTime, 0);
  return {
    averageResponseTime: sumTime > 0 ? Number((sumTime / history.length).toFixed(2)) : 0.0,
    totalTokensGenerated: history.reduce((total, record) => total + record.tokens, 0),
  };
}
