/** Proves live recognition, the recording fallback, cancellation and the transcription answer. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createDictation, type DictationEnvironment, type DictationHost } from './dictation';

function host(draft = ''): DictationHost & { log: string[] } {
  const log: string[] = [];
  return {
    log,
    draft: () => draft,
    live: (text) => log.push(`live:${text}`),
    insert: (text) => log.push(`insert:${text}`),
    transcribe: vi.fn(async () => ({ text: 'Heard words.' })),
    notice: (text) => log.push(`notice:${text}`),
    fail: (text) => log.push(`fail:${text}`),
  };
}

class FakeRecognition {
  static last: FakeRecognition | null = null;
  continuous = false;
  interimResults = false;
  lang = '';
  onresult: ((event: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null =
    null;
  onerror: ((event: { error: string }) => void) | null = null;
  onend: (() => void) | null = null;
  calls: string[] = [];
  constructor() {
    FakeRecognition.last = this;
  }
  start() {
    this.calls.push('start');
  }
  stop() {
    this.calls.push('stop');
    this.onend?.();
  }
  abort() {
    this.calls.push('abort');
  }
}

class FakeRecorder {
  state = 'inactive';
  ondataavailable: ((event: BlobEvent) => void) | null = null;
  onstop: ((event: Event) => void) | null = null;
  constructor(
    readonly mimeType: string,
    private readonly log: string[],
  ) {}
  start() {
    this.state = 'recording';
    this.log.push('start');
  }
  stop() {
    this.state = 'inactive';
    this.log.push('stop');
    queueMicrotask(() => {
      this.ondataavailable?.({
        data: new Blob(['opus-bytes'], { type: this.mimeType }),
      } as BlobEvent);
      this.onstop?.(new Event('stop'));
    });
  }
}

/** A microphone, recorder and level meter that hear `level` for `seconds`. */
function environment(
  speech: DictationEnvironment['speech'],
  level = 0.3,
  seconds = 2,
  mimeType = 'audio/ogg;codecs=opus',
) {
  const log: string[] = [];
  let clock = 1_000;
  const context = {
    createMediaStreamSource: () => ({
      connect: () => log.push('meter'),
      disconnect: () => log.push('source'),
    }),
    createAnalyser: () => ({
      fftSize: 0,
      getFloatTimeDomainData: (into: Float32Array) => into.fill(level),
    }),
    close: async () => {
      log.push('context');
    },
  };
  const stream = { getTracks: () => [{ stop: () => log.push('track') }] };
  const microphone = vi.fn(async () => stream as unknown as MediaStream);
  const env: DictationEnvironment = {
    speech,
    microphone,
    recorder: () => new FakeRecorder(mimeType, log),
    audioContext: () => context as unknown as AudioContext,
    language: 'en-GB',
    now: () => {
      const now = clock;
      clock += seconds * 1000;
      return now;
    },
  };
  return { env, log, microphone };
}

describe('dictation', () => {
  it('extends the draft with live recognition and stops on a second toggle', async () => {
    const h = host('Fix ');
    const dictation = createDictation(h, environment(FakeRecognition).env);
    await dictation.toggle();
    const rec = FakeRecognition.last!;
    expect([rec.continuous, rec.interimResults, rec.lang]).toEqual([true, true, 'en-GB']);
    expect(get(dictation.state)).toBe('listening');
    rec.onresult?.({ results: [[{ transcript: 'the ' }], [{ transcript: 'login ' }]] });
    expect(h.log).toEqual(['live:Fix the login']);
    await dictation.toggle();
    expect(rec.calls).toEqual(['start', 'stop']);
    expect(get(dictation.state)).toBe('idle');
  });
  it('falls back to recording when speech is not allowed, and Escape aborts listening', async () => {
    const h = host();
    const { env, microphone } = environment(FakeRecognition);
    const dictation = createDictation(h, env);
    await dictation.toggle();
    FakeRecognition.last!.onerror?.({ error: 'not-allowed' });
    await vi.waitFor(() => expect(get(dictation.state)).toBe('recording'));
    expect(microphone).toHaveBeenCalledWith({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
    });
    expect(await dictation.cancel()).toBe(true);
    expect(h.transcribe).not.toHaveBeenCalled();
    await dictation.toggle();
    const listening = FakeRecognition.last!;
    expect(await dictation.cancel()).toBe(true);
    expect(listening.calls).toContain('abort');
    listening.onerror?.({ error: 'audio-capture' });
    expect(h.log).toEqual(['fail:Dictation: audio-capture']);
    expect(await dictation.cancel()).toBe(false);
  });
  it('records the container MediaRecorder chose, uploads it and inserts the words', async () => {
    const h = host();
    const { env, log } = environment(null);
    const dictation = createDictation(h, env);
    await dictation.toggle();
    expect(get(dictation.state)).toBe('recording');
    expect(get(dictation.since)).toBe(1_000);
    await dictation.toggle();
    expect(h.transcribe).toHaveBeenCalledWith(expect.any(Blob), 'ogg');
    const blob = vi.mocked(h.transcribe).mock.calls[0]![0];
    expect(blob.type).toBe('audio/ogg;codecs=opus');
    expect(await blob.text()).toBe('opus-bytes');
    expect(h.log).toEqual(['insert:Heard words.']);
    expect(log).toEqual(['meter', 'start', 'stop', 'source', 'track', 'context']);
    expect(get(dictation.state)).toBe('idle');
    const webm = host();
    const other = createDictation(webm, environment(null, 0.3, 2, 'audio/webm;codecs=opus').env);
    await other.toggle();
    await other.toggle();
    expect(webm.transcribe).toHaveBeenCalledWith(expect.any(Blob), 'webm');
  });
  it('explains silence, short clips, empty answers, failures and a missing microphone', async () => {
    for (const [level, seconds, expected] of [
      [0.001, 2, 'notice:Nothing heard: is the right microphone selected?'],
      [0.5, 0.2, 'notice:Too short: hold on a little longer'],
    ] as const) {
      const h = host();
      const dictation = createDictation(h, environment(null, level, seconds).env);
      await dictation.toggle();
      await dictation.toggle();
      expect(h.log).toEqual([expected]);
      expect(h.transcribe).not.toHaveBeenCalled();
    }
    const quiet = host();
    quiet.transcribe = vi.fn(async () => ({ text: '' }));
    const silent = createDictation(quiet, environment(null).env);
    await silent.toggle();
    await silent.toggle();
    expect(quiet.log).toEqual(['notice:Nothing heard']);
    const broken = host();
    broken.transcribe = vi.fn(async () => {
      throw new Error('no account has Gemini quota for dictation right now');
    });
    const failing = createDictation(broken, environment(null).env);
    await failing.toggle();
    await failing.toggle();
    expect(broken.log).toEqual(['fail:no account has Gemini quota for dictation right now']);
    expect(get(failing.state)).toBe('idle');
    const denied = host();
    const { env } = environment(null);
    env.microphone = vi.fn(async () => {
      throw new DOMException('Permission denied', 'NotAllowedError');
    });
    await createDictation(denied, env).toggle();
    expect(denied.log).toEqual(['fail:No microphone access: Permission denied']);
    const unsupported = host();
    const missing = environment(null);
    missing.env.recorder = () => {
      throw new Error('MediaRecorder is not defined');
    };
    await createDictation(unsupported, missing.env).toggle();
    expect(unsupported.log).toEqual([
      'fail:Recording is not available: MediaRecorder is not defined',
    ]);
    expect(missing.log).toEqual(['track']);
  });
  it('ignores the mic while transcribing and stays quiet on aborted or silent recognition', async () => {
    const h = host();
    let finish: (value: { text: string }) => void = () => {};
    h.transcribe = vi.fn(
      () =>
        new Promise<{ text: string }>((resolve) => {
          finish = resolve;
        }),
    );
    const { env, microphone } = environment(null);
    const dictation = createDictation(h, env);
    await dictation.toggle();
    const stopping = dictation.toggle();
    await vi.waitFor(() => expect(get(dictation.state)).toBe('transcribing'));
    await dictation.toggle();
    expect(microphone).toHaveBeenCalledTimes(1);
    expect(await dictation.cancel()).toBe(false);
    finish({ text: 'Done.' });
    await stopping;
    expect(h.log).toEqual(['insert:Done.']);
    const quiet = host();
    const speaking = createDictation(quiet, environment(FakeRecognition).env);
    for (const error of ['aborted', 'no-speech']) {
      await speaking.toggle();
      FakeRecognition.last!.onerror?.({ error });
      expect(get(speaking.state)).toBe('idle');
    }
    expect(quiet.log).toEqual([]);
  });
  it('cancelling a recording releases the microphone and audio graph', async () => {
    const h = host();
    const { env, log } = environment(null);
    const dictation = createDictation(h, env);
    await dictation.toggle();
    expect(await dictation.cancel()).toBe(true);
    expect(log).toEqual(['meter', 'start', 'stop', 'source', 'track', 'context']);
    expect(h.transcribe).not.toHaveBeenCalled();
    expect(get(dictation.state)).toBe('idle');
  });
});
