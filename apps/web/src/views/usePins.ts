import { useCallback, useEffect, useState } from "react";
import { get, post } from "../api/http";
import type { Pin } from "../store/pins";

/// The pins of a project, read again when `version` changes (the newest pin event of the
/// journal), and the call that answers one in place.
export function usePins(project: string | undefined, version: number) {
  const [pins, setPins] = useState<Pin[]>([]);
  const base = project ? `/projects/${encodeURIComponent(project)}/design-pins` : undefined;
  useEffect(() => {
    if (!base) return;
    let live = true;
    get<Pin[]>(base).then((l) => live && setPins(l)).catch(() => undefined);
    return () => { live = false; };
  }, [base, version]);
  const answer = useCallback(async (id: string, text: string) => {
    const done = await post<Pin>(`${base}/${id}/answer`, { text });
    setPins((all) => all.map((p) => (p.id === done.id ? done : p)));
  }, [base]);
  return { pins, answer };
}
