import { type CSSProperties, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { animated, useSpring, useTransition } from "@react-spring/web";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

type Platform = "codex" | "cursor";

function CodexPlatformIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M9.205 8.658v-2.26c0-.19.072-.333.238-.428l4.543-2.616c.619-.357 1.356-.523 2.117-.523 2.854 0 4.662 2.212 4.662 4.566 0 .167 0 .357-.024.547l-4.71-2.759a.797.797 0 0 0-.856 0l-5.97 3.473zm10.609 8.8V12.06c0-.333-.143-.57-.429-.737l-5.97-3.473 1.95-1.118a.433.433 0 0 1 .476 0l4.543 2.617c1.309.76 2.189 2.378 2.189 3.948 0 1.808-1.07 3.473-2.76 4.163zM7.802 12.703l-1.95-1.142c-.167-.095-.239-.238-.239-.428V5.899c0-2.545 1.95-4.472 4.591-4.472 1 0 1.927.333 2.712.928L8.23 5.067c-.285.166-.428.404-.428.737v6.898zM12 15.128l-2.795-1.57v-3.33L12 8.658l2.795 1.57v3.33L12 15.128zm1.796 7.23c-1 0-1.927-.332-2.712-.927l4.686-2.712c.285-.166.428-.404.428-.737v-6.898l1.974 1.142c.167.095.238.238.238.428v5.233c0 2.545-1.974 4.472-4.614 4.472zm-5.637-5.303-4.544-2.617c-1.308-.761-2.188-2.378-2.188-3.948A4.482 4.482 0 0 1 4.21 6.327v5.423c0 .333.143.571.428.738l5.947 3.449-1.95 1.118a.432.432 0 0 1-.476 0zm-.262 3.9c-2.688 0-4.662-2.021-4.662-4.519 0-.19.024-.38.047-.57l4.686 2.71c.286.167.571.167.856 0l5.97-3.448v2.26c0 .19-.07.333-.237.428l-4.543 2.616c-.619.357-1.356.523-2.117.523zm5.899 2.83a5.947 5.947 0 0 0 5.827-4.756C22.287 18.339 24 15.84 24 13.296c0-1.665-.713-3.282-1.998-4.448.119-.5.19-.999.19-1.498 0-3.401-2.759-5.947-5.946-5.947-.642 0-1.26.095-1.88.31A5.962 5.962 0 0 0 10.205 0a5.947 5.947 0 0 0-5.827 4.757C1.713 5.447 0 7.945 0 10.49c0 1.666.713 3.283 1.998 4.448-.119.5-.19 1-.19 1.499 0 3.401 2.759 5.946 5.946 5.946.642 0 1.26-.095 1.88-.309A5.96 5.96 0 0 0 13.796 23.9z" />
    </svg>
  );
}

function CursorPlatformIcon() {
  return (
    <svg viewBox="0 0 466.73 532.09" aria-hidden="true" focusable="false">
      <path d="M457.43,125.94L244.42,2.96c-6.84-3.95-15.28-3.95-22.12,0L9.3,125.94c-5.75,3.32-9.3,9.46-9.3,16.11v247.99c0,6.65,3.55,12.79,9.3,16.11l213.01,122.98c6.84,3.95,15.28,3.95,22.12,0l213.01-122.98c5.75-3.32,9.3-9.46,9.3-16.11v-247.99c0-6.65-3.55-12.79-9.3-16.11h-.01ZM444.05,151.99l-205.63,356.16c-1.39,2.4-5.06,1.42-5.06-1.36v-233.21c0-4.66-2.49-8.97-6.53-11.31L24.87,145.67c-2.4-1.39-1.42-5.06,1.36-5.06h411.26c5.84,0,9.49,6.33,6.57,11.39h-.01Z" />
    </svg>
  );
}

type RateWindow = { usedPercent: number; windowDurationMins: number; resetsAt: number };
type UsageSnapshot = {
  email: string | null;
  planType: string | null;
  primary: RateWindow | null;
  secondary: RateWindow | null;
  primaryLabel?: string;
  secondaryLabel?: string;
  creditBalance: string | null;
  hasCredits: boolean;
  unlimited: boolean;
  resetCredits: number;
  fetchedAt: number;
};
type TokenUsageStats = {
  totalTokens: number;
  todayTokens: number;
  sessionsScanned: number;
  unreadableSessions: number;
  available: boolean;
  updatedAt: number;
};
type CursorQuotaSnapshot = {
  status: "connected" | "notConfigured" | "unauthorized" | "unavailable";
  message: string | null;
  email: string | null;
  membershipType: string | null;
  cursorModelsPercent: number | null;
  otherModelsPercent: number | null;
  totalPercent: number | null;
  planUsedUsd: number | null;
  planLimitUsd: number | null;
  planRemainingUsd: number | null;
  onDemandUsedUsd: number | null;
  onDemandLimitUsd: number | null;
  onDemandRemainingUsd: number | null;
  requestsUsed: number | null;
  requestsLimit: number | null;
  billingCycleEnd: string | null;
  fetchedAt: number;
};
type CursorTokenUsageStats = {
  totalTokens: number;
  todayTokens: number;
  eventsScanned: number;
  cacheAvailable: boolean;
  syncPerformed: boolean;
  syncError: string | null;
  updatedAt: number | null;
};
type MainQuotaUpdate =
  | { platform: "codex"; snapshot: UsageSnapshot }
  | { platform: "cursor"; snapshot: CursorQuotaSnapshot };
type MainQuotaSyncStatus = { platform: Platform; loading: boolean; error: string | null };
type FloatingSettings = {
  activePlatform: Platform;
  visible: boolean;
  pinned: boolean;
  opacity: number;
  alwaysOnTop: boolean;
  style: "card" | "orb";
  orbExpandDirection: "auto" | "left" | "right";
  orbSize: number;
  cardScale: number;
  orbWaveSpeed: number;
  orbWaveAmplitude: number;
  syncIntervalSecs: number;
  displayMode: "available" | "used";
  theme: "obsidian" | "titanium" | "spruce" | "dusk" | "abyss" | "cashmere" | "cinnabar" | "cyber";
  proxyMode: "system" | "none" | "custom";
  proxyAddress: string;
  dataDirectory: string;
};
type OrbDragResult = { moved: boolean; atEdge: boolean; side: "left" | "right" };
type OrbPointerPress = {
  pointerId: number;
  startX: number;
  startY: number;
  cursorStart: ReturnType<typeof cursorPosition>;
};

const MIN_ORB_WAVE_AMPLITUDE = 0.5;
const MAX_ORB_WAVE_AMPLITUDE = 4;
const MIN_ORB_SIZE = 48;
const MAX_ORB_SIZE = 88;
const MIN_CARD_SCALE = 80;
const MAX_CARD_SCALE = 140;
const MIN_SYNC_INTERVAL_SECS = 5;
const MAX_SYNC_INTERVAL_SECS = 86_400;
const LIQUID_WAVE_NODE_COUNT = 33;
const LIQUID_FIXED_STEP = 1 / 60;
const LIQUID_WAVE_VISUAL_GAIN = 0.38;

function usePageVisibility() {
  const [visible, setVisible] = useState(() => document.visibilityState === "visible");

  useEffect(() => {
    const updateVisibility = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", updateVisibility);
    updateVisibility();
    return () => document.removeEventListener("visibilitychange", updateVisibility);
  }, []);

  return visible;
}

function PrettyScroller({ children, active = true }: { children: ReactNode; active?: boolean }) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState({ top: 0, height: 40, visible: false });

  const update = useCallback(() => {
    const el = viewportRef.current;
    if (!el) return;
    const overflow = el.scrollHeight - el.clientHeight;
    if (overflow <= 2) {
      setThumb((value) => (value.visible ? { ...value, visible: false } : value));
      return;
    }
    const height = Math.max(36, (el.clientHeight / el.scrollHeight) * el.clientHeight);
    const top = (el.scrollTop / overflow) * (el.clientHeight - height);
    setThumb({ top, height, visible: true });
  }, []);

  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    update();
    const resize = new ResizeObserver(update);
    resize.observe(el);
    const mutate = new MutationObserver(update);
    mutate.observe(el, { childList: true, subtree: true, attributes: true });
    return () => {
      resize.disconnect();
      mutate.disconnect();
    };
  }, [update]);

  useEffect(() => {
    const frame = window.requestAnimationFrame(update);
    const timer = window.setTimeout(update, 40);
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [active, update]);

  return (
    <div className="pretty-scroll">
      <div ref={viewportRef} className="pretty-scroll__viewport" onScroll={update}>
        {children}
      </div>
      <div className="pretty-scroll__rail" aria-hidden="true">
        {thumb.visible && <i className="pretty-scroll__thumb" style={{ height: `${thumb.height}px`, transform: `translateY(${thumb.top}px)` }} />}
      </div>
    </div>
  );
}

function liquidSurfacePath(level: number, displacement: Float32Array, amplitude: number) {
  const safeLevel = clamp(level);
  const baseY = 200 * (1 - safeLevel / 100);
  const edgeScale = Math.min(1, safeLevel / 12, (100 - safeLevel) / 12);
  const scale = Math.max(0, edgeScale) * amplitude * LIQUID_WAVE_VISUAL_GAIN;
  const spacing = 200 / (displacement.length - 1);
  const yAt = (index: number) => baseY + displacement[index] * scale;
  const format = (value: number) => Math.round(value * 100) / 100;

  let path = `M0 ${format(yAt(0))}`;
  for (let index = 0; index < displacement.length - 1; index += 1) {
    const previous = Math.max(0, index - 1);
    const next = index + 1;
    const afterNext = Math.min(displacement.length - 1, index + 2);
    const x0 = previous * spacing;
    const x1 = index * spacing;
    const x2 = next * spacing;
    const x3 = afterNext * spacing;
    const y0 = yAt(previous);
    const y1 = yAt(index);
    const y2 = yAt(next);
    const y3 = yAt(afterNext);
    const cp1x = x1 + (x2 - x0) / 6;
    const cp2x = x2 - (x3 - x1) / 6;
    const cp1y = y1 + (y2 - y0) / 6;
    const cp2y = y2 - (y3 - y1) / 6;
    path += ` C${format(cp1x)} ${format(cp1y)},${format(cp2x)} ${format(cp2y)},${format(x2)} ${format(y2)}`;
  }
  return `${path} L200 212 H0 Z`;
}

// Keep the surface position continuous: impulses change liquid velocity, not
// displacement, so the waterline cannot teleport by several pixels in one frame.
function addLiquidVelocityImpulse(velocity: Float32Array, center: number, strength: number, spread: number) {
  let mean = 0;
  for (let index = 0; index < velocity.length; index += 1) {
    const distance = (index - center) / spread;
    mean += Math.exp(-0.5 * distance * distance);
  }
  mean /= velocity.length;

  for (let index = 0; index < velocity.length; index += 1) {
    const distance = (index - center) / spread;
    velocity[index] += strength * (Math.exp(-0.5 * distance * distance) - mean);
  }
}

function stepLiquidWave(displacement: Float32Array, velocity: Float32Array, next: Float32Array, speed: number) {
  const stiffness = 30 * speed * speed;
  const damping = 0.35 + 0.12 * speed;
  let meanDisplacement = 0;
  let meanVelocity = 0;

  for (let index = 0; index < displacement.length; index += 1) {
    const left = displacement[index === 0 ? 1 : index - 1];
    const right = displacement[index === displacement.length - 1 ? index - 1 : index + 1];
    const acceleration = stiffness * (left + right - 2 * displacement[index]) - damping * velocity[index];
    velocity[index] += acceleration * LIQUID_FIXED_STEP;
    next[index] = displacement[index] + velocity[index] * LIQUID_FIXED_STEP;
    meanDisplacement += next[index];
    meanVelocity += velocity[index];
  }

  meanDisplacement /= displacement.length;
  meanVelocity /= velocity.length;
  for (let index = 0; index < displacement.length; index += 1) {
    displacement[index] = next[index] - meanDisplacement;
    velocity[index] -= meanVelocity;
  }
}

function SpringLiquid({ level, speed, amplitude, dragging, running }: { level: number; speed: number; amplitude: number; dragging: boolean; running: boolean }) {
  const pathRef = useRef<SVGPathElement>(null);
  const speedRef = useRef(Math.min(3, Math.max(0.5, speed)));
  const amplitudeRef = useRef(Math.min(MAX_ORB_WAVE_AMPLITUDE, Math.max(MIN_ORB_WAVE_AMPLITUDE, amplitude)));
  const levelSpring = useSpring({ value: clamp(level), config: { mass: 1.8, tension: 45, friction: 22 } });
  const levelSpringRef = useRef(levelSpring.value);
  const pendingImpulse = useRef(0);
  const previousTargetLevel = useRef(clamp(level));
  const wasDragging = useRef(dragging);

  speedRef.current = Math.min(3, Math.max(0.5, speed));
  amplitudeRef.current = Math.min(MAX_ORB_WAVE_AMPLITUDE, Math.max(MIN_ORB_WAVE_AMPLITUDE, amplitude));
  levelSpringRef.current = levelSpring.value;

  useEffect(() => {
    const nextLevel = clamp(level);
    const change = nextLevel - previousTargetLevel.current;
    if (Math.abs(change) >= 0.5) pendingImpulse.current += Math.sign(change) * Math.min(1.5, Math.abs(change) * 0.08);
    previousTargetLevel.current = nextLevel;
  }, [level]);

  useEffect(() => {
    if (dragging !== wasDragging.current) pendingImpulse.current += dragging ? 1.2 : -0.55;
    wasDragging.current = dragging;
  }, [dragging]);

  useEffect(() => {
    const pathElement = pathRef.current;
    if (!pathElement || !running) return;

    const displacement = new Float32Array(LIQUID_WAVE_NODE_COUNT);
    const velocity = new Float32Array(LIQUID_WAVE_NODE_COUNT);
    const next = new Float32Array(LIQUID_WAVE_NODE_COUNT);
    let frameId = 0;
    let previousTime = 0;
    let accumulator = 0;
    let nextAmbientImpulse = 260;

    const animate = (time: number) => {
      if (previousTime === 0) previousTime = time;
      accumulator += Math.min(0.05, Math.max(0, (time - previousTime) / 1000));
      previousTime = time;

      while (accumulator >= LIQUID_FIXED_STEP) {
        const disturbance = pendingImpulse.current;
        pendingImpulse.current = 0;
        if (Math.abs(disturbance) > 0.01) {
          const center = 3 + Math.random() * (LIQUID_WAVE_NODE_COUNT - 6);
          addLiquidVelocityImpulse(velocity, center, disturbance * 8.7 * speedRef.current, 1.7);
        }

        if (time >= nextAmbientImpulse) {
          const center = 3 + Math.random() * (LIQUID_WAVE_NODE_COUNT - 6);
          const sign = Math.random() < 0.5 ? -1 : 1;
          const strength = sign * (40 + Math.random() * 15) * speedRef.current;
          const spread = 2 + Math.random() * 1.5;
          addLiquidVelocityImpulse(velocity, center, strength, spread);
          nextAmbientImpulse = time + 1550 + Math.random() * 1200;
        }

        stepLiquidWave(displacement, velocity, next, speedRef.current);
        accumulator -= LIQUID_FIXED_STEP;
      }

      pathElement.setAttribute("d", liquidSurfacePath(levelSpringRef.current.get(), displacement, amplitudeRef.current));
      frameId = window.requestAnimationFrame(animate);
    };

    frameId = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(frameId);
  }, [running]);

  return (
    <span className="orb-liquid" aria-hidden="true">
      <svg className="orb-liquid__surface" viewBox="0 0 200 200" preserveAspectRatio="none">
        <path ref={pathRef} d={liquidSurfacePath(level, new Float32Array(LIQUID_WAVE_NODE_COUNT), 1)} />
      </svg>
    </span>
  );
}

const defaultFloatingSettings: FloatingSettings = {
  activePlatform: "codex",
  visible: false,
  pinned: false,
  opacity: 0.92,
  alwaysOnTop: true,
  style: "card",
  orbExpandDirection: "auto",
  orbSize: 56,
  cardScale: 100,
  orbWaveSpeed: 2,
  orbWaveAmplitude: 1,
  syncIntervalSecs: 60,
  displayMode: "available",
  theme: "obsidian",
  proxyMode: "system",
  proxyAddress: "",
  dataDirectory: "data",
};

async function reserveQuotaSync(force: boolean): Promise<number> {
  if (!("__TAURI_INTERNALS__" in window)) return 0;
  try {
    return await invoke<number>("begin_quota_sync", { force });
  } catch {
    // Keep the UI usable during a frontend/backend version mismatch.
    return 0;
  }
}

function isAppStateNotReadyError(reason: unknown) {
  return String(reason).toLowerCase().includes("state not managed");
}

function publishMainQuotaUpdate(update: MainQuotaUpdate) {
  if ("__TAURI_INTERNALS__" in window) void emit("main-quota-updated", update).catch(() => undefined);
}

function publishMainQuotaSyncStatus(status: MainQuotaSyncStatus) {
  if ("__TAURI_INTERNALS__" in window) void emit("main-quota-sync-status", status).catch(() => undefined);
}

function scheduleQuotaSyncRetry(
  timerRef: { current: number | undefined },
  waitMs: number,
  shouldRun: () => boolean,
  isBusy: () => boolean,
  run: () => void,
) {
  if (timerRef.current !== undefined) window.clearTimeout(timerRef.current);

  const retry = () => {
    timerRef.current = undefined;
    if (!shouldRun()) return;
    if (isBusy()) {
      timerRef.current = window.setTimeout(retry, 500);
      return;
    }
    run();
  };

  timerRef.current = window.setTimeout(retry, Math.max(35, waitMs + 35));
}

function localDayUnixBounds() {
  const now = new Date();
  return {
    todayStart: new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1_000,
    tomorrowStart: new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime() / 1_000,
  };
}

async function refreshPlatformTokenData(platform: Platform): Promise<TokenUsageStats | CursorTokenUsageStats> {
  const { todayStart, tomorrowStart } = localDayUnixBounds();
  if (platform === "cursor") {
    return invoke<CursorTokenUsageStats>("get_cursor_token_usage_stats", { todayStart, tomorrowStart });
  }
  return invoke<TokenUsageStats>("get_token_usage_stats", { todayStart, tomorrowStart });
}

const themes: { id: FloatingSettings["theme"]; label: string; name: string; colors: string[] }[] = [
  { id: "obsidian", label: "曜石", name: "曜石紫罗兰", colors: ["#121113", "#1A191B", "#232225", "#3C393F", "#EEEEF0", "#B5B2BC", "#BAA7FF", "#D0C3FF", "#323035", "#BAA7FF"] },
  { id: "titanium", label: "翡翠", name: "翡翠岩青", colors: ["#101211", "#171918", "#202221", "#373B39", "#ECEEED", "#ADB5B2", "#1FD8A4", "#67E4C1", "#2E3130", "#1FD8A4"] },
  { id: "spruce", label: "深海", name: "深海幽蓝", colors: ["#111113", "#18191B", "#212225", "#363A3F", "#EDEEF0", "#B0B4BA", "#70B8FF", "#9ECFFF", "#2E3135", "#70B8FF"] },
  { id: "dusk", label: "暮色", name: "暮色玫瑰", colors: ["#121113", "#1A191B", "#232225", "#3C393F", "#EEEEF0", "#B5B2BC", "#FF8DCC", "#FFB1DC", "#323035", "#FF8DCC"] },
  { id: "abyss", label: "暗夜", name: "暗夜宝石红", colors: ["#121113", "#1A191B", "#232225", "#3C393F", "#EEEEF0", "#B5B2BC", "#FF949D", "#FFB6BC", "#323035", "#FF949D"] },
  { id: "cashmere", label: "铂银", name: "铂银极简", colors: ["#111111", "#191919", "#222222", "#3A3A3A", "#EEEEEE", "#B4B4B4", "#B4B4B4", "#CCCCCC", "#313131", "#B4B4B4"] },
  { id: "cinnabar", label: "琥珀", name: "琥珀熔金", colors: ["#111110", "#191918", "#222221", "#3B3A37", "#EEEEEC", "#B5B3AD", "#FFCA16", "#FFDB61", "#31312E", "#FFCA16"] },
  { id: "cyber", label: "冰川", name: "冰川暮青", colors: ["#111113", "#18191B", "#212225", "#363A3F", "#EDEEF0", "#B0B4BA", "#4CCCE6", "#85DCEE", "#2E3135", "#4CCCE6"] },
];

function initialFloatingSettings(): FloatingSettings {
  const style = new URLSearchParams(window.location.search).get("style");
  return { ...defaultFloatingSettings, style: style === "orb" ? "orb" : "card" };
}

const demoSnapshot: UsageSnapshot = {
  email: "you@example.com",
  planType: "plus",
  primary: { usedPercent: 28, windowDurationMins: 300, resetsAt: Date.now() / 1000 + 9240 },
  secondary: { usedPercent: 61, windowDurationMins: 10080, resetsAt: Date.now() / 1000 + 342000 },
  creditBalance: "0",
  hasCredits: false,
  unlimited: false,
  resetCredits: 0,
  fetchedAt: Date.now() / 1000,
};

const clamp = (value: number) => Math.min(100, Math.max(0, value));

function cursorQuotaAsUsage(quota: CursorQuotaSnapshot): UsageSnapshot {
  const requestPercent = quota.requestsUsed != null && quota.requestsLimit != null && quota.requestsLimit > 0
    ? quota.requestsUsed / quota.requestsLimit * 100
    : null;
  const primaryPercent = quota.cursorModelsPercent ?? quota.totalPercent ?? requestPercent;
  const parsedResetAt = quota.billingCycleEnd ? Date.parse(quota.billingCycleEnd) / 1000 : Number.NaN;
  const resetAt = Number.isFinite(parsedResetAt) ? parsedResetAt : quota.fetchedAt;
  const rate = (usedPercent: number | null): RateWindow | null => usedPercent == null
    ? null
    : { usedPercent: clamp(usedPercent), windowDurationMins: 0, resetsAt: resetAt };
  const dollars = (value: number | null) => value == null
    ? null
    : new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(value);

  return {
    email: quota.email,
    planType: quota.membershipType,
    primary: rate(primaryPercent),
    secondary: rate(quota.otherModelsPercent),
    primaryLabel: quota.cursorModelsPercent != null ? "Cursor 模型" : quota.totalPercent != null ? "方案用量" : "Cursor 模型",
    secondaryLabel: "其他模型",
    creditBalance: dollars(quota.planRemainingUsd),
    hasCredits: quota.planRemainingUsd != null && quota.planRemainingUsd > 0,
    unlimited: false,
    resetCredits: 0,
    fetchedAt: quota.fetchedAt,
  };
}

function formatDuration(minutes: number) {
  if (minutes >= 10080) return `${Math.round(minutes / 10080)} 周`;
  if (minutes >= 1440) return `${Math.round(minutes / 1440)} 天`;
  if (minutes >= 60) return `${Math.round(minutes / 60)} 小时`;
  return `${minutes} 分钟`;
}

function formatRemaining(timestamp: number, nowMs: number) {
  const target = new Date(timestamp * 1000);
  const totalMinutes = Math.ceil((target.getTime() - nowMs) / 60_000);
  if (totalMinutes <= 0) return "即将重置";
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  return days > 0 ? `${days} 天 ${hours} 小时` : hours > 0 ? `${hours} 小时 ${minutes} 分` : `${minutes} 分钟`;
}

function formatResetAt(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    timeZoneName: "short",
  });
}

function formatTokenCount(value: number) {
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 0 }).format(value);
}

function planLabel(plan: string | null) {
  if (!plan || plan === "unknown") return "ChatGPT 账户";
  return `${plan.charAt(0).toUpperCase()}${plan.slice(1)} 计划`;
}

function metricValue(usedPercent: number, mode: FloatingSettings["displayMode"]) {
  return clamp(mode === "used" ? usedPercent : 100 - usedPercent);
}

function metricLabel(mode: FloatingSettings["displayMode"]) {
  return mode === "used" ? "已使用" : "可用";
}

function oppositeDisplayMode(mode: FloatingSettings["displayMode"]): FloatingSettings["displayMode"] {
  return mode === "used" ? "available" : "used";
}

function Gauge({ usedPercent, mode, compact = false }: { usedPercent: number; mode: FloatingSettings["displayMode"]; compact?: boolean }) {
  const safeValue = metricValue(usedPercent, mode);
  const radius = compact ? 54 : 72;
  const circumference = 2 * Math.PI * radius;
  return (
    <div className={`gauge ${compact ? "gauge--compact" : ""}`} aria-label={`${metricLabel(mode)} ${Math.round(safeValue)}%`}>
      <svg viewBox="0 0 180 180" role="img">
        <circle className="gauge__track" cx="90" cy="90" r={radius} />
        <circle className="gauge__value" cx="90" cy="90" r={radius} style={{ strokeDasharray: circumference, strokeDashoffset: circumference * (1 - safeValue / 100) }} />
      </svg>
      <div className="gauge__number"><strong>{Math.round(safeValue)}</strong><span>% {metricLabel(mode)}</span></div>
    </div>
  );
}

function startNativeDrag(event: ReactMouseEvent, enabled = true) {
  if (!enabled || event.button !== 0 || !("__TAURI_INTERNALS__" in window)) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input")) return;
  void getCurrentWindow().startDragging();
}

function MainTitlebar() {
  const minimize = () => "__TAURI_INTERNALS__" in window && void getCurrentWindow().minimize();
  const maximize = () => "__TAURI_INTERNALS__" in window && void getCurrentWindow().toggleMaximize();
  const close = () => "__TAURI_INTERNALS__" in window && void getCurrentWindow().close();
  return <div className="main-titlebar" onMouseDown={(event) => startNativeDrag(event)}>
    <div className="main-titlebar__title"><i /> AI Usage Meter</div>
    <div className="main-titlebar__actions">
      <button onClick={minimize} aria-label="最小化"><svg viewBox="0 0 12 12"><path d="M2 8.5h8" /></svg></button>
      <button onClick={maximize} aria-label="最大化或还原"><svg viewBox="0 0 12 12"><rect x="2.5" y="2.5" width="7" height="7" /></svg></button>
      <button className="is-close" onClick={close} aria-label="关闭"><svg viewBox="0 0 12 12"><path d="m2.5 2.5 7 7m0-7-7 7" /></svg></button>
    </div>
  </div>;
}

function FloatingApp() {
  const windowStyle: FloatingSettings["style"] = new URLSearchParams(window.location.search).get("style") === "orb" ? "orb" : "card";
  const pageIsVisible = usePageVisibility();
  const [codexUsage, setCodexUsage] = useState<UsageSnapshot | null>(null);
  const [cursorUsage, setCursorUsage] = useState<UsageSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [nowMs, setNowMs] = useState(Date.now());
  const [settings, setSettings] = useState(initialFloatingSettings);
  const usage = settings.activePlatform === "cursor" ? cursorUsage : codexUsage;
  const activePlatformRef = useRef(settings.activePlatform);
  activePlatformRef.current = settings.activePlatform;
  const [orbExpanded, setOrbExpanded] = useState(() => new URLSearchParams(window.location.search).get("expanded") === "1");
  const [orbSide, setOrbSide] = useState<"left" | "right">("right");
  const [orbAtEdge, setOrbAtEdge] = useState(false);
  const [orbReady, setOrbReady] = useState(() => !("__TAURI_INTERNALS__" in window));
  const [orbDraggingVisual, setOrbDraggingVisual] = useState(false);
  const orbDragging = useRef(false);
  const orbWasExpanded = useRef(false);
  const orbHoverTimer = useRef<number | undefined>(undefined);
  const orbTransitionToken = useRef(0);
  const orbPointerPress = useRef<OrbPointerPress | null>(null);
  const orbSuppressHoverUntil = useRef(0);

  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme;
  }, [settings.theme]);

  const loadPlatformCache = useCallback(async (platform: Platform) => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      if (platform === "cursor") {
        const cached = await invoke<CursorQuotaSnapshot | null>("get_cached_cursor_quota");
        if (cached?.status === "connected") {
          const normalized = cursorQuotaAsUsage(cached);
          setCursorUsage((current) => !current || normalized.fetchedAt >= current.fetchedAt ? normalized : current);
        } else if (activePlatformRef.current === platform) {
          setCursorUsage(null);
          setError(true);
        }
      } else {
        const cached = await invoke<UsageSnapshot | null>("get_cached_usage");
        if (cached) setCodexUsage((current) => !current || cached.fetchedAt >= current.fetchedAt ? cached : current);
      }
    } catch {
      // A missing cache is fine; the main window will publish its next sync.
    } finally {
      if (activePlatformRef.current === platform) setLoading(false);
    }
  }, []);

  const requestMainQuotaRefresh = useCallback(async () => {
    if (!("__TAURI_INTERNALS__" in window)) {
      if (settings.activePlatform === "cursor") setCursorUsage(demoSnapshot);
      else setCodexUsage(demoSnapshot);
      setError(false);
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(false);
    try {
      await emit("floating-quota-refresh-requested");
    } catch {
      setLoading(false);
      setError(true);
    }
  }, [settings.activePlatform]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setLoading(false);
      return;
    }
    let active = true;
    const unlisteners: Array<() => void> = [];
    invoke<FloatingSettings>("get_floating_settings").then((savedSettings) => {
      activePlatformRef.current = savedSettings.activePlatform;
      setSettings(savedSettings);
      void loadPlatformCache(savedSettings.activePlatform);
    }).catch(() => undefined);
    invoke<"left" | "right">("get_floating_orb_side").then(setOrbSide).catch(() => undefined);
    void Promise.all([
      listen<FloatingSettings>("floating-settings-changed", (event) => {
        activePlatformRef.current = event.payload.activePlatform;
        setSettings(event.payload);
        setLoading(false);
        setError(false);
        void loadPlatformCache(event.payload.activePlatform);
      }),
      listen<MainQuotaUpdate>("main-quota-updated", (event) => {
        const update = event.payload;
        if (update.platform === "codex") {
          setCodexUsage((current) => !current || update.snapshot.fetchedAt >= current.fetchedAt ? update.snapshot : current);
          if (activePlatformRef.current === "codex") setError(false);
        } else {
          const snapshot = update.snapshot;
          if (snapshot.status === "connected") {
            const normalized = cursorQuotaAsUsage(snapshot);
            setCursorUsage((current) => !current || normalized.fetchedAt >= current.fetchedAt ? normalized : current);
          } else {
            setCursorUsage(null);
          }
          if (activePlatformRef.current === "cursor") setError(snapshot.status !== "connected");
        }
        if (activePlatformRef.current === update.platform) setLoading(false);
      }),
      listen<MainQuotaSyncStatus>("main-quota-sync-status", (event) => {
        if (activePlatformRef.current !== event.payload.platform) return;
        setLoading(event.payload.loading);
        setError(Boolean(event.payload.error));
      }),
    ]).then((stops) => {
      if (!active) {
        stops.forEach((stop) => stop());
        return;
      }
      unlisteners.push(...stops);
      void loadPlatformCache("codex");
      void loadPlatformCache("cursor");
    }).catch(() => undefined);
    return () => {
      active = false;
      if (orbHoverTimer.current !== undefined) window.clearTimeout(orbHoverTimer.current);
      unlisteners.forEach((stop) => stop());
    };
  }, [loadPlatformCache]);

  useEffect(() => {
    if (!pageIsVisible || !settings.visible) return;
    setNowMs(Date.now());
    const clockTimer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(clockTimer);
  }, [pageIsVisible, settings.visible]);

  useEffect(() => {
    if (!pageIsVisible || !("__TAURI_INTERNALS__" in window)) return;
    invoke<FloatingSettings>("get_floating_settings").then((savedSettings) => {
      activePlatformRef.current = savedSettings.activePlatform;
      setSettings(savedSettings);
      void loadPlatformCache(savedSettings.activePlatform);
    }).catch(() => undefined);
  }, [loadPlatformCache, pageIsVisible]);

  useEffect(() => {
    if (windowStyle !== "orb" || settings.style !== "orb" || !("__TAURI_INTERNALS__" in window)) return;
    invoke<"left" | "right">("get_floating_orb_side").then(setOrbSide).catch(() => undefined);
  }, [windowStyle, settings.style, settings.orbExpandDirection]);

  useEffect(() => {
    if (windowStyle !== "orb") {
      document.documentElement.classList.remove("orb-window-blurred");
      return;
    }
    const applyBlurred = (blurred: boolean) => {
      document.documentElement.classList.toggle("orb-window-blurred", blurred);
    };
    const syncBlurClass = () => applyBlurred(!document.hasFocus());
    syncBlurClass();
    window.addEventListener("blur", syncBlurClass);
    window.addEventListener("focus", syncBlurClass);
    let unlisten: (() => void) | undefined;
    if ("__TAURI_INTERNALS__" in window) {
      void listen<boolean>("orb-window-focus", (event) => {
        applyBlurred(!event.payload);
      }).then((fn) => {
        unlisten = fn;
      });
    }
    return () => {
      window.removeEventListener("blur", syncBlurClass);
      window.removeEventListener("focus", syncBlurClass);
      unlisten?.();
      document.documentElement.classList.remove("orb-window-blurred");
    };
  }, [windowStyle]);

  useEffect(() => {
    if (windowStyle !== "orb" || settings.style !== "orb" || !("__TAURI_INTERNALS__" in window)) return;
    let active = true;
    setOrbReady(false);
    void (async () => {
      try {
        const atEdge = await invoke<boolean>("get_floating_orb_edge_state");
        const targetExpanded = !atEdge;
        if (!active || orbDragging.current) return;
        setOrbAtEdge(atEdge);
        const side = await invoke<"left" | "right">("get_floating_orb_side");
        if (!active || orbDragging.current) return;
        setOrbSide(side);
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
        if (!active || orbDragging.current) return;
        const settledSide = await invoke<"left" | "right">("set_floating_orb_expanded", { expanded: targetExpanded });
        if (active) {
          setOrbSide(settledSide);
          setOrbExpanded(targetExpanded);
          setOrbReady(true);
        }
      } catch {
        if (active) setOrbReady(true);
      }
    })();
    return () => { active = false; };
  }, [windowStyle, settings.style]);

  const hide = () => {
    if ("__TAURI_INTERNALS__" in window) void invoke("set_floating_window", { visible: false });
  };

  const togglePin = async () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const pinned = await invoke<boolean>("set_floating_pinned", { pinned: !settings.pinned });
    setSettings((value) => ({ ...value, pinned }));
  };

  const setOrbOpen = async (expanded: boolean) => {
    if (windowStyle !== "orb") return;
    if (orbDragging.current || orbPointerPress.current) return;
    if (orbExpanded === expanded) return;
    const token = ++orbTransitionToken.current;
    if (!("__TAURI_INTERNALS__" in window)) {
      setOrbExpanded(expanded);
      return;
    }
    try {
      if (expanded) {
        const atEdge = await invoke<boolean>("get_floating_orb_edge_state");
        if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
        setOrbAtEdge(atEdge);
        const side = await invoke<"left" | "right">("get_floating_orb_side");
        if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
        setOrbSide(side);
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
        if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
        const settledSide = await invoke<"left" | "right">("set_floating_orb_expanded", { expanded: true });
        if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
        setOrbSide(settledSide);
        setOrbExpanded(true);
      } else {
        if ("__TAURI_INTERNALS__" in window) {
          const atEdge = await invoke<boolean>("get_floating_orb_edge_state");
          if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
          setOrbAtEdge(atEdge);
          if (!atEdge) return;
        }
        setOrbExpanded(false);
        await new Promise<void>((resolve) => window.setTimeout(resolve, 200));
        if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
        if ("__TAURI_INTERNALS__" in window) {
          const stillAtEdge = await invoke<boolean>("get_floating_orb_edge_state");
          if (token !== orbTransitionToken.current || orbDragging.current || orbPointerPress.current) return;
          if (!stillAtEdge) {
            setOrbAtEdge(false);
            setOrbExpanded(true);
            return;
          }
          setOrbAtEdge(true);
        }
        const side = await invoke<"left" | "right">("set_floating_orb_expanded", { expanded: false });
        if (token === orbTransitionToken.current && !orbDragging.current && !orbPointerPress.current) setOrbSide(side);
      }
    } catch {
      if (token === orbTransitionToken.current) setOrbExpanded(!expanded);
    }
  };

  const scheduleOrbOpen = (expanded: boolean) => {
    if (orbHoverTimer.current !== undefined) window.clearTimeout(orbHoverTimer.current);
    orbHoverTimer.current = window.setTimeout(async () => {
      orbHoverTimer.current = undefined;
      if (orbPointerPress.current || orbDragging.current) return;
      if (!expanded && "__TAURI_INTERNALS__" in window) {
        try {
          if (await invoke<boolean>("get_floating_orb_pointer_inside")) return;
        } catch { /* Continue closing if the window is unavailable. */ }
      }
      void setOrbOpen(expanded);
    }, expanded ? 35 : 115);
  };

  const scheduleOrbOpenRef = useRef(scheduleOrbOpen);
  scheduleOrbOpenRef.current = scheduleOrbOpen;

  useEffect(() => {
    if (windowStyle !== "orb" || settings.style !== "orb" || !orbReady || !("__TAURI_INTERNALS__" in window)) return;
    let stop: (() => void) | undefined;
    void listen("floating-orb-hover-entered", () => scheduleOrbOpenRef.current(true))
      .then((unlisten) => { stop = unlisten; });
    return () => stop?.();
  }, [orbReady, settings.style, windowStyle]);

  const finishOrbDrag = useCallback((result: OrbDragResult) => {
    if (!orbDragging.current) return;
    void (async () => {
      const token = ++orbTransitionToken.current;
      // Compact only when the orb was docked to an edge; otherwise the whole
      // widget stays expanded at the dropped position.
      const expand = !result.atEdge;
      setOrbSide(result.side);
      setOrbAtEdge(result.atEdge);
      orbSuppressHoverUntil.current = Date.now() + 280;
      await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      if (token !== orbTransitionToken.current) return;
      const side = await invoke<"left" | "right">("set_floating_orb_expanded", { expanded: expand });
      if (token === orbTransitionToken.current) {
        setOrbSide(side);
        setOrbExpanded(expand);
      }
    })().catch(() => {
      setOrbExpanded(false);
    }).finally(() => {
      // Even a stale transition or failed native call must not leave drag
      // guards enabled forever.
      orbDragging.current = false;
      setOrbDraggingVisual(false);
    });
  }, []);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let stopDragEnded: (() => void) | undefined;
    void listen<OrbDragResult>("floating-orb-drag-ended", (event) => {
      finishOrbDrag(event.payload);
    }).then((stop) => { stopDragEnded = stop; });
    return () => {
      stopDragEnded?.();
    };
  }, [finishOrbDrag]);

  const beginOrbDrag = async (press: OrbPointerPress) => {
    if (orbDragging.current || !("__TAURI_INTERNALS__" in window)) return;
    orbPointerPress.current = null;
    if (orbHoverTimer.current !== undefined) window.clearTimeout(orbHoverTimer.current);
    orbHoverTimer.current = undefined;
    ++orbTransitionToken.current;
    orbDragging.current = true;
    orbWasExpanded.current = orbExpanded;
    setOrbExpanded(false);
    setOrbDraggingVisual(true);
    try {
      const dragStart = await press.cursorStart;
      await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      await invoke("start_floating_orb_drag", { cursorStartX: dragStart.x, cursorStartY: dragStart.y });
    } catch {
      orbDragging.current = false;
      setOrbExpanded(orbWasExpanded.current);
      setOrbDraggingVisual(false);
    }
  };

  const handleOrbPointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || !orbReady || !("__TAURI_INTERNALS__" in window) || orbDragging.current) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    // Cancel any hover resize already in flight before the native drag can
    // change the window bounds.
    ++orbTransitionToken.current;
    if (orbHoverTimer.current !== undefined) window.clearTimeout(orbHoverTimer.current);
    orbHoverTimer.current = undefined;
    orbPointerPress.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      cursorStart: cursorPosition(),
    };
  };

  const handleOrbPointerMove = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const press = orbPointerPress.current;
    if (!press || press.pointerId !== event.pointerId) return;
    const deltaX = event.clientX - press.startX;
    const deltaY = event.clientY - press.startY;
    if (deltaX * deltaX + deltaY * deltaY < 16) return;
    void beginOrbDrag(press);
  };

  const handleOrbPointerUp = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const press = orbPointerPress.current;
    if (!press || press.pointerId !== event.pointerId) return;
    orbPointerPress.current = null;
    // A click should settle on the open state (and cancel a close transition
    // that may have been interrupted by the press).
    scheduleOrbOpen(true);
  };

  const handleOrbPointerCancel = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (orbPointerPress.current?.pointerId === event.pointerId) {
      orbPointerPress.current = null;
      ++orbTransitionToken.current;
      if (orbHoverTimer.current !== undefined) window.clearTimeout(orbHoverTimer.current);
      orbHoverTimer.current = undefined;
    }
  };

  const floatingBackgroundStyle = { backgroundColor: `rgb(var(--surface-rgb) / ${settings.opacity})` } as CSSProperties;
  const floatingWindowStyle = {
    "--orb-size": `${settings.orbSize}px`,
    "--floating-card-scale": settings.cardScale / 100,
  } as CSSProperties;
  const primaryMetric = metricValue(usage?.primary?.usedPercent ?? 0, settings.displayMode);
  const primaryWater = clamp(primaryMetric);
  const secondaryMetric = metricValue(usage?.secondary?.usedPercent ?? 0, settings.displayMode);
  const primaryLabel = usage?.primaryLabel ?? (usage?.primary ? formatDuration(usage.primary.windowDurationMins) : "5 小时");
  const secondaryLabel = usage?.secondaryLabel ?? (usage?.secondary?.windowDurationMins && usage.secondary.windowDurationMins >= 10080 ? "本周" : usage?.secondary ? formatDuration(usage.secondary.windowDurationMins) : "本周");
  const platformLabel = settings.activePlatform === "cursor" ? "Cursor" : "Codex";

  if (windowStyle === "orb") {
    return (
      <main
        className={`floating-shell floating-shell--orb is-${orbSide} ${orbAtEdge ? "is-docked" : ""} ${orbExpanded ? "is-expanded" : ""} ${orbDraggingVisual ? "is-dragging" : ""}`}
        style={floatingWindowStyle}
        onMouseEnter={() => { if (orbReady && Date.now() >= orbSuppressHoverUntil.current && !orbDragging.current && !orbPointerPress.current) scheduleOrbOpen(true); }}
        onMouseLeave={() => { if (orbReady && !orbDragging.current && !orbPointerPress.current) scheduleOrbOpen(false); }}
      >
        <div className="floating-background" style={floatingBackgroundStyle} aria-hidden="true" />
        <div className={`orb-teaser ${orbExpanded ? "is-visible" : ""}`} aria-hidden={!orbExpanded} aria-label={`${platformLabel} 额度摘要`}>
          <div className="orb-teaser__quotas">
            <div className="orb-quota-row">
              <div className="orb-quota-row__heading"><span>{primaryLabel}额度 · {metricLabel(settings.displayMode)}</span><strong>{Math.round(primaryMetric)}<small>%</small></strong></div>
              <div className="orb-teaser__track"><i style={{ width: `${primaryMetric}%` }} /></div>
              <b>{usage?.primary ? formatRemaining(usage.primary.resetsAt, nowMs) : "--"}</b>
            </div>
            <div className="orb-quota-row orb-quota-row--secondary">
              <div className="orb-quota-row__heading"><span>{secondaryLabel}额度 · {metricLabel(settings.displayMode)}</span><strong>{Math.round(secondaryMetric)}<small>%</small></strong></div>
              <div className="orb-teaser__track"><i style={{ width: `${secondaryMetric}%` }} /></div>
              <b>{usage?.secondary ? formatRemaining(usage.secondary.resetsAt, nowMs) : "--"}</b>
            </div>
          </div>
        </div>
        <button
          className="orb-trigger"
          onPointerDown={handleOrbPointerDown}
          onPointerMove={handleOrbPointerMove}
          onPointerUp={handleOrbPointerUp}
          onPointerCancel={handleOrbPointerCancel}
          aria-label={`${platformLabel} 额度悬浮球`}
        >
          <SpringLiquid level={primaryWater} speed={settings.orbWaveSpeed} amplitude={settings.orbWaveAmplitude} dragging={orbDraggingVisual} running={settings.visible && settings.style === "orb" && pageIsVisible} />
          <span className={`orb-core ${Math.round(primaryMetric) >= 100 ? "is-three-digit" : ""}`} aria-label={`${Math.round(primaryMetric)}%`}>
            <strong>{Math.round(primaryMetric)}</strong><small>%</small>
          </span>
        </button>
      </main>
    );
  }

  return (
    <main className="floating-shell floating-shell--card" style={floatingWindowStyle}>
      <div className="floating-background" style={floatingBackgroundStyle} aria-hidden="true" />
      <header className="floating-header" onMouseDown={(event) => startNativeDrag(event, !settings.pinned)}>
        <div className="floating-brand"><i /> AI USAGE METER</div>
        <div className="floating-actions">
          <button className={settings.pinned ? "is-active" : ""} onClick={togglePin} aria-label={settings.pinned ? "取消固定" : "固定并启用鼠标穿透"} title={settings.pinned ? "取消固定" : "固定并启用鼠标穿透"}>
            <svg viewBox="0 0 24 24"><path d="m8 4 8 8M14 3l7 7-4 1-4 4-1 4-7-7 4-1 4-4 1-4ZM5 19l4-4" /></svg>
          </button>
          <button onClick={() => void requestMainQuotaRefresh()} disabled={loading} aria-label="刷新额度" title="刷新额度">
            <svg className={loading ? "is-spinning" : ""} viewBox="0 0 24 24"><path d="M20 7v5h-5M4 17v-5h5M6.1 8.1A7 7 0 0 1 18.6 7M17.9 15.9A7 7 0 0 1 5.4 17" /></svg>
          </button>
          <button onClick={hide} aria-label="隐藏悬浮窗" title="隐藏悬浮窗">
            <svg viewBox="0 0 24 24"><path d="M6 12h12" /></svg>
          </button>
        </div>
      </header>

      <section className="floating-meters">
        <div className="floating-meter">
          <span>{primaryLabel} · {metricLabel(settings.displayMode)}</span>
          <strong>{Math.round(primaryMetric)}<small>%</small></strong>
          <div className="floating-track"><i style={{ width: `${primaryMetric}%` }} /></div>
          <b>{usage?.primary ? formatRemaining(usage.primary.resetsAt, nowMs) : "--"}</b>
        </div>
        <div className="floating-divider" />
        <div className="floating-meter">
          <span>{secondaryLabel} · {metricLabel(settings.displayMode)}</span>
          <strong>{Math.round(secondaryMetric)}<small>%</small></strong>
          <div className="floating-track floating-track--coral"><i style={{ width: `${secondaryMetric}%` }} /></div>
          <b>{usage?.secondary ? formatRemaining(usage.secondary.resetsAt, nowMs) : "--"}</b>
        </div>
      </section>

      <footer className="floating-footer" onMouseDown={(event) => startNativeDrag(event, !settings.pinned)}>
        <span className={error ? "is-error" : ""}><i /> {error ? "同步失败" : loading ? "同步中" : "额度可用"}</span>
        <span>{settings.pinned ? "已固定 · 鼠标穿透" : `拖动移动 · ${settings.alwaysOnTop ? "始终置顶" : "普通层级"}`}</span>
      </footer>
    </main>
  );
}

function TokenOverview({
  stats,
  loading,
  error,
  previewOnly,
  onRefresh,
  active = true,
}: {
  stats: TokenUsageStats | null;
  loading: boolean;
  error: string | null;
  previewOnly: boolean;
  onRefresh: () => void;
  active?: boolean;
}) {
  const updatedAt = stats?.updatedAt
    ? new Date(stats.updatedAt * 1000).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false })
    : "尚未读取";
  const totalTokens = stats?.available ? formatTokenCount(stats.totalTokens) : "—";
  const todayTokens = stats?.available ? formatTokenCount(stats.todayTokens) : "—";
  return (
    <section className="token-view" id={active ? "token-view" : undefined} role="tabpanel" aria-labelledby="main-view-tab-tokens">
      <header className="token-view__heading">
        <div>
          <span className="eyebrow">LOCAL TOKEN LEDGER / 01</span>
          <h1 id="token-view-title">Token 视角</h1>
          <p>统计当前所选平台的累计与当日 Token 消耗。</p>
        </div>
        <button className="token-refresh" onClick={onRefresh} disabled={loading}>
          <svg className={loading ? "is-spinning" : ""} viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5M6.1 8.1A7 7 0 0 1 18.6 7M17.9 15.9A7 7 0 0 1 5.4 17" /></svg>
          {loading ? "正在更新…" : "刷新统计"}
        </button>
      </header>

      <div className="token-stat-grid" aria-live="polite" aria-busy={loading}>
        <article className="token-stat-card token-stat-card--total">
          <div className="token-stat-card__top"><span>累计消耗</span><span>ALL TIME</span></div>
          <div className="token-stat-card__value">{totalTokens}</div>
          <div className="token-stat-card__unit">TOKENS <i /></div>
          <div className="token-stat-card__index" aria-hidden="true">Σ</div>
          <p>本机统计到的累计 Token 用量</p>
        </article>
        <article className="token-stat-card token-stat-card--today">
          <div className="token-stat-card__top"><span>今日消耗</span><span>LOCAL DAY</span></div>
          <div className="token-stat-card__value">{todayTokens}</div>
          <div className="token-stat-card__unit">TOKENS <i /></div>
          <p>按本机时区的今日 00:00 起统计</p>
        </article>
      </div>

      <div className="token-data-strip">
        <div className="token-data-strip__source"><span className="token-data-strip__mark">Σ</span><div><strong>本机 Token 账本</strong><span>仅用于 Token 用量统计</span></div></div>
        <div className="token-data-strip__meta"><span>已统计记录</span><strong>{stats?.available ? `${formatTokenCount(stats.sessionsScanned)} 条` : "—"}</strong></div>
        <div className="token-data-strip__meta"><span>最近读取</span><strong>{updatedAt}</strong></div>
      </div>

      {previewOnly && <p className="token-status-note">Token 统计仅在桌面版中读取；浏览器预览不会访问本机数据。</p>}
      {!previewOnly && error && <p className="token-status-note token-status-note--error">读取失败：{error}</p>}
      {!previewOnly && !loading && stats?.available === false && <p className="token-status-note">当前暂无可统计的 Token 数据，请确认平台已登录并产生使用记录后再刷新。</p>}
      {!previewOnly && !loading && stats?.available && stats.sessionsScanned === 0 && <p className="token-status-note">当前还没有可统计的记录。</p>}
      {!previewOnly && !loading && stats && stats.unreadableSessions > 0 && <p className="token-status-note">有 {formatTokenCount(stats.unreadableSessions)} 个日志文件暂时无法读取，其余记录已纳入统计。</p>}
      <p className="token-view__footnote">数据范围：当前所选平台在本机可取得的 Token 记录；数据来源与同步状态见上方账本信息。</p>
    </section>
  );
}

function DashboardApp() {
  const pageIsVisible = usePageVisibility();
  const isDesktopApp = "__TAURI_INTERNALS__" in window;
  const [mainView, setMainView] = useState<"quota" | "tokens">("quota");
  const [activePlatform, setActivePlatform] = useState<Platform>(() => {
    try {
      const saved = window.localStorage.getItem("ai-usage-meter-platform") ?? window.localStorage.getItem("quota-meter-platform");
      return saved === "cursor" ? "cursor" : "codex";
    } catch {
      return "codex";
    }
  });
  const [sidebarExpanded, setSidebarExpanded] = useState(() => {
    try {
      const saved = window.localStorage.getItem("ai-usage-meter-sidebar-collapsed") ?? window.localStorage.getItem("quota-meter-sidebar-collapsed");
      return saved === null ? window.innerWidth > 700 : saved !== "true";
    } catch {
      return window.innerWidth > 700;
    }
  });
  const [usage, setUsage] = useState<UsageSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [demoMode, setDemoMode] = useState(false);
  const [nowMs, setNowMs] = useState(Date.now());
  const [floatingSettings, setFloatingSettings] = useState(defaultFloatingSettings);
  const [floatingSettingsLoaded, setFloatingSettingsLoaded] = useState(!isDesktopApp);
  const [startupSyncStarted, setStartupSyncStarted] = useState(!isDesktopApp);
  const [showFloatingControls, setShowFloatingControls] = useState(false);
  const [proxyAddressDraft, setProxyAddressDraft] = useState("");
  const [syncIntervalDraft, setSyncIntervalDraft] = useState(String(defaultFloatingSettings.syncIntervalSecs));
  const [syncIntervalError, setSyncIntervalError] = useState<string | null>(null);
  const [tokenStats, setTokenStats] = useState<TokenUsageStats | null>(null);
  const [tokenStatsLoading, setTokenStatsLoading] = useState(isDesktopApp);
  const [tokenStatsError, setTokenStatsError] = useState<string | null>(null);
  const [cursorQuota, setCursorQuota] = useState<CursorQuotaSnapshot | null>(null);
  const [cursorTokenStats, setCursorTokenStats] = useState<CursorTokenUsageStats | null>(null);
  const [cursorQuotaLoading, setCursorQuotaLoading] = useState(isDesktopApp);
  const [cursorTokenStatsLoading, setCursorTokenStatsLoading] = useState(isDesktopApp);
  const [cursorQuotaError, setCursorQuotaError] = useState<string | null>(null);
  const [cursorTokenStatsError, setCursorTokenStatsError] = useState<string | null>(null);
  const startupSyncStartedRef = useRef(!isDesktopApp);
  const observedPlatformRef = useRef<Platform | null>(null);
  const observedVisibilityRef = useRef<boolean | null>(null);
  const observedTokenViewRef = useRef({ view: mainView, visible: pageIsVisible });
  const refreshInFlight = useRef(false);
  const tokenScanInFlight = useRef(false);
  const tokenScanQueued = useRef(false);
  const cursorRefreshInFlight = useRef(false);
  const cursorTokenScanInFlight = useRef(false);
  const cursorTokenScanQueued = useRef(false);
  const codexTokenRefreshRef = useRef<() => Promise<void>>(async () => undefined);
  const quotaOnlyRefreshRef = useRef<() => void>(() => undefined);
  const autoRetryTimer = useRef<number | undefined>(undefined);
  const activePlatformRef = useRef(activePlatform);
  const autoRefreshRef = useRef<() => void>(() => undefined);
  const settingsWrapRef = useRef<HTMLDivElement>(null);
  activePlatformRef.current = activePlatform;
  const sceneKey = `${activePlatform}:${mainView}`;
  const sceneTransition = useTransition(sceneKey, {
    keys: sceneKey,
    from: { opacity: 0, transform: "translate3d(0, 18px, 0) scale(0.975)" },
    enter: { opacity: 1, transform: "translate3d(0, 0px, 0) scale(1)" },
    leave: { opacity: 0, transform: "translate3d(0, -14px, 0) scale(0.99)" },
    config: { tension: 280, friction: 32 },
  });

  useEffect(() => {
    try {
      window.localStorage.setItem("ai-usage-meter-platform", activePlatform);
    } catch {
      // The selection remains usable if browser storage is unavailable.
    }
  }, [activePlatform]);

  useEffect(() => {
    try {
      window.localStorage.setItem("ai-usage-meter-sidebar-collapsed", String(!sidebarExpanded));
    } catch {
      // The sidebar remains controllable if browser storage is unavailable.
    }
  }, [sidebarExpanded]);

  const selectPlatform = async (platform: Platform) => {
    const previousPlatform = activePlatform;
    setActivePlatform(platform);
    setFloatingSettings((value) => ({ ...value, activePlatform: platform }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const savedPlatform = await invoke<Platform>("set_active_platform", { platform });
      setActivePlatform(savedPlatform);
    } catch (reason) {
      setActivePlatform(previousPlatform);
      setFloatingSettings((value) => ({ ...value, activePlatform: previousPlatform }));
      setError(String(reason));
    }
  };

  useEffect(() => {
    document.documentElement.dataset.theme = floatingSettings.theme;
  }, [floatingSettings.theme]);

  const refresh = useCallback(async (force = false, includeTokenStats = false) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    let syncError: string | null = null;
    try {
      if ("__TAURI_INTERNALS__" in window) {
        const waitMs = await reserveQuotaSync(force);
        if (waitMs > 0) {
          scheduleQuotaSyncRetry(
            autoRetryTimer,
            waitMs,
            () => true,
            () => refreshInFlight.current || cursorRefreshInFlight.current,
            () => autoRefreshRef.current(),
          );
          return;
        }
      }
      if (!force && autoRetryTimer.current !== undefined) {
        window.clearTimeout(autoRetryTimer.current);
        autoRetryTimer.current = undefined;
      }
      setLoading(true);
      setError(null);
      publishMainQuotaSyncStatus({ platform: "codex", loading: true, error: null });
      if (!("__TAURI_INTERNALS__" in window)) {
        setDemoMode(true);
        setUsage(demoSnapshot);
        if (includeTokenStats) {
          setTokenStats(null);
          setTokenStatsError(null);
        }
      } else {
        setDemoMode(false);
        if (includeTokenStats) void codexTokenRefreshRef.current();
        const snapshot = await invoke<UsageSnapshot>("get_codex_usage");
        setUsage(snapshot);
        publishMainQuotaUpdate({ platform: "codex", snapshot });
      }
    } catch (reason) {
      syncError = String(reason);
      setError(syncError);
    } finally {
      refreshInFlight.current = false;
      setLoading(false);
      publishMainQuotaSyncStatus({ platform: "codex", loading: false, error: syncError });
    }
  }, []);

  const refreshTokenStats = useCallback(async (queueIfBusy = false) => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setTokenStats(null);
      setTokenStatsError(null);
      return;
    }
    if (tokenScanInFlight.current) {
      if (queueIfBusy) tokenScanQueued.current = true;
      return;
    }

    tokenScanInFlight.current = true;
    setTokenStatsLoading(true);
    setTokenStatsError(null);
    const { todayStart, tomorrowStart } = localDayUnixBounds();
    try {
      setTokenStats(await invoke<TokenUsageStats>("get_token_usage_stats", { todayStart, tomorrowStart }));
    } catch (reason) {
      setTokenStatsError(String(reason));
    } finally {
      tokenScanInFlight.current = false;
      if (tokenScanQueued.current) {
        tokenScanQueued.current = false;
        void refreshTokenStats();
      } else {
        setTokenStatsLoading(false);
      }
    }
  }, []);

  codexTokenRefreshRef.current = refreshTokenStats;

  const refreshCursorTokenStats = useCallback(async (queueIfBusy = false) => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setCursorTokenStats(null);
      setCursorTokenStatsError(null);
      return;
    }
    if (cursorTokenScanInFlight.current) {
      if (queueIfBusy) cursorTokenScanQueued.current = true;
      return;
    }
    cursorTokenScanInFlight.current = true;
    setCursorTokenStatsLoading(true);
    setCursorTokenStatsError(null);
    try {
      const stats = (await refreshPlatformTokenData("cursor")) as CursorTokenUsageStats;
      setCursorTokenStats(stats);
    } catch (reason) {
      setCursorTokenStatsError(isAppStateNotReadyError(reason) ? null : String(reason));
    } finally {
      cursorTokenScanInFlight.current = false;
      if (cursorTokenScanQueued.current) {
        cursorTokenScanQueued.current = false;
        void refreshCursorTokenStats();
      } else {
        setCursorTokenStatsLoading(false);
      }
    }
  }, []);

  const refreshActiveTokenStats = useCallback(async (queueIfBusy = true) => {
    if (activePlatformRef.current === "cursor") await refreshCursorTokenStats(queueIfBusy);
    else await refreshTokenStats(queueIfBusy);
  }, [refreshCursorTokenStats, refreshTokenStats]);

  const refreshCursorOverview = useCallback(async (force = false, includeTokenStats = true) => {
    if (!("__TAURI_INTERNALS__" in window) || cursorRefreshInFlight.current) return;
    cursorRefreshInFlight.current = true;
    let syncError: string | null = null;
    try {
      const waitMs = await reserveQuotaSync(force);
      if (waitMs > 0) {
        scheduleQuotaSyncRetry(
          autoRetryTimer,
          waitMs,
          () => true,
          () => refreshInFlight.current || cursorRefreshInFlight.current,
          () => autoRefreshRef.current(),
        );
        return;
      }
      if (!force && autoRetryTimer.current !== undefined) {
        window.clearTimeout(autoRetryTimer.current);
        autoRetryTimer.current = undefined;
      }
      setCursorQuotaLoading(true);
      setCursorQuotaError(null);
      publishMainQuotaSyncStatus({ platform: "cursor", loading: true, error: null });
      const quotaRequest = invoke<CursorQuotaSnapshot>("get_cursor_quota")
        .then((snapshot) => {
          setCursorQuota(snapshot);
          publishMainQuotaUpdate({ platform: "cursor", snapshot });
          if (snapshot.status !== "connected") syncError = snapshot.message ?? "Cursor 额度不可用";
        })
        .catch((reason) => {
          if (isAppStateNotReadyError(reason)) {
            // Tauri can start the initial webview before setup has finished
            // registering AppState. Keep this transient race out of the
            // account error UI and retry through the normal sync path.
            setCursorQuotaError(null);
            scheduleQuotaSyncRetry(
              autoRetryTimer,
              350,
              () => true,
              () => refreshInFlight.current || cursorRefreshInFlight.current,
              () => autoRefreshRef.current(),
            );
          } else {
            syncError = String(reason);
            setCursorQuotaError(syncError);
          }
        })
        .finally(() => {
          setCursorQuotaLoading(false);
          publishMainQuotaSyncStatus({ platform: "cursor", loading: false, error: syncError });
        });
      if (includeTokenStats) void refreshCursorTokenStats();
      await quotaRequest;
    } finally {
      cursorRefreshInFlight.current = false;
    }
  }, [refreshCursorTokenStats]);

  autoRefreshRef.current = () => {
    if (activePlatformRef.current === "cursor") void refreshCursorOverview(false, true);
    else void refresh(false, true);
  };

  useEffect(() => {
    if (isDesktopApp && floatingSettingsLoaded) {
      invoke<UsageSnapshot | null>("get_cached_usage").then((cached) => {
        if (cached) setUsage((current) => !current || cached.fetchedAt >= current.fetchedAt ? cached : current);
      }).catch(() => undefined);
      invoke<CursorQuotaSnapshot | null>("get_cached_cursor_quota").then((cached) => {
        if (cached?.status === "connected") {
          setCursorQuota((current) => !current || cached.fetchedAt >= current.fetchedAt ? cached : current);
        }
      }).catch(() => undefined);
    }
  }, [floatingSettingsLoaded, isDesktopApp]);

  useEffect(() => {
    if (!pageIsVisible || mainView !== "quota") return;
    setNowMs(Date.now());
    const clockTimer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(clockTimer);
  }, [activePlatform, mainView, pageIsVisible]);

  useEffect(() => {
    if (!floatingSettingsLoaded || !isDesktopApp || startupSyncStartedRef.current) return;
    startupSyncStartedRef.current = true;
    setStartupSyncStarted(true);
    // On every app launch, hydrate both platforms regardless of the selected
    // platform or view. Startup sync deliberately bypasses the normal cooldown;
    // later platform switches and interval refreshes still use it.
    void Promise.all([
      refresh(true, false),
      refreshCursorOverview(true, false),
      refreshTokenStats(),
      refreshCursorTokenStats(),
    ]).catch((reason) => setError(String(reason)));
  }, [floatingSettingsLoaded, isDesktopApp, refresh, refreshCursorOverview, refreshCursorTokenStats, refreshTokenStats]);

  useEffect(() => {
    if (!floatingSettingsLoaded) return;
    const previousPlatform = observedPlatformRef.current;
    const previousVisibility = observedVisibilityRef.current;
    observedPlatformRef.current = activePlatform;
    observedVisibilityRef.current = pageIsVisible;

    if (previousPlatform === null) {
      if (!isDesktopApp && pageIsVisible && activePlatform === "codex") void refresh(false, true);
      return;
    }
    if (isDesktopApp && !startupSyncStarted) return;
    const platformChanged = previousPlatform !== activePlatform;
    const becameVisible = previousVisibility === false && pageIsVisible;
    if (!pageIsVisible || (!platformChanged && !becameVisible)) return;
    if (activePlatform === "cursor") void refreshCursorOverview(false, true);
    else void refresh(false, true);
  }, [activePlatform, floatingSettingsLoaded, isDesktopApp, pageIsVisible, refresh, refreshCursorOverview, startupSyncStarted]);

  useEffect(() => {
    const previous = observedTokenViewRef.current;
    observedTokenViewRef.current = { view: mainView, visible: pageIsVisible };
    if (!floatingSettingsLoaded || !pageIsVisible || mainView !== "tokens") return;
    if (previous.view === "tokens" && previous.visible) return;
    if (isDesktopApp && !startupSyncStarted) return;
    void refreshActiveTokenStats(true);
  }, [floatingSettingsLoaded, isDesktopApp, mainView, pageIsVisible, refreshActiveTokenStats, startupSyncStarted]);

  useEffect(() => {
    if (isDesktopApp && (!floatingSettingsLoaded || !startupSyncStarted)) return;
    if (activePlatform !== "codex") return;
    const refreshTimer = window.setInterval(() => void refresh(false, true), floatingSettings.syncIntervalSecs * 1_000);
    return () => window.clearInterval(refreshTimer);
  }, [activePlatform, floatingSettings.syncIntervalSecs, floatingSettingsLoaded, isDesktopApp, refresh, startupSyncStarted]);

  useEffect(() => {
    if (isDesktopApp && (!floatingSettingsLoaded || !startupSyncStarted)) return;
    if (activePlatform !== "cursor") return;
    const refreshTimer = window.setInterval(() => void refreshCursorOverview(), floatingSettings.syncIntervalSecs * 1_000);
    return () => window.clearInterval(refreshTimer);
  }, [activePlatform, floatingSettings.syncIntervalSecs, floatingSettingsLoaded, isDesktopApp, refreshCursorOverview, startupSyncStarted]);

  useEffect(() => () => {
    if (autoRetryTimer.current !== undefined) window.clearTimeout(autoRetryTimer.current);
  }, []);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let unlisten: (() => void) | undefined;
    void listen("floating-quota-refresh-requested", () => quotaOnlyRefreshRef.current())
      .then((stop) => { unlisten = stop; });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let active = true;
    let settingsReady = false;
    let retryTimer: number | undefined;
    let retryDelayMs = 150;
    let unlisten: (() => void) | undefined;
    void listen<FloatingSettings>("floating-settings-changed", (event) => {
      settingsReady = true;
      if (retryTimer !== undefined) {
        window.clearTimeout(retryTimer);
        retryTimer = undefined;
      }
      setFloatingSettings(event.payload);
      setActivePlatform(event.payload.activePlatform);
      setSyncIntervalDraft((draft) => document.activeElement?.id === "sync-interval" ? draft : String(event.payload.syncIntervalSecs));
      setFloatingSettingsLoaded(true);
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    }).catch(() => undefined);

    const loadSettings = async () => {
      try {
        const settings = await invoke<FloatingSettings>("get_floating_settings");
        if (!active || settingsReady) return;
        settingsReady = true;
        setFloatingSettings(settings);
        setActivePlatform(settings.activePlatform);
        setProxyAddressDraft(settings.proxyAddress);
        setSyncIntervalDraft(String(settings.syncIntervalSecs));
        setFloatingSettingsLoaded(true);
      } catch {
        if (!active || settingsReady) return;
        // Keep startup sync gated until the backend is genuinely ready. The
        // initial webview can race Tauri setup, so quietly retry with backoff.
        retryTimer = window.setTimeout(() => {
          retryDelayMs = Math.min(retryDelayMs * 2, 2_000);
          void loadSettings();
        }, retryDelayMs);
      }
    };
    void loadSettings();
    return () => {
      active = false;
      if (retryTimer !== undefined) window.clearTimeout(retryTimer);
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!showFloatingControls) return;
    const close = (event: Event) => {
      const target = event.target;
      if (target instanceof Node && settingsWrapRef.current?.contains(target)) return;
      setShowFloatingControls(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setShowFloatingControls(false);
    };
    document.addEventListener("pointerdown", close, true);
    document.addEventListener("mousedown", close, true);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", close, true);
      document.removeEventListener("mousedown", close, true);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [showFloatingControls]);

  const toggleFloatingWindow = async () => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setFloatingSettings((value) => ({ ...value, visible: !value.visible }));
      return;
    }
    try {
      const visible = await invoke<boolean>("set_floating_window", { visible: !floatingSettings.visible });
      setFloatingSettings((value) => ({ ...value, visible }));
    } catch (reason) {
      setError(String(reason));
    }
  };

  const setPinned = async () => {
    const next = !floatingSettings.pinned;
    if (!("__TAURI_INTERNALS__" in window)) {
      setFloatingSettings((value) => ({ ...value, pinned: next }));
      return;
    }
    try {
      const pinned = await invoke<boolean>("set_floating_pinned", { pinned: next });
      setFloatingSettings((value) => ({ ...value, pinned }));
    } catch (reason) { setError(String(reason)); }
  };

  const setOpacity = async (opacity: number) => {
    setFloatingSettings((value) => ({ ...value, opacity }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<number>("set_floating_opacity", { opacity });
      setFloatingSettings((value) => ({ ...value, opacity: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setAlwaysOnTop = async () => {
    const next = !floatingSettings.alwaysOnTop;
    setFloatingSettings((value) => ({ ...value, alwaysOnTop: next }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<boolean>("set_floating_always_on_top", { alwaysOnTop: next });
      setFloatingSettings((value) => ({ ...value, alwaysOnTop: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setFloatingStyle = async (style: FloatingSettings["style"]) => {
    setFloatingSettings((value) => ({ ...value, style }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const settings = await invoke<FloatingSettings>("set_floating_style", { style });
      setFloatingSettings(settings);
    } catch (reason) { setError(String(reason)); }
  };

  const setOrbExpandDirection = async (direction: FloatingSettings["orbExpandDirection"]) => {
    setFloatingSettings((value) => ({ ...value, orbExpandDirection: direction }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const settings = await invoke<FloatingSettings>("set_floating_orb_expand_direction", { direction });
      setFloatingSettings(settings);
    } catch (reason) { setError(String(reason)); }
  };

  const setOrbSize = async (orbSize: number) => {
    setFloatingSettings((value) => ({ ...value, orbSize }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<number>("set_floating_orb_size", { size: orbSize });
      setFloatingSettings((value) => ({ ...value, orbSize: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setCardScale = async (cardScale: number) => {
    setFloatingSettings((value) => ({ ...value, cardScale }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<number>("set_floating_card_scale", { scale: cardScale });
      setFloatingSettings((value) => ({ ...value, cardScale: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setOrbWaveSpeed = async (speed: number) => {
    const boundedSpeed = Math.min(3, Math.max(0.5, speed));
    setFloatingSettings((value) => ({ ...value, orbWaveSpeed: boundedSpeed }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<number>("set_orb_wave_speed", { speed: boundedSpeed });
      setFloatingSettings((value) => ({ ...value, orbWaveSpeed: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setOrbWaveAmplitude = async (amplitude: number) => {
    const boundedAmplitude = Math.min(MAX_ORB_WAVE_AMPLITUDE, Math.max(MIN_ORB_WAVE_AMPLITUDE, amplitude));
    setFloatingSettings((value) => ({ ...value, orbWaveAmplitude: boundedAmplitude }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<number>("set_orb_wave_amplitude", { amplitude: boundedAmplitude });
      setFloatingSettings((value) => ({ ...value, orbWaveAmplitude: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setSyncInterval = async (syncIntervalSecs: number) => {
    const previousInterval = floatingSettings.syncIntervalSecs;
    setFloatingSettings((value) => ({ ...value, syncIntervalSecs }));
    setSyncIntervalDraft(String(syncIntervalSecs));
    setSyncIntervalError(null);
    if (!("__TAURI_INTERNALS__" in window)) return true;
    try {
      const saved = await invoke<number>("set_sync_interval", { intervalSecs: syncIntervalSecs });
      setFloatingSettings((value) => ({ ...value, syncIntervalSecs: saved }));
      setSyncIntervalDraft(String(saved));
      return true;
    } catch (reason) {
      setFloatingSettings((value) => ({ ...value, syncIntervalSecs: previousInterval }));
      setSyncIntervalDraft(String(previousInterval));
      setError(String(reason));
      return false;
    }
  };

  const commitSyncIntervalDraft = () => {
    if (!/^\d+$/.test(syncIntervalDraft.trim())) {
      setSyncIntervalError(`请输入 ${MIN_SYNC_INTERVAL_SECS} 到 ${MAX_SYNC_INTERVAL_SECS} 之间的整数秒数。`);
      return;
    }
    const intervalSecs = Number(syncIntervalDraft);
    if (!Number.isSafeInteger(intervalSecs) || intervalSecs < MIN_SYNC_INTERVAL_SECS || intervalSecs > MAX_SYNC_INTERVAL_SECS) {
      setSyncIntervalError(`可设置范围为 ${MIN_SYNC_INTERVAL_SECS}–${MAX_SYNC_INTERVAL_SECS} 秒。`);
      return;
    }
    if (intervalSecs === floatingSettings.syncIntervalSecs) {
      setSyncIntervalDraft(String(intervalSecs));
      setSyncIntervalError(null);
      return;
    }
    void setSyncInterval(intervalSecs);
  };

  const setDisplayMode = async (mode: FloatingSettings["displayMode"]) => {
    setFloatingSettings((value) => ({ ...value, displayMode: mode }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<FloatingSettings["displayMode"]>("set_display_mode", { mode });
      setFloatingSettings((value) => ({ ...value, displayMode: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const setTheme = async (theme: FloatingSettings["theme"]) => {
    setFloatingSettings((value) => ({ ...value, theme }));
    if (!("__TAURI_INTERNALS__" in window)) return;
    try {
      const saved = await invoke<FloatingSettings["theme"]>("set_theme", { theme });
      setFloatingSettings((value) => ({ ...value, theme: saved }));
    } catch (reason) { setError(String(reason)); }
  };

  const saveProxy = async (mode: FloatingSettings["proxyMode"], address = proxyAddressDraft) => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setFloatingSettings((value) => ({ ...value, proxyMode: mode, proxyAddress: address }));
      return;
    }
    try {
      const settings = await invoke<FloatingSettings>("set_proxy_settings", { mode, address });
      setFloatingSettings(settings);
      setProxyAddressDraft(settings.proxyAddress);
    } catch (reason) { setError(String(reason)); }
  };

  const displayedUsage = activePlatform === "cursor"
    ? cursorQuota?.status === "connected" ? cursorQuotaAsUsage(cursorQuota) : null
    : usage;
  const displayedQuotaLoading = activePlatform === "cursor" ? cursorQuotaLoading : loading;
  const displayedQuotaError = activePlatform === "cursor"
    ? cursorQuotaError || (cursorQuota && cursorQuota.status !== "connected" ? cursorQuota.message ?? "Cursor 额度暂不可用" : null)
    : error;
  const displayedTokenStats: TokenUsageStats | null = activePlatform === "cursor"
    ? cursorTokenStats ? {
      totalTokens: cursorTokenStats.totalTokens,
      todayTokens: cursorTokenStats.todayTokens,
      sessionsScanned: cursorTokenStats.eventsScanned,
      unreadableSessions: 0,
      available: cursorTokenStats.cacheAvailable,
      updatedAt: cursorTokenStats.updatedAt ?? 0,
    } : null
    : tokenStats;
  const displayedTokenError = activePlatform === "cursor"
    ? cursorTokenStatsError || cursorTokenStats?.syncError || null
    : tokenStatsError;
  const displayedTokenLoading = activePlatform === "cursor" ? cursorTokenStatsLoading : tokenStatsLoading;
  const updatedAt = useMemo(() => displayedUsage ? new Date(displayedUsage.fetchedAt * 1000).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false }) : "尚未同步", [displayedUsage]);
  const primary = displayedUsage?.primary;
  const secondary = displayedUsage?.secondary;
  const primaryOppositeMode = oppositeDisplayMode(floatingSettings.displayMode);
  const primaryOppositeMetric = metricValue(primary?.usedPercent ?? 0, primaryOppositeMode);
  const secondaryMetric = metricValue(secondary?.usedPercent ?? 0, floatingSettings.displayMode);
  const primaryLabel = displayedUsage?.primaryLabel ?? (primary ? formatDuration(primary.windowDurationMins) : activePlatform === "cursor" ? "Cursor 模型" : "短周期");
  const secondaryLabel = displayedUsage?.secondaryLabel ?? (secondary ? formatDuration(secondary.windowDurationMins) : activePlatform === "cursor" ? "其他模型" : "长期");
  const refreshActiveQuota = () => activePlatform === "cursor" ? void refreshCursorOverview(true, false) : void refresh(true, false);
  const startingQuotaRead = isDesktopApp && (!floatingSettingsLoaded || (displayedQuotaLoading && !displayedUsage));
  quotaOnlyRefreshRef.current = refreshActiveQuota;

  return (
    <main className={`app-shell ${sidebarExpanded ? "" : "is-sidebar-collapsed"}`}>
      <MainTitlebar />
      <div className="main-workspace">
      <aside className={`platform-sidebar ${sidebarExpanded ? "" : "platform-sidebar--collapsed"}`} aria-label="平台切换">
        <div className="platform-sidebar__heading">
          <div className="platform-sidebar__identity"><span>PLATFORM</span><strong>监测平台</strong></div>
          <button
            className="platform-sidebar__collapse"
            onClick={() => setSidebarExpanded((expanded) => !expanded)}
            aria-expanded={sidebarExpanded}
            aria-label={sidebarExpanded ? "收起平台侧边栏" : "展开平台侧边栏"}
            title={sidebarExpanded ? "收起侧边栏" : "展开侧边栏"}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m10 3-5 5 5 5" /></svg>
          </button>
        </div>
        <nav className="platform-list" aria-label="选择统计平台">
          <button className={`platform-switch ${activePlatform === "codex" ? "is-active" : ""}`} onClick={() => void selectPlatform("codex")} disabled={isDesktopApp && !floatingSettingsLoaded} aria-label="Codex 平台" title="Codex" aria-pressed={activePlatform === "codex"}>
            <span className="platform-switch__mark platform-switch__mark--codex"><CodexPlatformIcon /></span>
            <span className="platform-switch__copy"><strong>Codex</strong><small>额度 · Token</small></span>
            <i />
          </button>
          <button className={`platform-switch platform-switch--cursor ${activePlatform === "cursor" ? "is-active" : ""}`} onClick={() => void selectPlatform("cursor")} disabled={isDesktopApp && !floatingSettingsLoaded} aria-label="Cursor 平台" title="Cursor" aria-pressed={activePlatform === "cursor"}>
            <span className="platform-switch__mark platform-switch__mark--cursor"><CursorPlatformIcon /></span>
            <span className="platform-switch__copy"><strong>Cursor</strong><small>额度 · Token</small></span>
            <i />
          </button>
        </nav>
        <div className="platform-sidebar__footer"><span className="platform-sidebar__pulse" /><strong>单平台显示</strong><small>当前只展示所选平台<br />的额度与 Token 视角</small></div>
      </aside>
      <section className="dashboard-panel">
      <div className="ambient ambient--one" /><div className="ambient ambient--two" />
      <header className="topbar">
        <div className="brand"><div className="brand__mark"><span /></div><div><div className="brand__name">AI USAGE METER</div><div className="brand__sub">{activePlatform === "codex" ? "Codex · 额度与 Token" : "Cursor · 额度与 Token"}</div></div></div>
        <nav className="main-view-tabs" role="tablist" aria-label="主界面视角">
          <button id="main-view-tab-quota" className={mainView === "quota" ? "is-active" : ""} role="tab" aria-controls="quota-view" aria-selected={mainView === "quota"} onClick={() => setMainView("quota")}>
            <span>01</span><strong>额度视角</strong>
          </button>
          <button id="main-view-tab-tokens" className={mainView === "tokens" ? "is-active" : ""} role="tab" aria-controls="token-view" aria-selected={mainView === "tokens"} onClick={() => setMainView("tokens")}>
            <span>02</span><strong>Token 视角</strong>
          </button>
        </nav>
        <div className="topbar__actions">
          {activePlatform === "codex" && demoMode && <span className="demo-pill">浏览器演示</span>}
          <div className="sync-state"><i className={displayedQuotaError ? "is-error" : ""} /><span>{!floatingSettingsLoaded && isDesktopApp ? "正在准备本机服务" : displayedQuotaLoading ? "正在同步" : displayedQuotaError ? "同步失败" : `更新于 ${updatedAt}`}</span></div>
          <button className={`floating-toggle ${floatingSettings.visible ? "is-active" : ""}`} onClick={toggleFloatingWindow} disabled={isDesktopApp && !floatingSettingsLoaded} aria-pressed={floatingSettings.visible} aria-label={floatingSettings.visible ? "关闭悬浮窗" : "开启悬浮窗"} title={floatingSettings.visible ? "关闭悬浮窗" : "开启悬浮窗"}>
            <span className="toggle-track"><i /></span>
            <span className="sidebar-action-label">悬浮窗</span>
          </button>
          <div className="floating-settings-wrap" ref={settingsWrapRef}>
            <button className={`settings-button ${showFloatingControls ? "is-active" : ""}`} onClick={() => setShowFloatingControls((value) => !value)} disabled={isDesktopApp && !floatingSettingsLoaded} aria-expanded={showFloatingControls} aria-label="全局设置" title="全局设置">
              <svg viewBox="0 0 24 24"><path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7ZM19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6V20h-4v-.08a1.7 1.7 0 0 0-1-.6 1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1H4v-4h.08a1.7 1.7 0 0 0 .6-1 1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6V4h4v.08a1.7 1.7 0 0 0 1 .6 1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9c.12.38.33.72.6 1h.08v4H20c-.27.28-.48.62-.6 1Z" /></svg>
            </button>
            <section className={`floating-popover ${showFloatingControls ? "is-open" : ""}`} aria-hidden={!showFloatingControls}>
              <PrettyScroller active={showFloatingControls}>
              <div className="popover-heading"><div><span>显示与连接</span><strong>GLOBAL DISPLAY / SETTINGS</strong></div><span className="settings-state">自动保存</span></div>
              <div className="settings-block">
                <span className="settings-label">额度显示形式</span>
                <div className="segment-control">
                  <button className={floatingSettings.displayMode === "available" ? "is-active" : ""} onClick={() => void setDisplayMode("available")}>显示可用</button>
                  <button className={floatingSettings.displayMode === "used" ? "is-active" : ""} onClick={() => void setDisplayMode("used")}>显示已使用</button>
                </div>
              </div>
              <div className="settings-block settings-block--interval">
                <label htmlFor="sync-interval" className="settings-interval-label">
                  <span className="settings-label">额度自动同步</span>
                  <small>自动刷新与平台切换共用冷却时间</small>
                </label>
                <div className="settings-interval-input-wrap">
                  <input
                    id="sync-interval"
                    className="settings-interval-input"
                    type="number"
                    min={MIN_SYNC_INTERVAL_SECS}
                    max={MAX_SYNC_INTERVAL_SECS}
                    step="1"
                    inputMode="numeric"
                    value={syncIntervalDraft}
                    onChange={(event) => { setSyncIntervalDraft(event.target.value); setSyncIntervalError(null); }}
                    onBlur={commitSyncIntervalDraft}
                    onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }}
                    aria-label="额度同步间隔秒数"
                    aria-describedby="sync-interval-hint"
                    aria-invalid={syncIntervalError !== null}
                  />
                  <span>秒</span>
                </div>
                <small id="sync-interval-hint" className={`settings-hint ${syncIntervalError ? "settings-hint--error" : ""}`}>
                  {syncIntervalError ?? `输入 ${MIN_SYNC_INTERVAL_SECS}–${MAX_SYNC_INTERVAL_SECS} 秒；手动刷新仍可立即同步。`}
                </small>
              </div>
              <div className="settings-block">
                <span className="settings-label">主题配色</span>
                <div className="theme-options" role="group" aria-label="主题配色">
                  {themes.map((theme) => <button key={theme.id} className={`theme-option ${floatingSettings.theme === theme.id ? "is-active" : ""}`} onClick={() => void setTheme(theme.id)} aria-pressed={floatingSettings.theme === theme.id} aria-label={`${theme.name}主题`} title={theme.name}>
                    <i style={{ background: `linear-gradient(135deg, ${theme.colors.join(", ")})` }} />{theme.label}
                  </button>)}
                </div>
              </div>
              <div className="settings-block">
                <span className="settings-label">悬浮窗样式</span>
                <div className="segment-control">
                  <button className={floatingSettings.style === "card" ? "is-active" : ""} onClick={() => void setFloatingStyle("card")}>紧凑卡片</button>
                  <button className={floatingSettings.style === "orb" ? "is-active" : ""} onClick={() => void setFloatingStyle("orb")}>浮动小球</button>
                </div>
                <small className="settings-hint">贴边后收起为独立圆球，悬停展开额度摘要；不贴边时保持展开，拖动靠近屏幕边缘会自动吸附。</small>
              </div>
              <label className="size-control">
                <span><span className="settings-label">悬浮球尺寸</span><b>{floatingSettings.orbSize} px</b></span>
                <input type="range" min={MIN_ORB_SIZE} max={MAX_ORB_SIZE} step="2" value={floatingSettings.orbSize} onChange={(event) => void setOrbSize(Number(event.target.value))} aria-label="悬浮球尺寸" />
                <small className="settings-hint">直径 {MIN_ORB_SIZE}–{MAX_ORB_SIZE} 逻辑像素，随 Windows 显示缩放自动适配。</small>
              </label>
              <label className="size-control">
                <span><span className="settings-label">紧凑卡片尺寸</span><b>{floatingSettings.cardScale}%</b></span>
                <input type="range" min={MIN_CARD_SCALE} max={MAX_CARD_SCALE} step="5" value={floatingSettings.cardScale} onChange={(event) => void setCardScale(Number(event.target.value))} aria-label="紧凑卡片尺寸" />
                <small className="settings-hint">整体缩放 {MIN_CARD_SCALE}%–{MAX_CARD_SCALE}%，不影响主窗口大小。</small>
              </label>
              <div className={`settings-reveal ${floatingSettings.style === "orb" ? "is-open" : ""}`}>
                <div className="settings-reveal__inner">
                  <div className="settings-block">
                    <span className="settings-label">小球展开方向</span>
                    <div className="segment-control segment-control--three">
                      <button className={floatingSettings.orbExpandDirection === "auto" ? "is-active" : ""} onClick={() => void setOrbExpandDirection("auto")}>自动</button>
                      <button className={floatingSettings.orbExpandDirection === "left" ? "is-active" : ""} onClick={() => void setOrbExpandDirection("left")}>向左展开</button>
                      <button className={floatingSettings.orbExpandDirection === "right" ? "is-active" : ""} onClick={() => void setOrbExpandDirection("right")}>向右展开</button>
                    </div>
                    <small className="settings-hint">自动按小球所在屏幕一侧展开；也可以固定向左或向右展开。</small>
                  </div>
                  <label className="wave-speed-control">
                    <span><span className="settings-label">水波速度</span><b>{floatingSettings.orbWaveSpeed.toFixed(1)}×</b></span>
                    <input type="range" min="0.5" max="3" step="0.1" value={floatingSettings.orbWaveSpeed} onChange={(event) => void setOrbWaveSpeed(Number(event.target.value))} aria-label="小球水波速度" />
                    <small className="settings-hint">即时预览并自动保存，范围 0.5×–3.0×</small>
                  </label>
                  <label className="wave-speed-control wave-amplitude-control">
                    <span><span className="settings-label">水波幅度</span><b>{floatingSettings.orbWaveAmplitude.toFixed(1)}×</b></span>
                    <input type="range" min={MIN_ORB_WAVE_AMPLITUDE} max={MAX_ORB_WAVE_AMPLITUDE} step="0.1" value={floatingSettings.orbWaveAmplitude} onChange={(event) => void setOrbWaveAmplitude(Number(event.target.value))} aria-label="小球水波幅度" />
                    <small className="settings-hint">只调整波浪起伏，不改变百分比液位；范围 0.5×–4.0×</small>
                  </label>
                </div>
              </div>
              <div className="settings-block settings-block--inline">
                <div><span className="settings-label">悬浮窗固定</span><small>{floatingSettings.pinned ? "主体鼠标穿透" : "可自由拖动"}</small></div>
                <button className={`compact-action ${floatingSettings.pinned ? "is-active" : ""}`} onClick={setPinned} disabled={floatingSettings.style === "orb"}>{floatingSettings.style === "orb" ? "小球可交互" : floatingSettings.pinned ? "取消固定" : "固定"}</button>
              </div>
              <div className="settings-block settings-block--inline">
                <div><span className="settings-label">窗口置顶</span><small>{floatingSettings.alwaysOnTop ? "标准 Windows 置顶层级" : "跟随普通窗口层级"}</small></div>
                <button className={`compact-action ${floatingSettings.alwaysOnTop ? "is-active" : ""}`} onClick={() => void setAlwaysOnTop()}>{floatingSettings.alwaysOnTop ? "已置顶" : "未置顶"}</button>
              </div>
              <label className="opacity-control"><span>黑色背景透明度 <b>{Math.round((1 - floatingSettings.opacity) * 100)}%</b></span><input type="range" min="0" max="100" value={Math.round((1 - floatingSettings.opacity) * 100)} onChange={(event) => void setOpacity(1 - Number(event.target.value) / 100)} /></label>
              <div className="settings-block proxy-settings">
                <span className="settings-label">网络代理</span>
                <div className="proxy-options">
                  <label><input type="radio" name="proxy" checked={floatingSettings.proxyMode === "system"} onChange={() => void saveProxy("system")} />系统代理</label>
                  <label><input type="radio" name="proxy" checked={floatingSettings.proxyMode === "none"} onChange={() => void saveProxy("none")} />无代理</label>
                  <label><input type="radio" name="proxy" checked={floatingSettings.proxyMode === "custom"} onChange={() => setFloatingSettings((value) => ({ ...value, proxyMode: "custom" }))} />本地代理</label>
                </div>
                <div className={`settings-reveal ${floatingSettings.proxyMode === "custom" ? "is-open" : ""}`}>
                  <div className="settings-reveal__inner">
                    <div className="proxy-input-row"><input value={proxyAddressDraft} onChange={(event) => setProxyAddressDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void saveProxy("custom"); }} placeholder="http://127.0.0.1:7890" aria-label="本地代理地址" /><button onClick={() => void saveProxy("custom")}>保存</button></div>
                  </div>
                </div>
              </div>
              <p>{floatingSettings.pinned ? "固定后主体会穿透鼠标，顶部图钉仍可取消固定。" : "设置会写入 EXE 同目录的 data 文件夹。"}</p>
              <small title={floatingSettings.dataDirectory}>设置自动保存在 data 文件夹</small>
              </PrettyScroller>
            </section>
          </div>
          <button className="refresh-button" onClick={refreshActiveQuota} disabled={displayedQuotaLoading || (isDesktopApp && !floatingSettingsLoaded)} aria-label="刷新额度" title="刷新额度">
            <svg className={displayedQuotaLoading ? "is-spinning" : ""} viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5M6.1 8.1A7 7 0 0 1 18.6 7M17.9 15.9A7 7 0 0 1 5.4 17" /></svg>
            <span className="refresh-label">{displayedQuotaLoading ? "同步中" : "刷 新"}</span>
          </button>
        </div>
      </header>

      <div className="view-stage">
      {sceneTransition((style, key) => (
        <animated.div className="view-pane" style={style} aria-hidden={key !== sceneKey}>
          {key.endsWith(":tokens") ? (
        <TokenOverview
          stats={displayedTokenStats}
          loading={displayedTokenLoading || (isDesktopApp && !floatingSettingsLoaded)}
          error={displayedTokenError}
          previewOnly={!("__TAURI_INTERNALS__" in window)}
          onRefresh={refreshActiveTokenStats}
          active={key === sceneKey}
        />
          ) : (
        <section className="quota-view" id={key === sceneKey ? "quota-view" : undefined} role="tabpanel" aria-labelledby="main-view-tab-quota">
        {startingQuotaRead ? (
          <section className="initializing-panel" role="status" aria-live="polite">
            <span className="error-panel__code">{floatingSettingsLoaded ? "QUOTA SYNC / 01" : "LOCAL SERVICE / STARTING"}</span>
            <div className="initializing-panel__body"><span className="initializing-panel__spinner" aria-hidden="true" /><div><h1>{floatingSettingsLoaded ? "正在同步额度" : "正在读取本机数据"}</h1><p>{floatingSettingsLoaded ? "正在检查所选平台账户并读取额度，已有缓存会优先显示。" : "正在准备账户缓存与后台同步，完成后会自动显示额度。"}</p></div></div>
          </section>
        ) : !displayedQuotaLoading && displayedQuotaError && !displayedUsage ? (
          <section className="error-panel"><span className="error-panel__code">CONNECTION / 01</span><h1>还没读到额度</h1><p>{displayedQuotaError}</p><button onClick={refreshActiveQuota}>重新检测</button></section>
        ) : (
          <>
          <section className="hero-grid">
            <article className="quota-card quota-card--primary">
              <div className="card-heading"><div><span className="eyebrow">PRIMARY WINDOW</span><h1>{primaryLabel}额度</h1></div><span className="live-badge"><i /> LIVE</span></div>
              <div className="primary-layout">
                <Gauge usedPercent={primary?.usedPercent ?? 0} mode={floatingSettings.displayMode} />
              <div className="quota-copy"><p>本周期{metricLabel(primaryOppositeMode)}</p><strong>{Math.round(primaryOppositeMetric)}<small>%</small></strong><div className="divider" /><p>重置时间</p><b>{primary ? formatResetAt(primary.resetsAt) : "等待数据"}</b>{primary && <span className="reset-countdown">剩余 {formatRemaining(primary.resetsAt, nowMs)}</span>}</div>
              </div>
              <div className="card-note">额度数据 · 自动同步</div>
            </article>

            <article className="quota-card quota-card--secondary">
              <div className="card-heading"><div><span className="eyebrow">SECONDARY WINDOW</span><h2>{secondaryLabel}额度</h2></div><span className="index-label">02</span></div>
              <Gauge usedPercent={secondary?.usedPercent ?? 0} mode={floatingSettings.displayMode} compact />
              <div className="bar-copy"><span>{metricLabel(floatingSettings.displayMode)} {Math.round(secondaryMetric)}%</span><span>{floatingSettings.displayMode === "used" ? "可用" : "已使用"} {Math.round(100 - secondaryMetric)}%</span></div>
              <div className="progress-track"><span style={{ width: `${secondaryMetric}%` }} /></div>
              <div className="reset-row"><span>重置时间</span><div><strong>{secondary ? formatResetAt(secondary.resetsAt) : "等待数据"}</strong>{secondary && <small>剩余 {formatRemaining(secondary.resetsAt, nowMs)}</small>}</div></div>
            </article>
          </section>

          <section className="account-strip">
            <div className="account-identity"><span className="avatar">{displayedUsage?.email?.slice(0, 1).toUpperCase() || "C"}</span><div><span>当前账户</span><strong>{displayedUsage?.email || "—"}</strong></div></div>
            <div className="stat"><span>订阅</span><strong>{planLabel(displayedUsage?.planType ?? null)}</strong></div>
            <div className="stat"><span>可用余额</span><strong>{activePlatform === "cursor" ? displayedUsage?.creditBalance ?? "—" : displayedUsage?.unlimited ? "无限" : displayedUsage?.hasCredits ? displayedUsage.creditBalance : "未启用"}</strong></div>
            <div className="stat"><span>重置券</span><strong>{displayedUsage?.resetCredits ?? 0} 张</strong></div>
          </section>
          </>
        )}
        </section>
          )}
        </animated.div>
      ))}
      </div>
      <footer><span>AI USAGE METER / WINDOWS</span><span>{activePlatform === "codex" ? "CODEX" : "CURSOR"} · LOCAL TOKEN LEDGER</span></footer>
      </section>
      </div>
    </main>
  );
}

function App() {
  const isFloating = new URLSearchParams(window.location.search).get("view") === "floating";
  if (isFloating) document.documentElement.classList.add("floating-document");
  return isFloating ? <FloatingApp /> : <DashboardApp />;
}

export default App;
