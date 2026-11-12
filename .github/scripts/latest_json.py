"""Build the updater's latest.json from a release's signature files.

usage: latest_json.py VERSION SIG_DIR OWNER/REPO > latest.json

The build jobs only upload installers and their .sig files. Writing latest.json once, here,
after every build has finished, avoids the race where parallel jobs each rewrite the same
asset and one of them fails with "Not Found" (and its platform goes missing from the file).
The output matches what tauri-action wrote before: the same keys and the same
releases/latest/download URLs, so installed launchers read it the same way.
"""
import datetime
import json
import os
import sys

# The updater looks up '<os>-<arch>-<bundle>' first, then '<os>-<arch>'. Windows prefers the
# NSIS installer, Linux the AppImage; the .deb has its own key for apt installs.
ENTRIES = [
    (["windows-x86_64", "windows-x86_64-nsis"], "_x64-setup.exe"),
    (["windows-aarch64", "windows-aarch64-nsis"], "_arm64-setup.exe"),
    (["linux-x86_64", "linux-x86_64-appimage"], "_amd64.AppImage"),
    (["linux-x86_64-deb"], "_amd64.deb"),
    (["linux-aarch64", "linux-aarch64-appimage"], "_aarch64.AppImage"),
    (["linux-aarch64-deb"], "_arm64.deb"),
]


def main() -> int:
    version, sig_dir, repo = sys.argv[1:4]
    base = f"https://github.com/{repo}/releases/latest/download/"
    sigs = sorted(f for f in os.listdir(sig_dir) if f.endswith(".sig"))
    platforms = {}
    for keys, suffix in ENTRIES:
        found = [f for f in sigs if f.endswith(f"_{version}{suffix}.sig")]
        if len(found) != 1:
            print(f"::error::expected one signature for *_{version}{suffix}, found {found}", file=sys.stderr)
            return 1
        with open(os.path.join(sig_dir, found[0]), encoding="utf-8") as fh:
            signature = fh.read()
        entry = {"signature": signature, "url": base + found[0][: -len(".sig")]}
        for key in keys:
            platforms[key] = entry
    now = datetime.datetime.now(datetime.timezone.utc)
    doc = {
        "version": version,
        "notes": "",
        "pub_date": now.isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "platforms": platforms,
    }
    json.dump(doc, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
