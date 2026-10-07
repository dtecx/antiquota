/** Owns dictation: live speech recognition where the browser has it, otherwise a
 * MediaRecorder recording that agy (Gemini) transcribes on an account with quota. */
import { writable, type Readable } from 'svelte/store';
import { RECORDING_TYPES, audioExtension, peakOf, recordingProblem } from './media';

export type DictationState = 'idle' | 'listening' | 'recording' | 'transcribing';

interface Recognition {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  onresult: ((event: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}
export type RecognitionConstructor = new () => Recognition;

/** The part of MediaRecorder dictation uses. */
export interface Recorder {
  readonly mimeType: string;
  readonly state: string;
  ondataavailable: ((event: BlobEvent) => void) | null;
  onstop: ((event: Event) => void) | null;
  start(): void;
  stop(): void;
}

interface Recording {
  stream: MediaStream;
  recorder: Recorder;
  chunks: Blob[];
  context: AudioContext;
  source: MediaStreamAudioSourceNode;
  analyser: AnalyserNode;
  samples: Float32Array<ArrayBuffer>;
  meter: ReturnType<typeof setInterval>;
  peak: number;
  started: number;
}

export interface DictationHost {
  /** The draft before dictation started, so live words extend it. */
  draft(): string;
  /** Replaces the draft while live recognition hears words. */
  live(text: string): void;
  /** Inserts transcribed words at the selection. */
  insert(text: string): void;
  transcribe(data: Blob, type: string): Promise<{ text: string }>;
  notice(text: string): void;
  fail(text: string): void;
}

export interface DictationEnvironment {
  speech: RecognitionConstructor | null;
  microphone: (constraints: MediaStreamConstraints) => Promise<MediaStream>;
  recorder: (stream: MediaStream) => Recorder;
  /** Measures the input level for the silence guard; it never shapes the recording. */
  audioContext: () => AudioContext;
  language: string;
  now: () => number;
}

/** The browser's speech recognition and microphone; the desktop shell always records. */
export function browserEnvironment(): DictationEnvironment {
  const scope = window as unknown as {
    SpeechRecognition?: RecognitionConstructor;
    webkitSpeechRecognition?: RecognitionConstructor;
    webkitAudioContext?: typeof AudioContext;
    __TAURI__?: unknown;
  };
  // The app's web view has no speech service: it records and transcribes instead.
  const speech = scope.__TAURI__
    ? null
    : (scope.SpeechRecognition ?? scope.webkitSpeechRecognition ?? null);
  return {
    speech,
    microphone: (constraints) => navigator.mediaDevices.getUserMedia(constraints),
    recorder: (stream) => {
      const mimeType = RECORDING_TYPES.find((type) => MediaRecorder.isTypeSupported(type));
      return new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
    },
    audioContext: () => new (window.AudioContext ?? scope.webkitAudioContext ?? AudioContext)(),
    language: navigator.language || 'en-US',
    now: () => Date.now(),
  };
}

export interface Dictation {
  state: Readable<DictationState>;
  /** When the current recording started, for the elapsed-time label. */
  since: Readable<number>;
  toggle(): Promise<void>;
  /** Escape: drops a recording or live recognition without inserting anything. */
  cancel(): Promise<boolean>;
}

export function createDictation(host: DictationHost, env: DictationEnvironment): Dictation {
  const state = writable<DictationState>('idle');
  const since = writable(0);
  let current: DictationState = 'idle';
  let recognition: Recognition | null = null;
  let recording: Recording | null = null;
  const set = (next: DictationState) => {
    current = next;
    state.set(next);
  };

  function startSpeech(speech: RecognitionConstructor) {
    const rec = new speech();
    const draft = host.draft();
    const base = draft ? `${draft.trimEnd()} ` : '';
    rec.continuous = true;
    rec.interimResults = true;
    rec.lang = env.language;
    rec.onresult = (event) => {
      let text = '';
      for (const result of Array.from(event.results)) text += result[0]?.transcript ?? '';
      host.live(base + text.trim());
    };
    rec.onerror = (event) => {
      recognition = null;
      set('idle');
      // No speech service (or no permission for it): record and transcribe instead.
      if (['not-allowed', 'service-not-allowed', 'network'].includes(event.error))
        void startRecording();
      else if (event.error !== 'aborted' && event.error !== 'no-speech')
        host.fail(`Dictation: ${event.error}`);
    };
    rec.onend = () => {
      recognition = null;
      if (current === 'listening') set('idle');
    };
    recognition = rec;
    rec.start();
    set('listening');
  }

  async function startRecording() {
    let stream: MediaStream;
    try {
      stream = await env.microphone({
        audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
      });
    } catch (error) {
      host.fail(
        `No microphone access: ${error instanceof Error ? error.message || error.name : String(error)}`,
      );
      return;
    }
    let recorder: Recorder;
    try {
      recorder = env.recorder(stream);
    } catch (error) {
      for (const track of stream.getTracks()) track.stop();
      host.fail(`Recording is not available: ${error instanceof Error ? error.message : error}`);
      return;
    }
    const context = env.audioContext();
    const source = context.createMediaStreamSource(stream);
    const analyser = context.createAnalyser();
    analyser.fftSize = 2048;
    source.connect(analyser);
    const chunks: Blob[] = [];
    recorder.ondataavailable = (event) => {
      if (event.data.size) chunks.push(event.data);
    };
    const active: Recording = {
      stream,
      recorder,
      chunks,
      context,
      source,
      analyser,
      samples: new Float32Array(analyser.fftSize),
      meter: setInterval(() => listen(active), 50),
      peak: 0,
      started: env.now(),
    };
    recording = active;
    recorder.start();
    since.set(active.started);
    set('recording');
  }

  function listen(rec: Recording) {
    rec.analyser.getFloatTimeDomainData(rec.samples);
    rec.peak = Math.max(rec.peak, peakOf(rec.samples));
  }

  async function stopRecording(cancel: boolean) {
    const rec = recording;
    if (!rec) return;
    recording = null;
    clearInterval(rec.meter);
    listen(rec);
    const seconds = (env.now() - rec.started) / 1000;
    if (rec.recorder.state !== 'inactive')
      await new Promise<void>((resolve) => {
        rec.recorder.onstop = () => resolve();
        rec.recorder.stop();
      });
    rec.source.disconnect();
    for (const track of rec.stream.getTracks()) track.stop();
    await rec.context.close();
    if (cancel) return set('idle');
    const problem = recordingProblem(seconds, rec.peak);
    if (problem) {
      set('idle');
      host.notice(
        problem === 'short'
          ? 'Too short: hold on a little longer'
          : 'Nothing heard: is the right microphone selected?',
      );
      return;
    }
    set('transcribing');
    const type = rec.recorder.mimeType || rec.chunks[0]?.type || 'audio/webm';
    try {
      const heard = await host.transcribe(new Blob(rec.chunks, { type }), audioExtension(type));
      if (heard.text) host.insert(heard.text);
      else host.notice('Nothing heard');
    } catch (error) {
      host.fail(error instanceof Error ? error.message : 'Dictation failed.');
    }
    set('idle');
  }

  return {
    state,
    since,
    async toggle() {
      if (current === 'listening') return recognition?.stop();
      if (current === 'recording') return stopRecording(false);
      if (current === 'transcribing') return;
      if (env.speech) startSpeech(env.speech);
      else await startRecording();
    },
    async cancel() {
      if (current === 'recording') {
        await stopRecording(true);
        return true;
      }
      if (current === 'listening') {
        recognition?.abort();
        recognition = null;
        set('idle');
        return true;
      }
      return false;
    },
  };
}
