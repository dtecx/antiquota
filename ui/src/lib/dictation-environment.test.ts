/** Proves platform recording preferences without accessing a real microphone or recorder. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { browserEnvironment } from './dictation';

function fixture(native: boolean, userAgent: string, supported: string[], language = 'en-GB') {
  const constructed: { stream: MediaStream; options: MediaRecorderOptions | undefined }[] = [];
  const stream = Object.freeze({ synthetic: true }) as unknown as MediaStream;
  const microphone = vi.fn<(constraints: MediaStreamConstraints) => Promise<MediaStream>>(
    async () => stream,
  );
  const primarySpeech = vi.fn(),
    webkitSpeech = vi.fn();
  class MockRecorder {
    static isTypeSupported = vi.fn((type: string) => supported.includes(type));
    constructor(input: MediaStream, options?: MediaRecorderOptions) {
      constructed.push({ stream: input, options });
    }
  }
  vi.stubGlobal('window', {
    ...(native ? { __TAURI__: {} } : {}),
    SpeechRecognition: primarySpeech,
    webkitSpeechRecognition: webkitSpeech,
  });
  vi.stubGlobal('navigator', {
    userAgent,
    language,
    mediaDevices: { getUserMedia: microphone },
  });
  vi.stubGlobal('MediaRecorder', MockRecorder);
  return {
    env: browserEnvironment(),
    stream,
    microphone,
    constructed,
    primarySpeech,
    webkitSpeech,
  };
}
afterEach(() => vi.unstubAllGlobals());

describe('dictation platform environment', () => {
  it('disables both speech services in native Mac mode and prefers specific AAC over every other supported format', () => {
    const value = fixture(true, 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7)', [
      'audio/webm;codecs=opus',
      'audio/webm',
      'audio/mp4',
      'audio/mp4;codecs=mp4a.40.2',
    ]);
    expect(value.env.speech).toBeNull();
    value.env.recorder(value.stream);
    expect(value.constructed).toEqual([
      { stream: value.stream, options: { mimeType: 'audio/mp4;codecs=mp4a.40.2' } },
    ]);
    expect(value.microphone).not.toHaveBeenCalled();
    expect(value.primarySpeech).not.toHaveBeenCalled();
    expect(value.webkitSpeech).not.toHaveBeenCalled();
  });
  it('prefers plain MP4 on native Mac when the specific AAC type is unavailable', () => {
    const value = fixture(true, 'Macintosh', ['audio/webm;codecs=opus', 'audio/mp4']);
    value.env.recorder(value.stream);
    expect(value.constructed[0]?.options).toEqual({ mimeType: 'audio/mp4' });
    expect(value.microphone).not.toHaveBeenCalled();
  });
  it.each([
    [false, 'Macintosh'],
    [true, 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)'],
  ])('keeps WebM first for native=%s platform=%s', (native, userAgent) => {
    const value = fixture(native, userAgent, [
      'audio/webm;codecs=opus',
      'audio/mp4;codecs=mp4a.40.2',
      'audio/mp4',
    ]);
    value.env.recorder(value.stream);
    expect(value.constructed[0]?.options).toEqual({ mimeType: 'audio/webm;codecs=opus' });
    expect(value.env.speech).toBe(native ? null : value.primarySpeech);
    expect(value.microphone).not.toHaveBeenCalled();
  });
  it('falls back from native Mac AAC and MP4 to a supported WebM type', () => {
    const value = fixture(true, 'Mac OS X', ['audio/webm']);
    value.env.recorder(value.stream);
    expect(value.constructed[0]?.options).toEqual({ mimeType: 'audio/webm' });
  });
  it('leaves the format to MediaRecorder when no declared type is supported', () => {
    const value = fixture(true, 'Macintosh', []);
    value.env.recorder(value.stream);
    expect(value.constructed).toEqual([{ stream: value.stream, options: undefined }]);
  });
  it('uses prefixed speech only when the ordinary browser service is absent', () => {
    const value = fixture(false, 'Macintosh', []);
    vi.stubGlobal('window', { webkitSpeechRecognition: value.webkitSpeech });
    expect(browserEnvironment().speech).toBe(value.webkitSpeech);
    vi.stubGlobal('window', {});
    expect(browserEnvironment().speech).toBeNull();
  });
  it('forwards microphone constraints only on explicit acquisition and defaults an empty language', async () => {
    const value = fixture(false, 'Synthetic browser', [], '');
    expect(value.env.language).toBe('en-US');
    expect(value.microphone).not.toHaveBeenCalled();
    const constraints = { audio: { channelCount: 1, echoCancellation: true } };
    expect(await value.env.microphone(constraints)).toBe(value.stream);
    expect(value.microphone).toHaveBeenCalledExactlyOnceWith(constraints);
  });
});
