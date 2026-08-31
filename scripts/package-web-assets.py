#!/usr/bin/env python3
"""Create a byte-for-byte deterministic Hanni web OTA tar.gz bundle."""

from __future__ import annotations

import gzip
import pathlib
import sys
import tarfile


EXCLUDED_TOP_LEVEL = {"assets", "sounds"}


def fail(message: str) -> None:
    raise SystemExit(message)


def main() -> None:
    if len(sys.argv) != 3:
        fail("usage: package-web-assets.py <source-dir> <bundle.tar.gz>")

    source = pathlib.Path(sys.argv[1]).resolve(strict=True)
    output = pathlib.Path(sys.argv[2]).resolve()
    if not source.is_dir():
        fail("source must be a directory")
    if output == source or source in output.parents:
        fail("output must be outside the source directory")

    entries: list[tuple[str, pathlib.Path]] = []
    for path in source.rglob("*"):
        relative = path.relative_to(source)
        if relative.parts[0] in EXCLUDED_TOP_LEVEL:
            continue
        if path.is_symlink():
            fail(f"symbolic links are not allowed in web OTA bundles: {relative}")
        if not path.is_dir() and not path.is_file():
            fail(f"unsupported web OTA entry: {relative}")
        entries.append((relative.as_posix(), path))

    if not any(name == "index.html" for name, _ in entries):
        fail("web OTA source does not contain index.html")

    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for name, path in sorted(entries):
                    info = tarfile.TarInfo(name + ("/" if path.is_dir() else ""))
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = 0
                    if path.is_dir():
                        info.type = tarfile.DIRTYPE
                        info.mode = 0o755
                        archive.addfile(info)
                    else:
                        info.type = tarfile.REGTYPE
                        info.mode = 0o644
                        info.size = path.stat().st_size
                        with path.open("rb") as content:
                            archive.addfile(info, content)

    if output.stat().st_size == 0:
        fail("deterministic web OTA bundle is empty")


if __name__ == "__main__":
    main()
