#!/usr/bin/env python3
"""
Genera la escena del benchmark B1 (cornell-glass).

La rotación de las cajas se aplica AQUÍ, al construir el JSON, no en el
renderer. Una caja son 6 quads definidos por un origen `q` y dos vectores de
arista `u` y `v`; nada obliga a que estén alineados a ejes. Así B1 tiene el
aspecto del Cornell canónico sin necesitar el wrapper `Transform` (F0.13), que
no existe en los commits viejos de la barrida histórica.

Uso:
    python3 scripts/gen_cornell.py
"""

import json
import math
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "scenes" / "bench" / "cornell"

# Rotación de cada caja, en grados sobre el eje Y. El Cornell canónico usa
# -18° en la caja baja y 15° en la alta; dejamos la alta sin rotar para que la
# escena conserve una referencia alineada a ejes contra la cual leer la
# geometría rotada.
SHORT_BOX_ANGLE = -18.0
TALL_BOX_ANGLE = 15.0


def rotate_y(v, degrees):
    """Rota (x, y, z) sobre el eje Y. Misma convención que RTOW."""
    rad = math.radians(degrees)
    sin_t, cos_t = math.sin(rad), math.cos(rad)
    x, y, z = v
    return (cos_t * x + sin_t * z, y, -sin_t * x + cos_t * z)


def quad(q, u, v, material):
    return {
        "type": "quad",
        "q": [round(float(c), 4) for c in q],
        "u": [round(float(c), 4) for c in u],
        "v": [round(float(c), 4) for c in v],
        "material": material,
    }


def box(size, material, angle=0.0, offset=(0.0, 0.0, 0.0)):
    """
    Caja de `size` construida en el origen, rotada `angle` grados sobre Y y
    luego trasladada a `offset` — el mismo orden que aplica RTOW.

    `q` es un punto (se rota y se traslada); `u` y `v` son vectores de arista
    (se rotan pero NO se trasladan).
    """
    dx, dy, dz = size
    faces = [
        ((0, 0, dz), (dx, 0, 0), (0, dy, 0)),    # frente
        ((dx, 0, dz), (0, 0, -dz), (0, dy, 0)),  # derecha
        ((dx, 0, 0), (-dx, 0, 0), (0, dy, 0)),   # atrás
        ((0, 0, 0), (0, 0, dz), (0, dy, 0)),     # izquierda
        ((0, dy, dz), (dx, 0, 0), (0, 0, -dz)),  # techo
        ((0, 0, 0), (dx, 0, 0), (0, 0, dz)),     # piso
    ]

    out = []
    for q, u, v in faces:
        rq = rotate_y(q, angle)
        out.append(quad(
            (rq[0] + offset[0], rq[1] + offset[1], rq[2] + offset[2]),
            rotate_y(u, angle),
            rotate_y(v, angle),
            material,
        ))
    return out


def footprint(size, angle, offset):
    """Extensión en XZ tras rotar y trasladar, para verificar solapamientos."""
    dx, _, dz = size
    corners = [(0, 0, 0), (dx, 0, 0), (0, 0, dz), (dx, 0, dz)]
    pts = [rotate_y(c, angle) for c in corners]
    xs = [p[0] + offset[0] for p in pts]
    zs = [p[2] + offset[2] for p in pts]
    return (min(xs), max(xs)), (min(zs), max(zs))


def main():
    objects = [
        # Paredes de la caja (555³, frente abierto hacia la cámara en z<0).
        quad((555, 0, 0), (0, 555, 0), (0, 0, 555), "green"),
        quad((0, 0, 0), (0, 555, 0), (0, 0, 555), "red"),
        quad((0, 0, 0), (555, 0, 0), (0, 0, 555), "white"),
        quad((555, 555, 555), (-555, 0, 0), (0, 0, -555), "white"),
        quad((0, 0, 555), (555, 0, 0), (0, 555, 0), "white"),
        # Luz en el techo: 130x105 sobre 555² — pequeña a propósito.
        quad((343, 554, 332), (-130, 0, 0), (0, 0, -105), "light"),
    ]

    short = ((165, 165, 165), SHORT_BOX_ANGLE, (130.0, 0.0, 65.0))
    tall = ((165, 330, 165), TALL_BOX_ANGLE, (265.0, 0.0, 295.0))

    objects += box(short[0], "white", short[1], short[2])
    objects += box(tall[0], "white", tall[1], tall[2])

    sphere_center = (400.0, 90.0, 190.0)
    sphere_radius = 90.0
    objects.append({
        "type": "sphere",
        "center": list(sphere_center),
        "radius": sphere_radius,
        "material": "glass",
    })

    scene = {
        "background": {"type": "solid", "color": [0.0, 0.0, 0.0]},
        "materials": {
            "red": {"type": "lambertian", "albedo": [0.65, 0.05, 0.05]},
            "white": {"type": "lambertian", "albedo": [0.73, 0.73, 0.73]},
            "green": {"type": "lambertian", "albedo": [0.12, 0.45, 0.15]},
            "light": {"type": "diffuse_light", "emit": [15.0, 15.0, 15.0]},
            "glass": {"type": "dielectric", "refraction_index": 1.5},
        },
        "objects": objects,
    }

    (OUT / "scene.json").write_text(json.dumps(scene) + "\n")

    # Verificación de solapamientos: la escena solo es válida si nada se
    # interpenetra, y con las cajas rotadas eso deja de ser evidente a ojo.
    print(f"{len(objects)} objetos → {OUT / 'scene.json'}")
    for name, (size, angle, offset) in (("caja baja", short), ("caja alta", tall)):
        (x0, x1), (z0, z1) = footprint(size, angle, offset)
        print(f"  {name:<10} {angle:>6.1f}°  x[{x0:7.2f}, {x1:7.2f}]  z[{z0:7.2f}, {z1:7.2f}]")
    cx, _, cz = sphere_center
    print(f"  {'esfera':<10} {'':>7}  x[{cx - sphere_radius:7.2f}, {cx + sphere_radius:7.2f}]"
          f"  z[{cz - sphere_radius:7.2f}, {cz + sphere_radius:7.2f}]")


if __name__ == "__main__":
    main()
