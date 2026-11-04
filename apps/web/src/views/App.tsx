import { useEffect, useState } from "react";
import { token } from "../api/http";
import { humanOnly, visibleChains } from "../store/state";
import { useStore } from "../store/useStore";
import { ChainCard } from "./ChainCard";

function useNow(ms: number): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

export function App() {
  const [state, dispatch] = useStore();
  const now = useNow(1000);
  if (!token()) {
    return <main><p className="error">Open the address that <code>tok open</code> prints; it carries the access token.</p></main>;
  }
  const chains = visibleChains(state);
  const onlyHuman = humanOnly(state);
  return (
    <main>
      <header className="top">
        <h1>
          <a
            href={`/${window.location.search}`}
            onClick={(e) => {
              e.preventDefault();
              dispatch({ type: "navigate", route: { name: "chains" } });
            }}
          >
            Toknila
          </a>
        </h1>
        <span className={`link l-${state.link}`} data-testid="link">{state.link}</span>
        <label>
          <input
            type="checkbox"
            checked={onlyHuman}
            disabled={state.route.name === "inbox"}
            onChange={(e) => dispatch({ type: "toggleNeedsHuman", on: e.target.checked })}
          />
          needs a human
        </label>
      </header>
      {state.error && <p className="error">{state.error}</p>}
      {!state.loaded ? <p>Loading…</p> : chains.length === 0 ? (
        <p className="empty">No chains yet. Try <code>tok chain import examples/demo.toml</code>.</p>
      ) : (
        chains.map((c) => (
          <ChainCard
            key={c.chain.id}
            c={c}
            now={now}
            needsHuman={state.needsHuman}
            onlyHuman={onlyHuman}
            open={(route) => dispatch({ type: "navigate", route })}
          />
        ))
      )}
    </main>
  );
}
