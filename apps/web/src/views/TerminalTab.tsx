import { useState } from "react";
import type { Ticket } from "../api/types";
import { openTerminal } from "../terminal/connect";
import { LiveTerminal } from "./LiveTerminal";

/// The card's runner terminal: the worker's, or the reviewer's.
export function TerminalTab({ card }: { card: Ticket }) {
  const [role, setRole] = useState<"worker" | "reviewer">("worker");
  const lead = (
    <>
      {(["worker", "reviewer"] as const).map((r) => (
        <button key={r} className={role === r ? "primary" : ""} data-testid={`term-role-${r}`} onClick={() => setRole(r)}>
          {r === "worker" ? "Worker" : "Reviewer"}
        </button>
      ))}{" "}
    </>
  );
  return (
    <LiveTerminal
      streamKey={`${card.key}:${role}`}
      connect={(sink) => openTerminal(card.key, sink, role)}
      noneText="no runner yet"
      placeholder="No runner yet. Start the story to see its terminal."
      lead={lead}
    />
  );
}
