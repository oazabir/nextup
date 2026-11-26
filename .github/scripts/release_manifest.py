"""Stage release assets and assemble the Tauri updater manifest."""
import json
import shutil
import sys
from pathlib import Path
from urllib.parse import quote


PLATFORMS = {
    "windows-x64": (("windows-x86_64",), ".exe", ".exe"),
    "macos-universal": (("darwin-aarch64", "darwin-x86_64"), ".app.tar.gz", ".dmg"),
    "linux-x64": (("linux-x86_64",), ".AppImage", ".AppImage"),
}


def required_asset(directory, suffix):
    matches = [file for file in directory.rglob(f"*{suffix}") if file.is_file()]
    if len(matches) != 1:
        raise SystemExit(f"Expected one {suffix} asset in {directory}, found {len(matches)}")
    return matches[0]


def signature_for(package):
    signature = package.with_name(package.name + ".sig")
    if not signature.is_file() or not signature.read_text().strip():
        raise SystemExit(f"Missing signature for {package}")
    return signature


def main():
    source, destination = map(Path, sys.argv[1:3])
    repository, tag = sys.argv[3:]
    downloads = destination / "downloads"
    updater = destination / "updater"
    downloads.mkdir(parents=True)
    updater.mkdir()
    platforms = {}
    for platform, (targets, update_suffix, installer_suffix) in PLATFORMS.items():
        directory = source / f"next-up-{platform}"
        basename = f"Next-Up-{tag[1:]}-{platform}"
        installer = required_asset(directory, installer_suffix)
        installer_name = basename + installer_suffix
        shutil.copyfile(installer, downloads / installer_name)
        shutil.copyfile(signature_for(installer), downloads / (installer_name + ".sig"))

        package = required_asset(directory, update_suffix)
        name = basename + update_suffix
        update_tag = tag
        if update_suffix != installer_suffix:
            shutil.copyfile(package, updater / name)
            update_tag = f"updater-{tag}"
        entry = {
            "signature": signature_for(package).read_text().strip(),
            "url": f"https://github.com/{repository}/releases/download/{quote(update_tag)}/{quote(name)}",
        }
        for target in targets:
            platforms[target] = entry

    (downloads / "latest.json").write_text(
        json.dumps({"version": tag[1:], "platforms": platforms}, indent=2) + "\n"
    )



if __name__ == "__main__":
    main()
