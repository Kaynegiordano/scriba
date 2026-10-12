"""Télécharge les images ARASAAC des mots courants, les réduit, et les regroupe pour Scriba.

Usage : python -I tools/fetch-pictos.py <ids.txt> <dossier-cache> <sortie.bin> [taille]
Sortie : suite de (numéro u32, longueur u32, PNG) en little-endian.
Pictogrammes : Sergio Palao, propriété du Gouvernement d'Aragon, ARASAAC (arasaac.org), CC BY-NC-SA.
"""
import io
import struct
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from PIL import Image

ids_file, cache_dir, out_file = sys.argv[1:4]
size = int(sys.argv[4]) if len(sys.argv) > 4 else 72
cache = Path(cache_dir)
cache.mkdir(parents=True, exist_ok=True)
ids = [int(x) for x in Path(ids_file).read_text().split()]


def fetch(pid: int) -> tuple[int, bytes | None]:
    raw = cache / f"{pid}_300.png"
    if not raw.exists():
        url = f"https://static.arasaac.org/pictograms/{pid}/{pid}_300.png"
        for attempt in range(3):
            try:
                req = urllib.request.Request(url, headers={"User-Agent": "Scriba (usage prive)"})
                with urllib.request.urlopen(req, timeout=30) as r:
                    raw.write_bytes(r.read())
                break
            except Exception:
                time.sleep(1 + attempt)
        else:
            return pid, None
    img = Image.open(raw).convert("RGBA")
    img.thumbnail((size, size), Image.LANCZOS)
    # palette réduite : les pictogrammes ont des aplats de couleur, l'image reste nette
    img = img.quantize(colors=48, method=Image.Quantize.FASTOCTREE)
    buf = io.BytesIO()
    img.save(buf, "PNG", optimize=True)
    return pid, buf.getvalue()


with ThreadPoolExecutor(max_workers=6) as pool:
    results = list(pool.map(fetch, ids))

ok = [(pid, data) for pid, data in results if data]
with open(out_file, "wb") as f:
    for pid, data in ok:
        f.write(struct.pack("<II", pid, len(data)))
        f.write(data)
total = sum(len(d) for _, d in ok)
print(f"{len(ok)}/{len(ids)} images, {total / 1024 / 1024:.1f} Mo -> {out_file}")
