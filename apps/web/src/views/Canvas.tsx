import type { ChainDetail } from "../api/types";
import { layoutChain, nodeKey } from "../layout";
import { outOfOrderText } from "../format";
import { statusGroup, statusText } from "../store/groups";
import type { Intent } from "../store/msg";
import { type State, currentProject, hrefOf, queuePosition } from "../store/state";
import { PrChip } from "./PrChip";
import { UsageChip } from "./UsageChip";

const W = 196;
const H = 84;
const GAP_X = 44;
const GAP_Y = 20;
const PAD = 16;
const x = (col: number) => PAD + col * (W + GAP_X);
const y = (lane: number) => PAD + lane * (H + GAP_Y);

/// The chain canvas: cards left to right in dependency order, releases as their own nodes.
export function Canvas({ chain, state, dispatch }: {
  chain: ChainDetail;
  state: State;
  dispatch: (i: Intent) => void;
}) {
  const layout = layoutChain(chain);
  const projectKey = currentProject(state)?.key ?? chain.chain.project_id;
  const at = new Map(layout.nodes.map((n) => [nodeKey(n), n]));
  const tickets = new Map(chain.tickets.map((t) => [t.id, t]));
  const releases = new Map(chain.releases.map((r) => [r.id, r]));
  const selected = state.route.name === "card" ? state.route.key : "";
  const width = PAD * 2 + layout.columns * W + Math.max(0, layout.columns - 1) * GAP_X;
  const height = PAD * 2 + layout.lanes * H + Math.max(0, layout.lanes - 1) * GAP_Y;
  return (
    <div className="canvas" data-testid="canvas" data-chain={chain.chain.name}>
      <div className="canvas-inner" style={{ width, height }}>
        <svg width={width} height={height} aria-hidden="true">
          {layout.edges.map((e) => {
            const a = at.get(e.from);
            const b = at.get(e.to);
            if (!a || !b) return null;
            const x1 = x(a.col) + W;
            const y1 = y(a.lane) + H / 2;
            const x2 = x(b.col);
            const y2 = y(b.lane) + H / 2;
            const mid = (x1 + x2) / 2;
            return <path key={`${e.from}>${e.to}`} d={`M${x1},${y1} C${mid},${y1} ${mid},${y2} ${x2},${y2}`} />;
          })}
        </svg>
        {layout.nodes.map((n) => {
          const style = { left: x(n.col), top: y(n.lane), width: W, height: H };
          if (n.kind === "release") {
            const r = releases.get(n.id);
            if (!r) return null;
            return (
              <div key={nodeKey(n)} className="node release" data-version={r.version} style={style}>
                <b className="key">◆ {r.version}</b>
                <span className="muted">{r.channel} · {r.start_mode}</span>
                <small className="muted" data-testid="release-state">{r.state}</small>
              </div>
            );
          }
          const t = tickets.get(n.id);
          if (!t) return null;
          const route = { name: "card" as const, project: projectKey, chain: chain.chain.name, key: t.key };
          return (
            <a
              key={nodeKey(n)}
              className={`node ticket g-${statusGroup(t.status)}${selected === t.key ? " sel" : ""}`}
              data-key={t.key}
              href={hrefOf(route)}
              style={style}
              onClick={(e) => {
                e.preventDefault();
                dispatch({ type: "selectCard", project: route.project, chain: route.chain, key: t.key });
              }}
            >
              <span className="row">
                <span className="title">{t.title}</span>
                {t.profile && <span className="runner">{t.profile}</span>}
              </span>
              <span className="row">
                <span className="st" data-testid="status">{statusText(t.status)}</span>
                <UsageChip figures={chain.usage?.[t.key]} />
                <span className="key">{t.key}</span>
              </span>
              {queuePosition(state, t.id) !== undefined && (
                <span className="mark queued" data-testid="queued" title="Waiting for a free agent slot">
                  queued #{queuePosition(state, t.id)}
                </span>
              )}
              {t.pr && <PrChip pr={t.pr} />}
              {t.done_out_of_order && (
                <span className="mark" title={outOfOrderText(t.done_out_of_order)}>out of order</span>
              )}
            </a>
          );
        })}
      </div>
    </div>
  );
}
