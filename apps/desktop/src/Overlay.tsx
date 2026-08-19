import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  availableMonitors,
  cursorPosition,
  currentMonitor,
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  type Monitor,
} from '@tauri-apps/api/window';
import { GripVertical, Square, X } from 'lucide-react';
import type { Bootstrap, RuntimeStatus } from './types';

interface WidgetLocation {
  centerX: number;
  centerY: number;
  monitorName?: string;
}

const POSITION_KEY = 'tertius-widget-position-v4';
const EXPANDED_SIZE = { width: 158, height: 38 };
const CONDENSED_SIZE = { width: 64, height: 30 };
const initial: RuntimeStatus = {
  phase: 'idle',
  mode: 'pushToTalk',
  level: 0,
  elapsedMs: 0,
};

export default function Overlay() {
  const [status, setStatus] = useState(initial);
  const [hovered, setHovered] = useState(false);
  const [dragging, setDragging] = useState(false);
  const draggingRef = useRef(false);
  const hoveredRef = useRef(false);
  const expandedRef = useRef(false);
  const positionedRef = useRef(false);
  const scaleRef = useRef(1);
  const centerRef = useRef<{ x: number; y: number } | undefined>(undefined);
  const settleTimer = useRef<number | undefined>(undefined);
  const appWindow = useMemo(() => getCurrentWindow(), []);
  const recording = status.phase === 'recording' || status.phase === 'starting';
  const expanded = hovered;
  const voiceEnergy = Math.min(1, Math.sqrt(Math.max(0, status.level) * 10));

  const setWindowGeometry = useCallback(
    async (center: { x: number; y: number }, open: boolean, monitor: Monitor) => {
      const logical = open ? EXPANDED_SIZE : CONDENSED_SIZE;
      const scale = monitor.scaleFactor;
      const width = Math.round(logical.width * scale);
      const height = Math.round(logical.height * scale);
      await appWindow.setSize(new PhysicalSize(width, height));
      await appWindow.setPosition(
        new PhysicalPosition(Math.round(center.x - width / 2), Math.round(center.y - height / 2)),
      );
    },
    [appWindow],
  );

  const initializePosition = useCallback(async () => {
    const monitors = await availableMonitors();
    const stored = readPosition();
    const monitor =
      monitors.find((item) => item.name === stored?.monitorName) ??
      (await currentMonitor()) ??
      monitors[0];
    if (!monitor) return;
    const scale = monitor.scaleFactor;
    scaleRef.current = scale;
    const expandedWidth = Math.round(EXPANDED_SIZE.width * scale);
    const inset = Math.round(18 * scale);
    const area = monitor.workArea;
    const center = clampCenter(
      stored
        ? { x: stored.centerX, y: stored.centerY }
        : {
            x: area.position.x + area.size.width - expandedWidth / 2 - inset,
            y: area.position.y + area.size.height / 2,
          },
      monitor,
    );
    centerRef.current = center;
    await setWindowGeometry(center, false, monitor);
    positionedRef.current = true;
  }, [setWindowGeometry]);

  const resizeAroundCenter = useCallback(
    async (open: boolean) => {
      const center = centerRef.current;
      if (!center) return;
      const monitors = await availableMonitors();
      const monitor =
        (await currentMonitor()) ??
        monitors.find((item) => pointInside(center, item)) ??
        monitors[0];
      if (!monitor) return;
      const clamped = clampCenter(center, monitor);
      centerRef.current = clamped;
      await setWindowGeometry(clamped, open, monitor);
    },
    [setWindowGeometry],
  );

  const finishDrag = useCallback(async () => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    const position = await appWindow.outerPosition();
    const size = await appWindow.outerSize();
    const monitor = await currentMonitor();
    const center = {
      x: position.x + size.width / 2,
      y: position.y + size.height / 2,
    };
    centerRef.current = center;
    if (monitor) scaleRef.current = monitor.scaleFactor;
    localStorage.setItem(
      POSITION_KEY,
      JSON.stringify({
        centerX: center.x,
        centerY: center.y,
        monitorName: monitor?.name ?? undefined,
      } satisfies WidgetLocation),
    );
    setDragging(false);
  }, [appWindow]);

  useEffect(() => {
    void initializePosition();
    void invoke<Bootstrap>('bootstrap').then((value) => setStatus(value.runtime));
    const unlistenStatus = listen<RuntimeStatus>('runtime-status', (event) =>
      setStatus(event.payload),
    );
    const unlistenMove = appWindow.onMoved(() => {
      if (!draggingRef.current || !positionedRef.current) return;
      window.clearTimeout(settleTimer.current);
      settleTimer.current = window.setTimeout(() => void finishDrag(), 420);
    });
    let checkingPointer = false;
    const detectPointer = async () => {
      if (checkingPointer || draggingRef.current || !positionedRef.current) return;
      const center = centerRef.current;
      if (!center) return;
      checkingPointer = true;
      try {
        const pointer = await cursorPosition();
        const logical = expandedRef.current ? EXPANDED_SIZE : CONDENSED_SIZE;
        const scale = scaleRef.current;
        const margin = 7 * scale;
        const inside =
          Math.abs(pointer.x - center.x) <= (logical.width * scale) / 2 + margin &&
          Math.abs(pointer.y - center.y) <= (logical.height * scale) / 2 + margin;
        if (inside !== hoveredRef.current) {
          hoveredRef.current = inside;
          setHovered(inside);
        }
      } catch {
        // DOM pointer events remain a fallback on platforms without global cursor coordinates.
      } finally {
        checkingPointer = false;
      }
    };
    void detectPointer();
    const pointerTimer = window.setInterval(() => void detectPointer(), 75);
    return () => {
      window.clearInterval(pointerTimer);
      window.clearTimeout(settleTimer.current);
      void unlistenStatus.then((dispose) => dispose());
      void unlistenMove.then((dispose) => dispose());
    };
  }, [appWindow, finishDrag, initializePosition]);

  useEffect(() => {
    expandedRef.current = expanded;
    if (!positionedRef.current || draggingRef.current) return;
    void resizeAroundCenter(expanded);
  }, [expanded, resizeAroundCenter]);

  const beginDrag = async (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    draggingRef.current = true;
    setDragging(true);
    window.clearTimeout(settleTimer.current);
    try {
      await appWindow.startDragging();
    } catch {
      draggingRef.current = false;
      setDragging(false);
    }
  };

  const toggleRecording = async () => {
    if (status.phase === 'recording') {
      await invoke('stop_recording');
    } else if (status.phase === 'starting') {
      await invoke('cancel');
    } else if (status.phase === 'idle') {
      await invoke('start_recording');
    }
  };

  const closeWidget = async () => {
    if (recording) await invoke('cancel');
    hoveredRef.current = false;
    setHovered(false);
    await appWindow.hide();
  };

  const label = {
    idle: 'Dictate',
    starting: 'Ready…',
    recording: 'Listening',
    transcribing: 'Transcribing',
    cleaning: 'Composing',
    inserting: 'Writing',
    complete: 'Done',
    error: 'Try again',
  }[status.phase];

  return (
    <main
      className={`voice-dock ${expanded ? 'expanded' : 'condensed'} ${hovered ? 'hovered' : ''} ${dragging ? 'dragging' : ''} phase-${status.phase}`}
      onMouseEnter={() => {
        hoveredRef.current = true;
        setHovered(true);
      }}
      onMouseLeave={() => {
        if (draggingRef.current) return;
        hoveredRef.current = false;
        setHovered(false);
      }}
      onPointerDownCapture={() => void invoke('hide_main_window')}
    >
      <div className="voice-pill">
        <button
          className="voice-action"
          style={
            {
              '--voice-circle-scale': 1 + voiceEnergy * 0.09,
              '--voice-halo-size': `${0.5 + voiceEnergy * 1.5}px`,
              '--voice-halo-opacity': 0.1 + voiceEnergy * 0.28,
              '--voice-gradient-stop': `${32 + voiceEnergy * 14}%`,
            } as CSSProperties
          }
          aria-label={recording ? 'Stop dictation' : 'Start dictation'}
          onClick={() => void toggleRecording()}
          disabled={!recording && status.phase !== 'idle'}
        >
          {recording ? <Square size={9} fill="currentColor" /> : <ClassicMicrophone />}
        </button>

        <div className="voice-status">
          <span>{label}</span>
          <small>
            {recording
              ? formatClock(status.elapsedMs)
              : status.phase === 'idle'
                ? 'CLICK'
                : 'LOCAL'}
          </small>
        </div>

        <div className="voice-tail">
          <div className="voice-wave" aria-hidden="true">
            {Array.from({ length: 5 }, (_, index) => (
              <i
                key={index}
                style={{
                  height: `${recording ? Math.max(3, 4 + status.level * 15 * (0.55 + ((index * 5) % 4) / 4)) : 3}px`,
                }}
              />
            ))}
          </div>
          <div className="voice-controls">
            <button
              className="voice-grip"
              aria-label="Move dictation widget"
              title="Move"
              onPointerDown={(event) => void beginDrag(event)}
            >
              <GripVertical size={12} />
            </button>
            <button
              className="voice-close"
              aria-label="Close dictation widget"
              title="Close widget"
              onClick={() => void closeWidget()}
            >
              <X size={11} />
            </button>
          </div>
        </div>
      </div>
    </main>
  );
}

function ClassicMicrophone() {
  return (
    <svg className="classic-mic" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="8" y="2.5" width="8" height="13" rx="4" stroke="currentColor" strokeWidth="1.55" />
      <path
        d="M9.35 6h5.3M8.85 9h6.3M9.35 12h5.3"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
      <path
        d="M5.5 10.4v.8a6.5 6.5 0 0 0 13 0v-.8M12 17.7v3.1M8.8 21h6.4"
        stroke="currentColor"
        strokeWidth="1.55"
        strokeLinecap="round"
      />
    </svg>
  );
}

function readPosition(): WidgetLocation | null {
  try {
    const stored = JSON.parse(
      localStorage.getItem(POSITION_KEY) ?? 'null',
    ) as WidgetLocation | null;
    if (stored && Number.isFinite(stored.centerX) && Number.isFinite(stored.centerY)) return stored;
  } catch {
    // A corrupt preference should never prevent the widget from appearing.
  }
  return null;
}

function clampCenter(center: { x: number; y: number }, monitor: Monitor) {
  const scale = monitor.scaleFactor;
  const halfWidth = Math.round(EXPANDED_SIZE.width * scale) / 2;
  const halfHeight = Math.round(EXPANDED_SIZE.height * scale) / 2;
  const area = monitor.workArea;
  return {
    x: clamp(center.x, area.position.x + halfWidth, area.position.x + area.size.width - halfWidth),
    y: clamp(
      center.y,
      area.position.y + halfHeight,
      area.position.y + area.size.height - halfHeight,
    ),
  };
}

function pointInside(point: { x: number; y: number }, monitor: Monitor) {
  const area = monitor.workArea;
  return (
    point.x >= area.position.x &&
    point.x <= area.position.x + area.size.width &&
    point.y >= area.position.y &&
    point.y <= area.position.y + area.size.height
  );
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

function formatClock(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
