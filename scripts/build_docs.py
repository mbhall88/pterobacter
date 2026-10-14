#!/usr/bin/env python3
"""Stage the linked user guide and build it with mdBook 0.5.4."""
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
LINK = re.compile(r"(!?\[[^\]\n]*\]\()([^\s)]+)(\))")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mdbook", default="mdbook")
    parser.add_argument("--output", type=Path, default=ROOT / "target/docs")
    args = parser.parse_args()
    version = subprocess.check_output([args.mdbook, "--version"], text=True).strip()
    if version != "mdbook v0.5.4":
        raise SystemExit(f"Expected mdbook v0.5.4; got {version}")
    summary = (DOCS / "SUMMARY.md").read_text()
    chapters = [m[1] for m in LINK.findall(summary)]
    queued = set(chapters)
    assets = set()
    texts = {}
    # Follow local guide links. Files outside docs stay repository source links.
    for name in chapters:
        path = DOCS / name
        text = path.read_text()
        def rewrite(match):
            target = match[2]
            if ":" in target or target.startswith("#"):
                return match[0]
            target_path, marker, fragment = target.partition("#")
            dest = (path.parent / unquote(target_path)).resolve()
            if not dest.is_file():
                raise SystemExit(f"Missing link: {name}: {target}")
            if not dest.is_relative_to(DOCS):
                relative = dest.relative_to(ROOT).as_posix()
                url = "https://github.com/mbhall88/terobacter/blob/main/" + relative
                return match[1] + url + (marker + fragment if marker else "") + match[3]
            relative = dest.relative_to(DOCS).as_posix()
            if dest.suffix == ".md":
                if relative not in queued:
                    queued.add(relative)
                    chapters.append(relative)
            else:
                assets.add(relative)
            # mdBook renders chapter README.md as index.html.
            if dest.name == "README.md":
                return match[1] + target_path[:-9] + "index.html" + (marker + fragment if marker else "") + match[3]
            return match[0]
        texts[name] = LINK.sub(rewrite, text)
    listed = {m[1] for m in LINK.findall(summary)}
    extra = [name for name in chapters if name not in listed]
    if extra:
        summary += "\n# Further reference\n\n"
        for name in extra:
            heading = next((line.lstrip("# ") for line in texts[name].splitlines() if line.startswith("# ")), name)
            summary += f"- [{heading}]({name})\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="docs-", dir=args.output.parent) as tmp:
        stage = Path(tmp)
        shutil.copyfile(ROOT / "book.toml", stage / "book.toml")
        shutil.copytree(DOCS / "theme", stage / "theme")
        for name, text in texts.items():
            path = stage / "src" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        for name in assets:
            path = stage / "src" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(DOCS / name, path)
        (stage / "src/SUMMARY.md").write_text(summary)
        subprocess.run([args.mdbook, "build", str(stage), "--dest-dir", str(args.output.resolve())], check=True)
    print(f"Built {len(chapters)} pages and {len(assets)} linked assets in {args.output}")


if __name__ == "__main__":
    main()
