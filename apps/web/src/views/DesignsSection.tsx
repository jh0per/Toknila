import { type DragEvent, useCallback, useEffect, useRef, useState } from "react";
import { get, patch, post, token } from "../api/http";
import { sendEmpty, uploadDesign } from "../api/designs";
import type { Design, DesignList, Project } from "../api/types";
import { dayText, fileUrl, kindLabel, placeholder, refusedFiles, sizeText } from "../store/designs";
import { DesignViewer } from "./DesignViewer";
import { usePins } from "./usePins";
import { FigmaTokenBox } from "./FigmaTokenBox";

const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// Designs: the project's library of images, pdfs and Figma frame links. Files are dropped on the
/// section or picked; a Figma link is pasted. Each card shows a thumbnail with its name, kind and
/// day, and renames, removes or opens large.
export function DesignsSection({ project, pinSeq = 0 }: {
  project: Project;
  /// The newest pin event of the journal: the pins are read again when it changes.
  pinSeq?: number;
}) {
  const base = `/projects/${project.key}/designs`;
  const { pins, answer } = usePins(project.key, pinSeq);
  const [list, setList] = useState<DesignList | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const [over, setOver] = useState(false);
  const [link, setLink] = useState("");
  const [open, setOpen] = useState<Design | undefined>();
  const picker = useRef<HTMLInputElement>(null);
  const load = useCallback(async () => {
    try {
      setList(await get<DesignList>(base));
    } catch (e) {
      setError(messageOf(e));
    }
  }, [base]);
  useEffect(() => {
    void load();
  }, [load]);
  const run = async (what: () => Promise<unknown>) => {
    setBusy(true);
    setError(undefined);
    try {
      await what();
    } catch (e) {
      setError(messageOf(e));
    } finally {
      await load();
      setBusy(false);
    }
  };
  const addFiles = (files: File[]) => {
    const refused = refusedFiles(files);
    const ok = files.filter((f) => !refusedFiles([f]));
    void run(async () => {
      for (const f of ok) await uploadDesign(project.key, f);
      if (refused) setError(refused);
    });
  };
  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setOver(false);
    const files = [...e.dataTransfer.files];
    if (files.length > 0) return addFiles(files);
    const text = e.dataTransfer.getData("text/uri-list") || e.dataTransfer.getData("text/plain");
    if (text.includes("figma.com")) void addLink(text.trim());
  };
  const addLink = (url: string) =>
    run(async () => {
      await post(`${base}/figma`, { url });
      setLink("");
    });
  return (
    <div className="page-section" data-testid="project-designs">
      <section
        className={`card-block design-drop${over ? " over" : ""}`} data-testid="designs-drop"
        onDragOver={(e) => { e.preventDefault(); setOver(true); }} onDragLeave={() => setOver(false)} onDrop={onDrop}
      >
        <h2>Designs</h2>
        <p className="muted">Drop png, jpg, webp or pdf files here, or paste the link of a Figma frame.</p>
        <div className="design-add">
          <button type="button" className="btn" disabled={busy} data-testid="designs-pick" onClick={() => picker.current?.click()}>
            Add files…
          </button>
          <input
            ref={picker} type="file" multiple hidden data-testid="designs-file-input"
            accept="image/png,image/jpeg,image/webp,application/pdf,.png,.jpg,.jpeg,.webp,.pdf"
            onChange={(e) => { addFiles([...(e.target.files ?? [])]); e.target.value = ""; }}
          />
          <input
            className="design-link" placeholder="https://www.figma.com/design/…?node-id=…" spellCheck={false}
            value={link} data-testid="figma-url" aria-label="Figma frame link" onChange={(e) => setLink(e.target.value)}
          />
          <button type="button" className="btn" disabled={busy || !link.trim()} data-testid="figma-add" onClick={() => void addLink(link)}>
            Add link
          </button>
        </div>
        {error && <p className="field-error" role="alert" data-testid="designs-error">{error}</p>}
      </section>
      {list && list.designs.length === 0 && <p className="muted" data-testid="designs-none">No designs yet.</p>}
      {list && list.designs.length > 0 && (
        <ul className="design-grid" data-testid="designs-list">
          {list.designs.map((d) => (
            <DesignCard
              key={d.id} design={d} project={project.key} busy={busy} onOpen={() => setOpen(d)}
              onRename={(name) => run(() => patch(`${base}/${d.id}`, { name }))}
              onRemove={() => run(() => sendEmpty("DELETE", `${base}/${d.id}`))}
              onFetch={list.figma_token && d.kind === "figma" && !d.has_file ? () => run(() => post(`${base}/${d.id}/fetch`, {})) : undefined}
            />
          ))}
        </ul>
      )}
      <FigmaTokenBox project={project.key} saved={list?.figma_token ?? false} onChange={load} />
      {open && (
        <DesignViewer
          project={project.key} design={open} onClose={() => setOpen(undefined)}
          pins={pins.filter((p) => p.design === open.id)} onAnswer={answer}
        />
      )}
    </div>
  );
}

function DesignCard({ design: d, project, busy, onOpen, onRename, onRemove, onFetch }: {
  design: Design;
  project: string;
  busy: boolean;
  onOpen: () => void;
  onRename: (name: string) => Promise<void>;
  onRemove: () => Promise<void>;
  onFetch?: () => Promise<void>;
}) {
  const [name, setName] = useState<string | undefined>();
  const [sure, setSure] = useState(false);
  const note = placeholder(d);
  return (
    <li className="design-card" data-testid="design-card" data-name={d.name}>
      <button type="button" className="design-thumb" data-testid="design-open" aria-label={`Open ${d.name}`} onClick={onOpen}>
        {d.thumb ? <img src={fileUrl(project, d, token())} alt={d.name} loading="lazy" data-testid="design-thumb" /> : <span className="design-note">{note}</span>}
      </button>
      {name === undefined ? (
        <strong className="design-name" data-testid="design-name">{d.name}</strong>
      ) : (
        <input
          autoFocus value={name} data-testid="design-rename-input" aria-label="New name" onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setName(undefined);
            if (e.key === "Enter") void onRename(name).then(() => setName(undefined));
          }}
        />
      )}
      <span className="muted design-meta" data-testid="design-meta">
        {kindLabel(d)}{d.has_file ? ` · ${sizeText(d.size)}` : ""} · {dayText(d.created_at)}
      </span>
      {d.kind === "figma" && !d.has_file && d.thumb === false && d.hint && <span className="muted design-hint" data-testid="design-hint">{d.hint}</span>}
      <span className="design-actions">
        {onFetch && <button type="button" className="btn" disabled={busy} data-testid="design-fetch" onClick={() => void onFetch()}>Fetch image</button>}
        <button type="button" className="btn" disabled={busy} data-testid="design-rename" onClick={() => setName(d.name)}>Rename</button>
        {sure ? (
          <button type="button" className="btn danger" disabled={busy} data-testid="design-remove-sure" onClick={() => void onRemove()}>Really remove</button>
        ) : (
          <button type="button" className="btn" disabled={busy} data-testid="design-remove" onClick={() => setSure(true)}>Remove</button>
        )}
      </span>
    </li>
  );
}
