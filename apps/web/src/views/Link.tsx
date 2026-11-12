import type { ReactNode } from "react";
import type { Intent } from "../store/msg";
import { type Route, routePath } from "../store/route";

/// An anchor that keeps the URL honest (middle click works) but navigates through the store.
export function NavLink({ route, dispatch, className, children, testId, title, row }: {
  route: Route;
  dispatch: (i: Intent) => void;
  className?: string;
  children: ReactNode;
  testId?: string;
  title?: string;
  /// The link is a row of the sidebar's tree: the keys of the tree move between such rows.
  row?: boolean;
}) {
  return (
    <a
      className={className}
      data-testid={testId}
      title={title}
      data-row={row || undefined}
      href={`${routePath(route)}${window.location.search}`}
      onClick={(e) => {
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
        e.preventDefault();
        dispatch({ type: "navigate", route });
      }}
    >
      {children}
    </a>
  );
}
