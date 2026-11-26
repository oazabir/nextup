"""Stage release assets and assemble the Tauri updater manifest."""
import json
import shutil
import sys
from pathlib import Path
from urllib.parse import quote


PLATFORMS = {
    "windows-x64": ("windows-x86_64", ".exe", ()),
    "macos-apple-silicon": ("darwin-aarch64", ".app.tar.gz", (".dmg",)),
    "macos-intel": ("darwin-x86_64", ".app.tar.gz", (".dmg",)),
    "linux-x64": ("linux-x86_64", ".AppImage", (".deb",)),
}


def required_asset(directory, suffix):
    matches = [file for file in directory.rglob(f"*{suffix}") if file.is_file()]
    if len(matches) != 1:
        raise SystemExit(f"Expected one {suffix} asset in {directory}, found {len(matches)}")
    return matches[0]


def main():
    source, destination = map(Path, sys.argv[1:3])
    repository, tag = sys.argv[3:]
    destination.mkdir()
    platforms = {}
    for platform, (target, suffix, installers) in PLATFORMS.items():
        directory = source / f"next-up-{platform}"
        package = required_asset(directory, suffix)
        signature = package.with_name(package.name + ".sig")
        if not signature.is_file() or not (signed := signature.read_text().strip()):
            raise SystemExit(f"Missing signature for {package}")
        name = f"Next-Up-{tag[1:]}-{platform}{suffix}"
        shutil.copyfile(package, destination / name)
        shutil.copyfile(signature, destination / (name + ".sig"))
        for installer in installers:
            shutil.copyfile(
                required_asset(directory, installer),
                destination / f"Next-Up-{tag[1:]}-{platform}{installer}",
            )
        platforms[target] = {
            "signature": signed,
            "url": f"https://github.com/{repository}/releases/download/{quote(tag)}/{quote(name)}",
        }
    (destination / "latest.json").write_text(
        json.dumps({"version": tag[1:], "platforms": platforms}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
