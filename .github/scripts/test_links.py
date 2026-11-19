"""Every relative link and image of README.md and docs/install.md must resolve to a file.

Run by `cargo xtask scripts` (and so by `cargo xtask verify`): `python3 -B .github/scripts/test_links.py`.
The pictures of the README are held to the story's limits too: at most four, each under 400 KB.
"""

import re
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKED = ["README.md", "docs/install.md", "CONTRIBUTING.md", "CLA.md"]

# Files a later story adds. A link to one of these passes until the file exists; once it exists the
# entry is only a note and the link is checked like any other. None is expected now.
EXPECTED_LATER = set()

PICTURES = ROOT / "docs" / "screenshots" / "readme"
MAX_PICTURES = 4
MAX_PICTURE_BYTES = 400 * 1024

FENCE = re.compile(r"^(```|~~~).*?^\1[ \t]*$", re.S | re.M)
INLINE_CODE = re.compile(r"`[^`\n]*`")
MD_LINK = re.compile(r"!?\[[^\]\n]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
HTML_REF = re.compile(r"""(?:src|href)\s*=\s*["']([^"']+)["']""", re.I)


def targets(text):
    """The link and image targets of a Markdown text, code left out."""
    text = FENCE.sub("", text)
    text = INLINE_CODE.sub("", text)
    return MD_LINK.findall(text) + HTML_REF.findall(text)


def is_relative(target):
    return not (re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target) or target.startswith(("#", "//")))


def dead_links(path, root=ROOT, expected_later=EXPECTED_LATER):
    """The relative targets in `path` that name no file under `root`, as `target`."""
    dead = []
    for target in targets(path.read_text(encoding="utf-8")):
        if not is_relative(target):
            continue
        file = target.split("#", 1)[0].split("?", 1)[0]
        if not file:
            continue
        found = (path.parent / file).resolve()
        if found.exists():
            continue
        if file in expected_later and not (root / file).exists():
            continue
        dead.append(target)
    return dead


class LinkTests(unittest.TestCase):
    def test_every_relative_link_and_image_resolves(self):
        for name in CHECKED:
            with self.subTest(file=name):
                self.assertEqual(dead_links(ROOT / name), [], f"dead relative links in {name}")

    def test_the_checker_finds_a_planted_dead_link_and_a_planted_missing_image(self):
        # The control: the same function that passes the real files must fail on these.
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "there.md").write_text("here\n", encoding="utf-8")
            page = tmp / "page.md"
            page.write_text(
                "[good](there.md) [good with anchor](there.md#top) [web](https://example.com/x) [here](#a)\n"
                "[bad](nowhere.md) ![pic](img/missing.png)\n"
                "```\n[in a fence](ignored.md)\n```\n"
                "and `[inline](ignored2.md)` code\n",
                encoding="utf-8",
            )
            self.assertEqual(dead_links(page, root=tmp, expected_later=set()), ["nowhere.md", "img/missing.png"])

    def test_a_link_to_a_file_a_later_story_adds_passes_only_until_it_is_expected_no_more(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            page = tmp / "page.md"
            page.write_text("[later](LATER.md)\n", encoding="utf-8")
            self.assertEqual(dead_links(page, root=tmp, expected_later={"LATER.md"}), [])
            self.assertEqual(dead_links(page, root=tmp, expected_later=set()), ["LATER.md"])
            (tmp / "LATER.md").write_text("x\n", encoding="utf-8")
            self.assertEqual(dead_links(page, root=tmp, expected_later=set()), [])

    def test_the_readme_pictures_stay_within_the_limits(self):
        pictures = sorted(PICTURES.glob("*.png"))
        self.assertTrue(pictures, "no picture under docs/screenshots/readme")
        self.assertLessEqual(len(pictures), MAX_PICTURES)
        for p in pictures:
            self.assertLess(p.stat().st_size, MAX_PICTURE_BYTES, f"{p.name} is too large")

    def test_the_readme_shows_every_picture_with_alt_text(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        shown = re.findall(r"!\[([^\]]+)\]\(docs/screenshots/readme/([^)]+)\)", readme)
        self.assertEqual(sorted(n for _, n in shown), sorted(p.name for p in PICTURES.glob("*.png")))
        for alt, name in shown:
            self.assertGreater(len(alt.split()), 3, f"alt text of {name} says too little")


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1], verbosity=2)
