import { useCallback, useEffect, useRef, useState } from "react";
import {
  DICTATE, ENDED_AT_ONCE, FAST_END_MS, type Heard, type Mic, type Piece, fieldText, hear, micFailure, settledText, shouldAutoSend, speechLang, startHeard,
} from "../store/speech";

/// The part of the Web Speech recognizer this uses (the page's own, or the one a test injects).
interface Recognizer {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((e: { results: ArrayLike<ArrayLike<{ transcript: string }> & { isFinal: boolean }> }) => void) | null;
  onerror: ((e: { error: string; message?: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
}
type RecognizerClass = new () => Recognizer;

/// The system's recognizer class, when the window has one.
export function recognizerClass(): RecognizerClass | undefined {
  const w = window as unknown as { SpeechRecognition?: RecognizerClass; webkitSpeechRecognition?: RecognizerClass };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition;
}

const AUTO_KEY = "toknila.sendOnPause";
const readAuto = (): boolean => {
  try {
    return window.localStorage.getItem(AUTO_KEY) === "1";
  } catch {
    return false;
  }
};

/// What the box shows of listening: the button's state, the words not yet settled, the sentence
/// for a refusal or a failure, and the setting "send when I stop speaking".
export interface Speech {
  mic: Mic;
  interim: string;
  message: string;
  auto: boolean;
  setAuto: (on: boolean) => void;
  /// Press to listen, press again to stop.
  toggle: () => void;
  /// Stop listening and keep the words (a person typing takes the box over).
  stop: () => void;
}

/// Listens for one box: the transcript joins what is typed (`text`) through `setText`; with the
/// setting on, a pause of two seconds calls `send`. The language is the window's.
export function useSpeech(text: string, setText: (t: string) => void, send: () => void): Speech {
  const Cls = recognizerClass();
  const [mic, setMic] = useState<Mic>(Cls ? "idle" : "none");
  const [message, setMessage] = useState(Cls ? "" : DICTATE);
  const [interim, setInterim] = useState("");
  const [auto, setAutoState] = useState(readAuto);
  const rec = useRef<Recognizer | null>(null);
  const heard = useRef<Heard>(startHeard(""));
  const lastAt = useRef(0);
  const startedAt = useRef(0);
  const reported = useRef(false);
  const latest = useRef({ text, setText, send });
  latest.current = { text, setText, send };

  const finish = useCallback((flush: boolean) => {
    const r = rec.current;
    rec.current = null;
    if (r) {
      r.onresult = null;
      r.onerror = null;
      r.onend = null;
      try { r.stop(); } catch { /* it had stopped already */ }
    }
    if (flush && heard.current.interim) latest.current.setText(fieldText(heard.current));
    setInterim("");
    setMic((m) => (m === "listening" ? "idle" : m));
  }, []);

  const start = () => {
    if (!Cls) return;
    const r = new Cls();
    r.lang = speechLang(navigator);
    r.continuous = true;
    r.interimResults = true;
    heard.current = startHeard(latest.current.text);
    lastAt.current = Date.now();
    r.onresult = (e) => {
      const pieces: Piece[] = [];
      for (let i = 0; i < e.results.length; i++) pieces.push({ text: e.results[i][0].transcript, final: e.results[i].isFinal });
      reported.current = true;
      heard.current = hear(heard.current, pieces);
      lastAt.current = Date.now();
      latest.current.setText(settledText(heard.current));
      setInterim(heard.current.interim);
    };
    r.onerror = (e) => {
      reported.current = true;
      const f = micFailure(e.error, e.message);
      finish(true);
      setMic(f.mic);
      setMessage(f.message);
    };
    r.onend = () => {
      const atOnce = !reported.current && Date.now() - startedAt.current < FAST_END_MS;
      finish(true);
      if (atOnce) {
        setMic("error");
        setMessage(ENDED_AT_ONCE);
      }
    };
    startedAt.current = Date.now();
    reported.current = false;
    rec.current = r;
    setMessage("");
    setMic("listening");
    try {
      r.start();
    } catch (e) {
      finish(false);
      setMic("error");
      setMessage(`Could not listen (${e instanceof Error ? e.message : "start failed"}). ${DICTATE}`);
    }
  };

  useEffect(() => {
    if (mic !== "listening") return;
    const t = window.setInterval(() => {
      const h = heard.current;
      const words = fieldText(h);
      if (!shouldAutoSend(auto, words, lastAt.current, Date.now())) return;
      latest.current.setText(words);
      finish(false);
      latest.current.send();
    }, 250);
    return () => window.clearInterval(t);
  }, [mic, auto, finish]);

  useEffect(() => () => finish(false), [finish]);

  return {
    mic, interim, message, auto,
    setAuto: (on) => {
      setAutoState(on);
      try { window.localStorage.setItem(AUTO_KEY, on ? "1" : "0"); } catch { /* the choice lasts for this visit */ }
    },
    toggle: () => (mic === "listening" ? finish(true) : start()),
    stop: () => finish(false),
  };
}
