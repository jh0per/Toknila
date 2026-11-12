// The one Node call tests make: read a stylesheet as text (vitest returns an empty string for a
// CSS file imported with ?raw). The web build has no @types/node.
declare module "node:fs" {
  export function readFileSync(path: URL, encoding: "utf8"): string;
}
