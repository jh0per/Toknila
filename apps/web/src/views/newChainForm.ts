// The New epic dialog's pure parts: the slug a title gives, and the request the form sends.
// The slug preview mirrors core's `slug_from_title`; the daemon stays the judge (it derives the
// slug itself when none comes, and refuses a bad one).

import type { NewChain, ReviewMode } from "../api/types";

const TRANSLIT: Record<string, string> = {
  а: "a", б: "b", в: "v", г: "h", ґ: "g", д: "d", е: "e", є: "ie", ж: "zh", з: "z", и: "y", ы: "y",
  і: "i", ї: "i", й: "i", к: "k", л: "l", м: "m", н: "n", о: "o", п: "p", р: "r", с: "s", т: "t",
  у: "u", ф: "f", х: "kh", ц: "ts", ч: "ch", ш: "sh", щ: "shch", э: "e", ю: "iu", я: "ia", ё: "e",
  à: "a", á: "a", â: "a", ä: "a", ã: "a", å: "a", è: "e", é: "e", ê: "e", ë: "e",
  ì: "i", í: "i", î: "i", ï: "i", ò: "o", ó: "o", ô: "o", ö: "o", õ: "o",
  ù: "u", ú: "u", û: "u", ü: "u", ç: "c", ñ: "n",
};

/// Lower-case ASCII letters and digits joined by single hyphens; Cyrillic transliterated.
/// Undefined when nothing is left.
export function slugFromTitle(title: string): string | undefined {
  let out = "";
  let gap = false;
  for (const ch of title.toLowerCase()) {
    const piece = /^[a-z0-9]$/.test(ch) ? ch : (TRANSLIT[ch] ?? "");
    if (piece === "") {
      gap = true;
      continue;
    }
    if (gap && out) out += "-";
    gap = false;
    out += piece;
  }
  return out || undefined;
}

/// The slug the dialog shows: what the person typed, else what the title gives.
export function slugLine({ title, typed }: { title: string; typed?: string }): { slug: string | undefined; edited: boolean } {
  return typed !== undefined ? { slug: typed, edited: true } : { slug: slugFromTitle(title), edited: false };
}

/// The request body: the title and the slug; no name when none can be made (the daemon names it).
export function chainInput(f: {
  title: string; slug: string | undefined; goal: string; autostart: boolean; mode: ReviewMode | ""; version: string;
}): NewChain {
  const slug = f.slug?.trim();
  return {
    title: f.title.trim(),
    ...(slug ? { name: slug } : {}),
    goal: f.goal.trim() || null,
    autostart: f.autostart,
    review_mode: f.mode || null,
    version: f.version.trim() || null,
  };
}
