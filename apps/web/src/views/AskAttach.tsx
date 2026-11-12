import { type ClipboardEvent, type DragEvent, type ReactNode, useRef, useState } from "react";
import { token } from "../api/http";
import type { Intent } from "../store/msg";
import { placeOf } from "../store/askAttach";
import { thumbUrl } from "../store/designs";
import type { State } from "../store/state";
import { pastedImages } from "./CardDesigns";

/// Where the pictures of one message go: the box's place in the store's drafts, the project, and
/// what a pasted picture is named after (a card's key, or the project's key).
export interface AskTarget {
  project: string;
  where: string;
  prefix: string;
}

/// The attachments of one box: what it holds, and the handlers that add files to it by drop, paste
/// or pick. While an upload runs, new files wait.
export function useAskAttach(state: State, dispatch: (i: Intent) => void, t: AskTarget) {
  const [over, setOver] = useState(false);
  const place = placeOf(state, t.where);
  const add = (files: File[], pasted?: boolean) =>
    !place.busy && files.length > 0 && dispatch({ type: "askAttach", act: "add", ...t, files, pasted });
  const frame = {
    onDragOver: (e: DragEvent) => { e.preventDefault(); setOver(true); },
    onDragLeave: () => setOver(false),
    onDrop: (e: DragEvent) => { e.preventDefault(); setOver(false); add([...e.dataTransfer.files]); },
    onPaste: (e: ClipboardEvent) => {
      const files = pastedImages(e.clipboardData);
      if (files.length === 0) return;
      e.preventDefault();
      add(files, true);
    },
  };
  return { place, add, frame, over, remove: (design: string) => dispatch({ type: "askAttach", act: "remove", where: t.where, design }) };
}

/// The small attach button: it picks images or pdfs from the disk.
export function AttachButton({ busy, onFiles, testId }: { busy: boolean; onFiles: (files: File[]) => void; testId: string }) {
  const picker = useRef<HTMLInputElement>(null);
  return (
    <>
      <button
        type="button" className="btn icon attach-btn" data-testid={`${testId}-attach`} disabled={busy}
        aria-label="Attach images or PDFs" title="Attach images or PDFs" onClick={() => picker.current?.click()}
      >
        <span aria-hidden="true">📎</span>
      </button>
      <input
        ref={picker} type="file" multiple hidden data-testid={`${testId}-file-input`}
        accept="image/png,image/jpeg,image/webp,application/pdf"
        onChange={(e) => { onFiles([...(e.target.files ?? [])]); e.target.value = ""; }}
      />
    </>
  );
}

/// The chips under a box (a thumbnail and the library's name each, with a way to take it off the
/// message), the upload on its way and the sentence about a refused file.
export function AskChips({ a, project, testId }: { a: ReturnType<typeof useAskAttach>; project: string; testId: string }) {
  const { chips, busy, refused } = a.place;
  if (chips.length === 0 && !busy && !refused) return null;
  return (
    <div className="ask-attached" data-testid={`${testId}-attached`}>
      {(chips.length > 0 || busy) && (
        <ul className="ask-chips">
          {chips.map((d) => (
            <li key={d.id} className="ask-chip" data-testid={`${testId}-chip`}>
              {d.thumb ? <img src={thumbUrl(project, d, token())} alt="" /> : <span className="ask-chip-mark" aria-hidden="true">{d.kind === "pdf" ? "PDF" : "▣"}</span>}
              <span className="ask-chip-name">{d.name}</span>
              <button
                type="button" className="ask-chip-x" data-testid={`${testId}-chip-remove`} aria-label={`Take ${d.name} off the message`}
                onClick={() => a.remove(d.id)}
              >
                ✕
              </button>
            </li>
          ))}
          {busy && <li className="ask-chip busy" role="status" data-testid={`${testId}-uploading`}>Uploading…</li>}
        </ul>
      )}
      {refused && <p className="error" role="alert" data-testid={`${testId}-refused`}>{refused}</p>}
    </div>
  );
}

/// A wrapper that makes a box take dropped files, with the hint while a drag is over it.
export function DropFrame({ a, className, testId, children }: { a: ReturnType<typeof useAskAttach>; className: string; testId?: string; children: ReactNode }) {
  return (
    <div className={`${className}${a.over ? " drop-over" : ""}`} data-testid={testId} {...a.frame}>
      {children}
      {a.over && <span className="ask-drop-hint">Drop to attach</span>}
    </div>
  );
}
