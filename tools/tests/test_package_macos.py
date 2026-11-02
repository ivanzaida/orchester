"""tools/package-macos.sh with stand-ins for cargo and Apple's tools: what it signs, in which order, and when it stops.

python3 -B -m unittest discover -s tools/tests -p test_package_macos.py
"""

import os
import platform
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "tools" / "package-macos.sh"
REPO_TMP = REPO / ".tmp"
IDENTITY = "Developer ID Application: Test (TEAM123456)"
NOTARY = ["--notary-key", "KEY", "--notary-key-id", "key-id-value", "--notary-issuer", "issuer-value"]

# Each stand-in writes its name and arguments to $CALLS, one call per line.
RECORD = 'echo "$(basename "$0") $*" >> "$CALLS"\n'
STAND_INS = {
    "cargo": "",
    "codesign": "",
    "spctl": 'echo "accepted\nsource=Notarized Developer ID"\n',
    "security": f'echo \'  1) ABC "{IDENTITY}"\'\n',
    "hdiutil": 'for last; do :; done; : > "$last"\n',
    "xcrun": """case "$1 $2" in
  "notarytool submit") echo "{\\"id\\":\\"sub-1\\",\\"status\\":\\"$NOTARY_VERDICT\\"}"; [ "$NOTARY_VERDICT" = Accepted ] ;;
  "notarytool log") echo "the submission's log" ;;
esac
""",
}


@unittest.skipUnless(platform.system() == "Darwin", "the script runs on a Mac")
class PackageMacos(unittest.TestCase):
    def setUp(self):
        REPO_TMP.mkdir(parents=True, exist_ok=True)
        folder = tempfile.TemporaryDirectory(dir=REPO_TMP)
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)
        self.calls = self.root / "calls.txt"
        self.out = self.root / "out"
        tools = self.root / "tools"
        tools.mkdir()
        for name, body in STAND_INS.items():
            path = tools / name
            path.write_text("#!/bin/bash\n" + RECORD + body)
            path.chmod(path.stat().st_mode | stat.S_IEXEC)
        self.key = self.root / "key.p8"
        self.key.write_text("not a key\n")
        release = self.root / "build" / "release"
        release.mkdir(parents=True)
        for program in ("orchester", "orchester-runner"):
            (release / program).write_text("a program\n")
        self.env = {
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "HOME": os.environ.get("HOME", ""),
            "CALLS": str(self.calls),
            "NOTARY_VERDICT": "Accepted",
        }

    def run_script(self, *arguments, env=None):
        command = [str(SCRIPT), "--build-dir", str(self.root / "build"), "--out-dir", str(self.out), *arguments]
        command = [str(self.key) if word == "KEY" else word for word in command]
        return subprocess.run(command, env={**self.env, **(env or {})}, capture_output=True, text=True, check=False)

    def recorded(self, tool):
        if not self.calls.exists():
            return []
        return [line for line in self.calls.read_text().splitlines() if line.startswith(f"{tool} ")]

    def test_without_an_identity_it_signs_ad_hoc(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        signs = [call for call in self.recorded("codesign") if "--sign" in call]
        self.assertEqual(len(signs), 2)
        for call in signs:
            self.assertIn("--sign - --timestamp=none", call)
            self.assertNotIn("--options runtime", call)
        self.assertTrue(signs[0].endswith("orchester-runner"))
        self.assertEqual(self.recorded("xcrun"), [])
        self.assertTrue((self.out / "Orchester.app" / "Contents" / "MacOS" / "orchester-runner").exists())

    def test_an_identity_signs_the_runner_first_with_the_hardened_runtime_and_a_timestamp(self):
        for how in ([["--identity", IDENTITY], None], [[], {"ORCHESTER_MACOS_IDENTITY": IDENTITY}]):
            self.calls.unlink(missing_ok=True)
            result = self.run_script(*how[0], env=how[1])
            self.assertEqual(result.returncode, 0, result.stderr)
            signs = [call for call in self.recorded("codesign") if "--sign" in call]
            self.assertEqual(len(signs), 2)
            for call in signs:
                self.assertIn(f"--force --options runtime --timestamp --sign {IDENTITY}", call)
                self.assertNotIn("--entitlements", call)
            self.assertTrue(signs[0].endswith("orchester-runner"))
            self.assertTrue(signs[1].endswith(".partial"))
            self.assertEqual(self.recorded("xcrun"), [], "signing alone does not notarize")

    def test_an_unknown_identity_stops_before_the_build(self):
        result = self.run_script("--identity", "Developer ID Application: Nobody (0000000000)")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no valid signing identity", result.stderr)
        self.assertEqual(self.recorded("cargo"), [])

    def test_what_does_not_go_together_stops_before_the_build(self):
        for arguments, said in (
            (["--identity", IDENTITY, "--notary-key", "KEY"], "needs all of"),
            (NOTARY, "needs a Developer ID identity"),
            (["--identity", IDENTITY, "--no-sign"], "exclude each other"),
            (["--no-sign", "--archives"], "does not go with --no-sign"),
            (["--universal", "--archives"], "does not go with --universal"),
        ):
            result = self.run_script(*arguments)
            self.assertNotEqual(result.returncode, 0, arguments)
            self.assertIn(said, result.stderr)
            self.assertEqual(self.recorded("cargo"), [], arguments)

    def test_notarizing_staples_and_packs_from_the_stapled_app(self):
        result = self.run_script("--identity", IDENTITY, *NOTARY, "--archives")
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls.read_text().splitlines()
        steps = [line.split(" ")[0:3] for line in calls if line.startswith("xcrun")]
        self.assertEqual(
            [" ".join(step[1:3]) for step in steps],
            ["notarytool submit", "stapler staple", "stapler validate"] * 2,
            "the bundle, then the disk image",
        )
        submissions = [line for line in calls if "notarytool submit" in line]
        self.assertIn(".notary.zip", submissions[0])
        self.assertIn("--wait --timeout", submissions[0])
        self.assertIn("macos-aarch64.dmg", submissions[1])
        staple_app = next(i for i, line in enumerate(calls) if "stapler staple" in line)
        image = next(i for i, line in enumerate(calls) if line.startswith("hdiutil"))
        self.assertLess(staple_app, image, "the disk image holds the stapled app")
        version = next(self.out.glob("*.zip")).name.split("-")[1]
        names = sorted(path.name for path in self.out.iterdir() if not path.name.startswith("."))
        self.assertEqual(
            names,
            ["Orchester-%s-macos-aarch64.dmg" % version, "Orchester-%s-macos-aarch64.zip" % version, "Orchester.app"],
        )
        self.assertEqual([path.name for path in self.out.iterdir() if path.name.endswith(".notary.zip")], [])
        zipped = subprocess.run(
            ["unzip", "-Z1", str(next(self.out.glob("*.zip")))], capture_output=True, text=True, check=True
        ).stdout
        self.assertIn("Orchester.app/Contents/MacOS/orchester-runner", zipped)

    def test_a_rejection_prints_the_log_and_stops(self):
        result = self.run_script("--identity", IDENTITY, *NOTARY, "--archives", env={"NOTARY_VERDICT": "Invalid"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("the submission's log", result.stderr)
        self.assertIn('"Invalid"', result.stderr)
        self.assertEqual(len([call for call in self.recorded("xcrun") if "notarytool log sub-1" in call]), 1)
        self.assertEqual([call for call in self.recorded("xcrun") if "stapler" in call], [])
        self.assertEqual(list(self.out.glob("Orchester-*")), [], "nothing is packed from a rejected app")

    def test_ad_hoc_archives_are_packed_without_apple(self):
        result = self.run_script("--archives")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.recorded("xcrun"), [])
        self.assertTrue(list(self.out.glob("*.dmg")) and list(self.out.glob("*.zip")))


if __name__ == "__main__":
    unittest.main()
