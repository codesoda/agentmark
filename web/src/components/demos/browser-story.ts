"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const REDUCED = "(prefers-reduced-motion: reduce)";

function reducedNow() {
  return window.matchMedia(REDUCED).matches;
}

/**
 * Drives a demo through numbered beats on a timeline.
 *
 * `times[i]` is when beat `i` starts, in ms from the start of a run. The last
 * beat is the settled final state, which is also the initial state, so the
 * server render and no-JS view show the finished story. After hydration the
 * demo is primed to beat 0, then plays once the first time it is in view.
 * If it scrolls out of view mid-run it jumps to the end rather than playing
 * unseen. With reduced motion it never leaves the final beat.
 */
export function useStory({
  times,
  inView,
  reduced,
  onBeat,
}: {
  times: readonly number[];
  inView: boolean;
  reduced: boolean;
  /** Runs for every beat, from timers and from `play`/`settle`. */
  onBeat?: (beat: number) => void;
}) {
  const last = times.length - 1;
  const [beat, setBeat] = useState(last);
  const timers = useRef<number[]>([]);
  // The ref is read by timers, the state by render.
  const played = useRef(false);
  const [hasPlayed, setHasPlayed] = useState(false);
  const onBeatRef = useRef(onBeat);

  useEffect(() => {
    onBeatRef.current = onBeat;
  });

  const go = useCallback((b: number) => {
    setBeat(b);
    onBeatRef.current?.(b);
  }, []);

  const clear = useCallback(() => {
    timers.current.forEach((id) => window.clearTimeout(id));
    timers.current = [];
  }, []);

  /** Runs the story from beat `from` to the end. With reduced motion it jumps straight there. */
  const play = useCallback(
    (from = 0) => {
      clear();
      played.current = true;
      setHasPlayed(true);
      if (reducedNow()) {
        go(last);
        return;
      }
      go(from);
      for (let b = from + 1; b <= last; b++) {
        timers.current.push(window.setTimeout(() => go(b), times[b] - times[from]));
      }
    },
    [clear, go, last, times],
  );

  /** Stops any run and shows the final state. */
  const settle = useCallback(() => {
    clear();
    go(last);
  }, [clear, go, last]);

  useEffect(() => clear, [clear]);

  // Prime once after hydration, so the first run has something to reveal.
  useEffect(() => {
    if (reduced) return;
    const id = requestAnimationFrame(() => {
      if (!played.current && !reducedNow()) go(0);
    });
    return () => cancelAnimationFrame(id);
  }, [go, reduced]);

  const running = hasPlayed && beat < last;

  useEffect(() => {
    if (reduced) {
      const id = window.setTimeout(settle, 0);
      return () => window.clearTimeout(id);
    }
    if (inView && !hasPlayed) {
      const id = window.setTimeout(() => play(0), 0);
      return () => window.clearTimeout(id);
    }
    if (!inView && running) {
      const id = window.setTimeout(settle, 0);
      return () => window.clearTimeout(id);
    }
  }, [inView, reduced, running, hasPlayed, play, settle]);

  return { beat, last, running, play, settle } as const;
}

/** Owns a set of timeouts and clears them on unmount. */
export function useTimeouts() {
  const ids = useRef<number[]>([]);
  const clear = useCallback(() => {
    ids.current.forEach((id) => window.clearTimeout(id));
    ids.current = [];
  }, []);
  const after = useCallback((ms: number, fn: () => void) => {
    ids.current.push(window.setTimeout(fn, ms));
  }, []);
  useEffect(() => clear, [clear]);
  return { after, clear } as const;
}
