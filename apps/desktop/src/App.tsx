import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Check, Copy, Download, LockKeyhole, Mic2, X } from 'lucide-react';
import { BrandMark } from './BrandMark';
import type { ActivationMode, Bootstrap, ModelStatus, RuntimeStatus } from './types';

export default function App() {
  const [bootstrap, setBootstrap] = useState<Bootstrap>();
  const [runtime, setRuntime] = useState<RuntimeStatus>();
  const [notice, setNotice] = useState<string>();
  const [download, setDownload] = useState(0);
  const [view, setView] = useState<'dictate' | 'recent'>('dictate');
  const [copiedId, setCopiedId] = useState<string>();
  const copyResetTimer = useRef<number | undefined>(undefined);

  const refresh = useCallback(
    () =>
      invoke<Bootstrap>('bootstrap').then((value) => {
        setBootstrap(value);
        setRuntime(value.runtime);
      }),
    [],
  );

  useEffect(() => {
    void refresh().catch((error) => setNotice(friendlyError(error)));
    const cancelWithEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void invoke('cancel');
    };
    window.addEventListener('keydown', cancelWithEscape);
    const unlistenRuntime = listen<RuntimeStatus>('runtime-status', (event) =>
      setRuntime(event.payload),
    );
    const unlistenTranscript = listen('transcript-added', () => void refresh());
    const unlistenDownload = listen<{ downloaded: number; total: number }>(
      'model-download-progress',
      (event) => setDownload(event.payload.downloaded / event.payload.total),
    );
    return () => {
      window.clearTimeout(copyResetTimer.current);
      void unlistenRuntime.then((dispose) => dispose());
      void unlistenTranscript.then((dispose) => dispose());
      void unlistenDownload.then((dispose) => dispose());
      window.removeEventListener('keydown', cancelWithEscape);
    };
  }, [refresh]);

  useEffect(() => {
    if (!bootstrap || (bootstrap.shortcutReady && bootstrap.autoInsertReady)) return;
    const retryReadiness = () => {
      const shortcut = bootstrap.shortcutReady
        ? Promise.resolve(true)
        : invoke<boolean>('enable_shortcut');
      void Promise.all([shortcut, invoke<boolean>('enable_auto_insert', { prompt: false })]).then(
        ([shortcutReady, autoInsertReady]) => {
          if (
            shortcutReady !== bootstrap.shortcutReady ||
            autoInsertReady !== bootstrap.autoInsertReady
          )
            void refresh();
        },
      );
    };
    window.addEventListener('focus', retryReadiness);
    return () => window.removeEventListener('focus', retryReadiness);
  }, [bootstrap, refresh]);

  useEffect(() => {
    if (!bootstrap || bootstrap.platform !== 'MACOS' || bootstrap.autoInsertReady) return;
    const modelReady = bootstrap.models.some(
      (item) => item.id === bootstrap.data.settings.modelId && item.downloaded,
    );
    const promptKey = 'tertius-auto-insert-permission-v3';
    if (!modelReady || localStorage.getItem(promptKey)) return;
    localStorage.setItem(promptKey, 'requested');
    void invoke<boolean>('enable_auto_insert', { prompt: true }).then((enabled) => {
      if (enabled) void refresh();
    });
  }, [bootstrap, refresh]);

  if (!bootstrap) {
    return (
      <div className="boot">
        <BrandMark />
        <span>Preparing Tertius</span>
      </div>
    );
  }

  const model =
    bootstrap.models.find((item) => item.id === bootstrap.data.settings.modelId) ??
    bootstrap.models[0];
  const ready = Boolean(model?.downloaded);
  const active = runtime?.phase === 'recording';
  const working = runtime && !['idle', 'recording', 'error'].includes(runtime.phase);
  const activationMode = bootstrap.data.settings.activationMode ?? 'hold';
  const keys = bootstrap.platform === 'MACOS' ? ['⌃', '⌥', 'V'] : ['CTRL', 'ALT', 'V'];
  const shortcutName =
    bootstrap.platform === 'MACOS' ? 'Control + Option + V' : 'Control + Alt + V';

  const setActivationMode = async (mode: ActivationMode) => {
    setBootstrap(
      (current) =>
        current && {
          ...current,
          data: { ...current.data, settings: { ...current.data.settings, activationMode: mode } },
        },
    );
    try {
      await invoke<ActivationMode>('set_activation_mode', { mode });
    } catch (error) {
      setNotice(friendlyError(error));
      await refresh();
    }
  };

  const enableShortcut = async () => {
    try {
      const enabled = await invoke<boolean>('enable_shortcut');
      await refresh();
      if (!enabled) {
        setNotice(
          'That shortcut is already reserved by another app. The dictation button is still available.',
        );
      }
    } catch (error) {
      setNotice(friendlyError(error));
    }
  };

  const setup = async () => {
    if (!model || download > 0) return;
    setDownload(0.001);
    try {
      const models = await invoke<ModelStatus[]>('download_model', { modelId: model.id });
      setBootstrap((current) => current && { ...current, models });
      setDownload(0);
    } catch (error) {
      setDownload(0);
      setNotice(friendlyError(error));
    }
  };

  const record = async () => {
    if (!ready) {
      await setup();
      return;
    }
    try {
      await invoke(active ? 'stop_recording' : 'start_recording');
    } catch {
      // The runtime error state carries the friendly, contextual recovery message.
    }
  };

  const copyTranscript = async (id: string, text: string) => {
    try {
      await invoke('copy_text', { text });
      setCopiedId(id);
      window.clearTimeout(copyResetTimer.current);
      copyResetTimer.current = window.setTimeout(() => setCopiedId(undefined), 1400);
    } catch {
      setNotice('Tertius could not copy that dictation. Try again.');
    }
  };

  const state = statusCopy(runtime, ready, activationMode, bootstrap.shortcutReady);
  const recent = bootstrap.data.history[0];
  const startWindowDrag = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    void getCurrentWindow()
      .startDragging()
      .catch(() => {
        setNotice('Tertius could not move this window. Close and reopen the app, then try again.');
      });
  };

  return (
    <div className={`simple-app platform-${bootstrap.platform.toLowerCase()}`}>
      <div
        className="window-drag-strip"
        data-tauri-drag-region
        onMouseDown={startWindowDrag}
        aria-hidden="true"
      />
      <header className="app-header">
        <div className="identity">
          <BrandMark live={active} />
          <div>
            <strong>TERTIUS</strong>
            <small>BY FARYNTH</small>
          </div>
        </div>
        <nav className="app-tabs" aria-label="Tertius views">
          <button
            className={view === 'dictate' ? 'selected' : ''}
            onClick={() => setView('dictate')}
          >
            Dictate
          </button>
          <button className={view === 'recent' ? 'selected' : ''} onClick={() => setView('recent')}>
            Recent <span>{bootstrap.data.history.length}</span>
          </button>
        </nav>
      </header>

      {view === 'dictate' ? (
        <main className="dictate-view">
          <section className={`dictation ${active ? 'active' : ''}`}>
            <div className="dictation-copy">
              <small>{state.kicker}</small>
              <h1>{state.title}</h1>
              <p>{state.detail}</p>
              {!active && !working && (
                <div className="hero-keycaps">
                  {keys.map((key) => (
                    <kbd key={key}>{key}</kbd>
                  ))}
                </div>
              )}
            </div>

            <button
              className={`dictation-button phase-${runtime?.phase ?? 'idle'}`}
              onClick={() => void record()}
              disabled={Boolean(working)}
            >
              <TopographicBands phase={runtime?.phase ?? 'idle'} level={runtime?.level ?? 0} />
              <span className="button-ring" />
              {download > 0 ? (
                <DownloadProgress value={download} />
              ) : working ? (
                <WorkingMark />
              ) : (
                <BrandMark live={active} />
              )}
              <strong>{download > 0 ? 'Setting up' : state.button}</strong>
              <small>
                {download > 0
                  ? `${Math.round(download * 100)}%`
                  : active
                    ? formatClock(runtime?.elapsedMs ?? 0)
                    : ready
                      ? keys.join(' + ')
                      : 'ONE-TIME DOWNLOAD'}
              </small>
            </button>
          </section>

          <section className="shortcut-card">
            <div className="shortcut-copy">
              <small>
                {bootstrap.shortcutReady ? 'SHORTCUT BEHAVIOR' : 'SHORTCUT UNAVAILABLE'}
              </small>
              <p>
                {bootstrap.shortcutReady
                  ? activationMode === 'hold'
                    ? `Hold ${shortcutName} while you speak. Release when you are done.`
                    : `Press ${shortcutName} to start. Press it again to finish.`
                  : 'Another app may already own this shortcut. You can retry or use the dictation button.'}
              </p>
            </div>
            <div className="shortcut-actions">
              {bootstrap.shortcutReady ? (
                <div className="mode-picker" aria-label="Shortcut behavior">
                  <button
                    className={activationMode === 'hold' ? 'selected' : ''}
                    onClick={() => void setActivationMode('hold')}
                  >
                    Hold
                  </button>
                  <button
                    className={activationMode === 'toggle' ? 'selected' : ''}
                    onClick={() => void setActivationMode('toggle')}
                  >
                    Press on / off
                  </button>
                </div>
              ) : (
                <button className="enable-shortcut" onClick={() => void enableShortcut()}>
                  Retry shortcut
                </button>
              )}
            </div>
          </section>

          {recent && (
            <section className="recent-output">
              <CopyTranscriptButton
                copied={copiedId === recent.id}
                onCopy={() => void copyTranscript(recent.id, recent.text)}
              />
              <small>LAST DICTATION</small>
              <p>{recent.text}</p>
              <span>
                {recent.appName ?? 'This device'} · {recent.words} words
              </span>
            </section>
          )}

          <footer className="privacy-line">
            <LockKeyhole size={15} />
            <span>Audio is transcribed locally, then discarded.</span>
          </footer>
        </main>
      ) : (
        <main className="history-view">
          <section className="history-intro">
            <small>THREE-DAY LOCAL HISTORY</small>
            <h1>Recent dictations.</h1>
            <p>
              Finished text stays on this device for three days, then Tertius removes it
              automatically.
            </p>
          </section>
          {bootstrap.data.history.length > 0 ? (
            <div className="history-list">
              {bootstrap.data.history.map((transcript) => (
                <article key={transcript.id}>
                  <CopyTranscriptButton
                    copied={copiedId === transcript.id}
                    onCopy={() => void copyTranscript(transcript.id, transcript.text)}
                  />
                  <p>{transcript.text}</p>
                  <footer>
                    <span>{formatTranscriptTime(transcript.createdAtMs)}</span>
                    <span>{transcript.appName ?? 'This device'}</span>
                    <span>{transcript.words} words</span>
                  </footer>
                </article>
              ))}
            </div>
          ) : (
            <section className="history-empty">
              <BrandMark />
              <h2>Nothing here yet.</h2>
              <p>Your finished dictations will appear here automatically.</p>
              <button onClick={() => setView('dictate')}>Start dictating</button>
            </section>
          )}
        </main>
      )}

      {notice && (
        <div className="notice">
          <span>{notice}</span>
          <button onClick={() => setNotice(undefined)}>
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}

function statusCopy(
  runtime: RuntimeStatus | undefined,
  ready: boolean,
  mode: ActivationMode,
  shortcutReady: boolean,
) {
  if (!ready)
    return {
      kicker: 'FIRST RUN',
      title: 'Set up once.',
      detail: 'Download the private speech engine. After that, Tertius works locally.',
      button: 'Set up Tertius',
    };
  if (runtime?.phase === 'recording')
    return {
      kicker:
        runtime.mode === 'pushToTalk'
          ? 'LISTENING / RELEASE TO FINISH'
          : 'LISTENING / PRESS AGAIN TO FINISH',
      title: 'I’m listening.',
      detail: 'Speak naturally. Tertius will clean the words and write them where your cursor is.',
      button: 'Finish dictation',
    };
  if (runtime?.phase === 'starting')
    return {
      kicker: 'CONNECTING MICROPHONE',
      title: 'One moment.',
      detail: 'Tertius is getting the microphone ready.',
      button: 'Connecting',
    };
  if (runtime?.phase === 'complete')
    return {
      kicker: 'DICTATION COMPLETE',
      title: 'Written.',
      detail: runtime.message ?? 'The full dictation is also on your clipboard.',
      button: 'All set',
    };
  if (runtime && !['idle', 'error'].includes(runtime.phase))
    return {
      kicker: 'LOCAL PROCESSING',
      title: runtime.phase === 'inserting' ? 'Writing.' : 'Composing.',
      detail: 'Your audio stays on this machine.',
      button: 'Working',
    };
  if (runtime?.phase === 'error')
    return {
      kicker: 'LET’S TRY THAT AGAIN',
      title: 'Almost there.',
      detail: runtime.message ?? 'Tertius was interrupted. Try once more.',
      button: 'Try again',
    };
  if (!shortcutReady)
    return {
      kicker: 'ONE-TIME PERMISSION',
      title: 'Enable the shortcut.',
      detail: 'The default shortcut is already in use. Retry below or use the dictation button.',
      button: 'Start dictation',
    };
  return {
    kicker: 'READY IN EVERY APP',
    title: 'All set!',
    detail:
      mode === 'hold'
        ? 'Give it a try. Hold the keys below, speak, then release.'
        : 'Give it a try. Press the keys below to start and stop.',
    button: 'Start dictation',
  };
}

function DownloadProgress({ value }: { value: number }) {
  return (
    <span className="download-mark">
      <Download size={25} />
      <i style={{ transform: `scaleX(${value})` }} />
    </span>
  );
}

function WorkingMark() {
  return (
    <span className="working-mark">
      <Mic2 size={30} />
    </span>
  );
}

function TopographicBands({ phase, level }: { phase: RuntimeStatus['phase']; level: number }) {
  const listening = phase === 'recording' || phase === 'starting';
  const composing = ['transcribing', 'cleaning', 'inserting'].includes(phase);
  const energy = Math.min(1, Math.sqrt(Math.max(0, level) * 10));
  return (
    <span
      className={`topographic-bands ${listening ? 'listening' : ''} ${composing ? 'composing' : ''}`}
      aria-hidden="true"
    >
      {Array.from({ length: 6 }, (_, index) => {
        const listeningBase = 0.86 + index * 0.058;
        const voicePush = energy * (0.012 + index * 0.012);
        const processingBase = 0.91 + index * 0.033;
        return (
          <i
            key={index}
            style={
              {
                '--listen-duration': `${1.14 + index * 0.11}s`,
                '--listen-delay': `${index * -0.07}s`,
                '--compose-duration': `${0.38 + index * 0.055}s`,
                '--compose-delay': `${index * -0.055}s`,
                '--ring-scale-x': listening
                  ? listeningBase + voicePush * (index % 2 === 0 ? 1 : 0.72)
                  : processingBase,
                '--ring-scale-y': listening
                  ? listeningBase + voicePush * (index % 2 === 0 ? 0.72 : 1)
                  : processingBase,
                '--ring-rotation': `${(index - 2.5) * 1.8 + energy * (index % 2 === 0 ? 5 : -5)}deg`,
                '--ring-opacity': listening
                  ? 0.22 + index * 0.055 + energy * (0.25 + index * 0.025)
                  : 0.26 + index * 0.045,
                '--ring-width': `${0.72 + energy * (0.34 + index * 0.04)}px`,
              } as CSSProperties
            }
          >
            <b />
          </i>
        );
      })}
    </span>
  );
}

function CopyTranscriptButton({ copied, onCopy }: { copied: boolean; onCopy: () => void }) {
  return (
    <button
      className={`copy-transcript ${copied ? 'copied' : ''}`}
      type="button"
      aria-label={copied ? 'Dictation copied' : 'Copy dictation'}
      title={copied ? 'Copied' : 'Copy dictation'}
      onClick={onCopy}
    >
      {copied ? <Check size={13} /> : <Copy size={13} />}
      <span>{copied ? 'Copied' : 'Copy'}</span>
    </button>
  );
}

function friendlyError(error: unknown) {
  const message = String(error);
  if (message.toLowerCase().includes('timed out'))
    return 'The microphone did not respond. Check Sound settings, then try again.';
  if (
    message.toLowerCase().includes('microphone') ||
    message.toLowerCase().includes('input device')
  )
    return 'Tertius needs microphone access to listen.';
  if (message.toLowerCase().includes('speech engine') || message.toLowerCase().includes('download'))
    return 'Set up the local speech engine once, then try again.';
  if (message.toLowerCase().includes('shortcut'))
    return 'That shortcut could not be registered. Another app may already be using it.';
  return 'Tertius was interrupted. Try once more.';
}

function formatClock(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

function formatTranscriptTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    hour: 'numeric',
    minute: '2-digit',
  }).format(new Date(ms));
}
