/// The outline icon of a sidebar row: one inline SVG per name, 16px, drawn with the row's colour.
const PATHS = {
  roadmap: <path d="M2 3h12M2 8h8M2 13h10" />,
  assistant: <path d="M2 3h12v8H7l-3 3v-3H2z" />,
  inbox: <path d="M2 9l2-6h8l2 6v4H2zM2 9h4l1 2h2l1-2h4" />,
  agents: <><rect x="3" y="5" width="10" height="8" rx="2" /><path d="M8 2v3M6 9h.01M10 9h.01" /></>,
  chains: <path d="M6.5 9.5l3-3M5 8L3.5 9.5a2.1 2.1 0 003 3L8 11M11 8l1.5-1.5a2.1 2.1 0 00-3-3L8 5" />,
  designs: <><rect x="2" y="2" width="12" height="12" rx="2" /><path d="M2 6h12M6 6v8" /></>,
  settings: <><circle cx="8" cy="8" r="2.2" /><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" /></>,
  usage: <path d="M3 13V8M8 13V3M13 13V6" />,
  guide: <path d="M3 2.5h7a3 3 0 013 3v8H6a3 3 0 01-3-3zM3 10.5a3 3 0 013-3h7" />,
  shortcuts: <><rect x="1.5" y="4" width="13" height="8" rx="2" /><path d="M4 7h.01M7 7h.01M10 7h.01M12 7h.01M5 10h6" /></>,
};

export type IconName = keyof typeof PATHS;

export function NavIcon({ name }: { name: IconName }) {
  return (
    <svg className="ico" viewBox="0 0 16 16" aria-hidden="true" focusable="false" data-icon={name}>
      {PATHS[name]}
    </svg>
  );
}
