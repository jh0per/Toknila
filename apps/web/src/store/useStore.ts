// Glue between the pure reducer, the effect runner and React.

import { useCallback, useEffect, useRef, useState } from "react";
import { effectRunner } from "./effects";
import type { Effect, Intent, Msg } from "./msg";
import { reduce } from "./reduce";
import { parseRoute } from "./route";
import { type State, initialState } from "./state";
import { readAssistant } from "./assistantEffects";
import { readTheme } from "./themeEffects";

export function useStore(): [State, (intent: Intent) => void] {
  const [state, setState] = useState(() => ({ ...initialState(parseRoute(window.location.pathname)), theme: readTheme(), assistant: readAssistant() }));
  const current = useRef(state);
  const runner = useRef<{ run: (e: Effect) => void; stop: () => void } | null>(null);

  const dispatch = useCallback((msg: Msg) => {
    const [next, effects] = reduce(current.current, msg);
    current.current = next;
    setState(next);
    for (const e of effects) runner.current?.run(e);
  }, []);

  useEffect(() => {
    runner.current = effectRunner(dispatch);
    const onPop = () => dispatch({ type: "urlChanged", route: parseRoute(window.location.pathname) });
    // The desktop menu asks for a form through the address hash (`#/add-project`).
    const onHash = () => dispatch({ type: "hashChanged", hash: window.location.hash });
    window.addEventListener("popstate", onPop);
    window.addEventListener("hashchange", onHash);
    dispatch({ type: "started" });
    if (window.location.hash) onHash();
    return () => {
      window.removeEventListener("popstate", onPop);
      window.removeEventListener("hashchange", onHash);
      runner.current?.stop();
      runner.current = null;
    };
  }, [dispatch]);

  return [state, dispatch];
}
