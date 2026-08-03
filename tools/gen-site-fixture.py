#!/usr/bin/env python3
"""
Genera un sitio HTML estático sintético para medir el modo `filesystem`.

Para qué sirve: los dos umbrales del modo `filesystem` —elementos/s y
páginas/s— se miden sobre dos regímenes muy distintos, y la diferencia entre ellos es de
siete veces. Sin un generador versionado, el número que respalda el rendimiento no
se puede volver a comprobar. Con él, la medición es un comando.

La salida imita un `dist/` de Astro: cada página es `ruta/index.html`, que el sitio
publica como `/ruta/`. Eso ejercita a propósito el recorte del segmento `index.html`,
donde ya hubo un fallo (`noindex.html` se convertía en `/no`).

Es determinista: mismos argumentos, mismos bytes. No usa `random`, para que dos
ejecuciones sean comparables sin fijar semilla.

Uso:
  tools/gen-site-fixture.py --pages 50000 --links 123 --out /tmp/fixture-denso
  tools/gen-site-fixture.py --pages 3001  --links 14  --out /tmp/fixture-ligero

Los dos regímenes que se miden:
  denso   50.000 páginas × 123 enlaces = 6.150.000 enlaces  (~400 MB)
  ligero   3.001 páginas ×  14 enlaces =    42.014 enlaces
"""

import argparse
import base64
import shutil
import sys
from pathlib import Path

# Paso del generador de destinos. Coprimo con cualquier número de páginas razonable, y
# lo bastante pequeño para que `links * STEP` no dé la vuelta al sitio: así los destinos
# de una página son todos distintos y el recuento de enlaces es exacto.
STEP = 7

# Cuántos ficheros de imagen distintos se generan. Las páginas los reutilizan en rotación.
N_IMAGES = 1000

# JPEG válido de 1x1 píxel. Las imágenes tienen que existir: si no, cada `<img>` da un 404
# y el fixture se llena de hallazgos que ha inventado él mismo, que es justo lo que un
# banco de pruebas no debe hacer.
JPEG_1X1 = base64.b64decode(
    "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRof"
    "Hh0aHBwcJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPDs0NDP/wAALCAABAAEBAREA/8QAFAAB"
    "AQAAAAAAAAAAAAAAAAAAAAv/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAA/AKgA/9k="
)

PAGE = """<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8">
<title>{title}</title>
<meta name="description" content="{desc}">
<link rel="canonical" href="{base}{route}">
<meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
<h1>{h1}</h1>
<p>Contenido de relleno de la p&aacute;gina {n}. Sirve para que el parseo tenga texto
que recorrer y para que la p&aacute;gina no sea s&oacute;lo enlaces.</p>
<img src="/img/{img_a}.jpg" alt="Ilustraci&oacute;n {n}">
<img src="/img/{img_b}.jpg" alt="Segunda ilustraci&oacute;n {n}">
<nav>
{links}
</nav>
</body>
</html>
"""


def route_for(n: int) -> str:
    """La ruta publicada de la página n. La 0 es la portada."""
    return "/" if n == 0 else f"/p/{n:06d}/"


def targets(n: int, links: int, pages: int):
    """Los destinos de la página n: `links` páginas distintas, sin auto-enlace."""
    return [(n + 1 + j * STEP) % pages for j in range(links)]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pages", type=int, default=50000, help="número de páginas HTML")
    ap.add_argument("--links", type=int, default=123, help="enlaces internos por página")
    ap.add_argument("--out", type=Path, required=True, help="directorio de salida (se borra si existe)")
    ap.add_argument("--base", default="https://fixture.local", help="URL base para el canonical")
    args = ap.parse_args()

    if args.links * STEP >= args.pages:
        print(
            f"Con {args.pages} páginas no caben {args.links} enlaces distintos por página "
            f"(hacen falta más de {args.links * STEP}).",
            file=sys.stderr,
        )
        return 1

    out: Path = args.out
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    base = args.base.rstrip("/")

    img_dir = out / "img"
    img_dir.mkdir()
    n_images = min(N_IMAGES, args.pages)
    for i in range(n_images):
        (img_dir / f"{i}.jpg").write_bytes(JPEG_1X1)

    for n in range(args.pages):
        route = route_for(n)
        dest = out / "index.html" if n == 0 else out / "p" / f"{n:06d}" / "index.html"
        dest.parent.mkdir(parents=True, exist_ok=True)

        links = "\n".join(
            f'<a href="{route_for(t)}">P&aacute;gina {t}</a>' for t in targets(n, args.links, args.pages)
        )
        dest.write_text(
            PAGE.format(
                title=f"Página {n} del fixture" if n else "Portada del fixture",
                desc=f"Descripción única de la página {n}, con acentos para el FTS.",
                h1=f"Página {n}" if n else "Portada",
                base=base,
                route=route,
                n=n,
                img_a=n % n_images,
                img_b=(n + n_images // 2) % n_images,
                links=links,
            ),
            encoding="utf-8",
        )

    total_links = args.pages * args.links
    print(
        f"{args.pages} páginas, {total_links} enlaces, {args.pages * 2} referencias a "
        f"{n_images} imágenes en {out}"
    )
    print(f"Medir con: cargo build --profile bench-max -p crawlforge-cli && \\")
    print(f"  ./target/bench-max/crawlforge audit {out} --base {base}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
