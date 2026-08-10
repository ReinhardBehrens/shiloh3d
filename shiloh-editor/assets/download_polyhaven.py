#!/usr/bin/env python3
"""Download Poly Haven CC0 assets for Shiloh Studio (Powered by Poly Haven).

Usage:
  python3 shiloh-editor/assets/download_polyhaven.py

Assets land under:
  shiloh-editor/assets/props/     — 1k glTF models (skips .bin > MAX_BIN_MB)
  shiloh-editor/assets/textures/  — 1k PBR maps
  shiloh-editor/assets/hdris/     — tonemapped JPG / small HDR

License: assets are CC0. Live API requires a clear “Powered by Poly Haven” credit
(https://polyhaven.com/our-api).
"""

from __future__ import annotations

import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "props"
TEX = ROOT / "textures"
HDRI = ROOT / "hdris"
UA = {"User-Agent": "Shiloh3D/0.1 (CC0 assets; Powered by Poly Haven)"}
MAX_BIN_MB = 60

MODELS_1K = [
    "shrub_03",
    "fern_02",
    "rock_09",
    "rock_06",
    "pine_sapling_small",
    "grass_medium_01",
    "dead_tree_trunk",
    "rock_moss_set_01",
    "tree_stump_01",
]

TEXTURES_1K = [
    "forest_leaves_04",
    "forest_ground_04",
    "aerial_grass_rock",
]

HDRIS = [
    "fouriesburg_mountain_midday",
    "forest_slope",
    "drakensberg_solitary_mountain_puresky",
]


def get_json(url: str):
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def fetch(url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.stat().st_size > 0:
        print(f"  skip {path.relative_to(ROOT)}")
        return
    print(f"  get  {path.relative_to(ROOT)}")
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=300) as r, open(path, "wb") as f:
        while True:
            chunk = r.read(256 * 1024)
            if not chunk:
                break
            f.write(chunk)


def download_model(aid: str, res: str = "1k") -> None:
    print(f"=== model {aid} {res} ===")
    files = get_json(f"https://api.polyhaven.com/files/{aid}")
    entry = files.get("gltf", {}).get(res, {}).get("gltf")
    if not entry:
        print(f"  missing gltf for {aid}", file=sys.stderr)
        return
    for rel, info in (entry.get("include") or {}).items():
        if rel.endswith(".bin") and info.get("size", 0) > MAX_BIN_MB * 1024 * 1024:
            print(
                f"  SKIP {aid}: .bin is {info['size']/1e6:.0f} MB (cap {MAX_BIN_MB} MB)"
            )
            return
    dest = OUT / aid
    fetch(entry["url"], dest / Path(entry["url"]).name)
    for rel, info in (entry.get("include") or {}).items():
        fetch(info["url"], dest / rel)


def download_texture(aid: str, res: str = "1k") -> None:
    print(f"=== texture {aid} {res} ===")
    files = get_json(f"https://api.polyhaven.com/files/{aid}")
    dest = TEX / aid
    for map_name in ("Diffuse", "nor_gl", "arm"):
        res_block = files.get(map_name, {}).get(res, {})
        for fmt in ("jpg", "png"):
            info = res_block.get(fmt)
            if info and "url" in info:
                fetch(info["url"], dest / Path(info["url"]).name)
                break


def download_hdri(aid: str) -> None:
    print(f"=== hdri {aid} ===")
    files = get_json(f"https://api.polyhaven.com/files/{aid}")
    url = None
    tone = files.get("tonemapped")
    if isinstance(tone, dict) and "url" in tone:
        url = tone["url"]
    hdri = files.get("hdri") or {}
    if url is None:
        for res in ("1k", "2k"):
            block = hdri.get(res, {})
            for fmt in ("hdr", "exr"):
                if fmt in block and "url" in block[fmt]:
                    url = block[fmt]["url"]
                    break
            if url:
                break
    if not url:
        print(f"  WARN: no file for {aid}", file=sys.stderr)
        return
    fetch(url, HDRI / aid / Path(url.split("?")[0]).name)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    TEX.mkdir(parents=True, exist_ok=True)
    HDRI.mkdir(parents=True, exist_ok=True)
    for m in MODELS_1K:
        download_model(m)
    for t in TEXTURES_1K:
        download_texture(t)
    for h in HDRIS:
        download_hdri(h)
    print("ALL DONE — Powered by Poly Haven (CC0)")


if __name__ == "__main__":
    main()
