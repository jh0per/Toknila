"""LICENSE is the PolyForm Shield License 1.0.0, byte for byte, under one Required Notice line.

Run by `cargo xtask scripts` (and so by `cargo xtask verify`): `python3 -B .github/scripts/test_license.py`.
The checksum is that of the official text,
https://github.com/polyformproject/polyform-licenses/blob/1.0.0/PolyForm-Shield-1.0.0.md
"""

import hashlib
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOTICE = "Required Notice: Copyright 2026 Mykyta Kravtsov (https://github.com/jh0per/Toknila)\n\n"
OFFICIAL_SHA256 = "56328093d57c87dcf2811ddcc824caf2723a07ffc332e6fbe4f9f108a2893a91"
OFFICIAL_BYTES = 5748


def licence_text(data):
    """The bytes after the notice line and the blank line under it, or None without the notice."""
    notice = NOTICE.encode("utf-8")
    return data[len(notice):] if data.startswith(notice) else None


class LicenseTests(unittest.TestCase):
    def test_the_licence_text_equals_the_official_one(self):
        data = (ROOT / "LICENSE").read_bytes().replace(b"\r\n", b"\n")
        text = licence_text(data)
        self.assertIsNotNone(text, "LICENSE does not start with the Required Notice line")
        self.assertEqual(len(text), OFFICIAL_BYTES)
        self.assertEqual(hashlib.sha256(text).hexdigest(), OFFICIAL_SHA256)

    def test_the_check_fails_on_a_changed_character_and_on_a_missing_notice(self):
        # The control: the same comparison must fail on these.
        data = (ROOT / "LICENSE").read_bytes().replace(b"\r\n", b"\n")
        changed = data.replace(b"Shield", b"Sheild", 1)
        self.assertNotEqual(hashlib.sha256(licence_text(changed)).hexdigest(), OFFICIAL_SHA256)
        self.assertIsNone(licence_text(data[len(NOTICE):]))


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1], verbosity=2)
