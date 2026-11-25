"""LICENSE is the Apache License 2.0 byte for byte, NOTICE is three lines, the docs name the licence.

Run by `cargo xtask scripts` (and so by `cargo xtask verify`): `python3 -B .github/scripts/test_license.py`.
The checksum is that of the official text, https://www.apache.org/licenses/LICENSE-2.0.txt
"""

import hashlib
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OFFICIAL_SHA256 = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"
OFFICIAL_BYTES = 11358
NOTICE = b"Toknila\nCopyright 2026 Mykyta Kravtsov\nhttps://github.com/jh0per/Toknila\n"
DOCS = ["README.md", "CONTRIBUTING.md", "docs/install.md"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def current_licence_sentences(text):
    """Lines that name PolyForm without describing history (a version or a date)."""
    return [
        line
        for line in text.splitlines()
        if "PolyForm" in line and not any(w in line for w in ("0.3.", "versions", "Versions", "2026-11-25"))
    ]


class LicenseTests(unittest.TestCase):
    def test_the_licence_text_equals_the_official_one(self):
        data = (ROOT / "LICENSE").read_bytes()
        self.assertEqual(len(data), OFFICIAL_BYTES)
        self.assertEqual(sha(data), OFFICIAL_SHA256)

    def test_the_check_fails_on_one_wrong_byte(self):
        # The control: the same comparison must fail on this.
        data = (ROOT / "LICENSE").read_bytes()
        self.assertNotEqual(sha(data[:-2] + b"X" + data[-1:]), OFFICIAL_SHA256)

    def test_notice_is_exactly_three_lines(self):
        self.assertEqual((ROOT / "NOTICE").read_bytes(), NOTICE)

    def test_the_docs_name_apache_and_not_polyform_as_current(self):
        for name in DOCS:
            text = (ROOT / name).read_text(encoding="utf-8")
            self.assertIn("Apache License 2.0", text, name)
            self.assertEqual(current_licence_sentences(text), [], name)
            self.assertNotIn("source-available", text, name)

    def test_the_polyform_control(self):
        self.assertEqual(current_licence_sentences("Toknila is under the PolyForm Shield License"), ["Toknila is under the PolyForm Shield License"])

    def test_there_is_no_cla(self):
        self.assertFalse((ROOT / "CLA.md").exists())
        self.assertFalse((ROOT / ".github/workflows/cla.yml").exists())


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1], verbosity=2)
