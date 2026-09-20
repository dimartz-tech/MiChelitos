#!/usr/bin/env python3
"""Revisión previa a publicar, en una sola orden.

Sustituye a la lista de comprobaciones que había que recordar y recomponer
a mano antes de cada push. Esa lista falló dos veces en la misma semana, y no
por ser corta: por ser una lista. Se barrían nombres de entidades y rutas
absolutas porque eran lo que uno recordaba, y se escapaban los importes.

Tres comprobaciones, en orden de lo que más duele que falle:

    1. Privacidad — que nada del diff identifique finanzas reales.
    2. Coherencia de versión — que el número no se desincronice entre
       los cuatro archivos que lo declaran.
    3. Pruebas — Rust y JavaScript.

**Este archivo no contiene ningún dato del titular.** Los importes con los
que compara se leen de la base viva, que vive fuera del repositorio; si no
está disponible, esa parte se omite diciéndolo en voz alta en vez de dar un
visto bueno que no ha comprobado nada.

Uso:
    python3 herramientas/revisar.py            # contra origin/main
    python3 herramientas/revisar.py --base X   # contra otra referencia
"""

from __future__ import annotations

import argparse
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
BASE_DE_DATOS = Path.home() / ".michelitos" / "databases" / "sql" / "michelitos.db"

# Rastros del equipo local: usuario, rutas absolutas, configuración de claves.
PATRONES_DE_ENTORNO = [
    (r"/Users/[a-z]", "ruta absoluta del equipo"),
    (r"use-keychain", "configuración de clave SSH"),
    (r"\bid_[a-z]+\b(?!\s*=)", "nombre de un archivo de clave"),
]

# Un importe: dos decimales, con separadores de millar o guiones bajos.
IMPORTE = re.compile(r"\d[\d_,]*\.\d{2}\b")

# Un revisor contiene por fuerza los patrones que busca, y señalarse a sí
# mismo enseñaría a ignorar sus propios avisos.
EXENTOS = ("herramientas/",)


def ejecutar(orden: list[str], cwd: Path = RAIZ) -> tuple[int, str]:
    r = subprocess.run(orden, cwd=cwd, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def lineas_anadidas(base: str) -> list[tuple[str, str]]:
    """Las líneas que este trabajo añade, con el archivo al que pertenecen.

    Mira **lo confirmado y lo que todavía no lo está**. Revisar solo los
    commits dejaría fuera el momento en que la revisión sirve de algo: antes
    de confirmar. Se comprobó, y era precisamente por lo que la primera
    versión de este guion no cazaba nada.
    """
    diffs = []
    codigo, salida = ejecutar(["git", "diff", "-U0", f"{base}...HEAD"])
    if codigo == 0:
        diffs.append(salida)
    # El índice y el árbol de trabajo, incluidos los archivos nuevos.
    diffs.append(ejecutar(["git", "diff", "-U0", "HEAD"])[1])
    for sin_seguir in ejecutar(
        ["git", "ls-files", "--others", "--exclude-standard"]
    )[1].split():
        ruta = RAIZ / sin_seguir
        if ruta.is_file():
            try:
                contenido = ruta.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            diffs.append(
                f"+++ b/{sin_seguir}\n"
                + "".join(f"+{l}\n" for l in contenido.splitlines())
            )

    archivo = "?"
    resultado = []
    vistas = set()
    for salida in diffs:
        for linea in salida.splitlines():
            if linea.startswith("+++ b/"):
                archivo = linea[6:]
            elif linea.startswith("+") and not linea.startswith("+++"):
                clave = (archivo, linea)
                if clave not in vistas:
                    vistas.add(clave)
                    resultado.append((archivo, linea[1:]))
    return resultado


def importes_reales() -> set[str] | None:
    """Los importes de la base viva, normalizados. `None` si no está."""
    if not BASE_DE_DATOS.exists():
        return None

    consulta = """
        SELECT monto FROM gastos WHERE monto IS NOT NULL
        UNION SELECT costo_adicional FROM gastos WHERE costo_adicional > 0
        UNION SELECT monto_pagado FROM pagos_tarjeta
        UNION SELECT balance_actual FROM cuentas_ahorro
        UNION SELECT monto_origen FROM transacciones_cuentas
        UNION SELECT monto_destino FROM transacciones_cuentas
        UNION SELECT balance_pesos FROM tarjetas
        UNION SELECT balance_dolares FROM tarjetas
        UNION SELECT monto_cuota FROM prestamos
        UNION SELECT monto_prestamo FROM prestamos
    """
    con = sqlite3.connect(f"file:{BASE_DE_DATOS}?mode=ro", uri=True)
    try:
        valores = {f"{v[0]:.2f}" for v in con.execute(consulta) if v[0]}
    finally:
        con.close()

    # Los importes pequeños coinciden por casualidad con cualquier ejemplo
    # —1.00, 100.00, 0.30—, de modo que señalarlos sería ruido que acabaría
    # haciendo ignorar el aviso entero.
    return {v for v in valores if float(v) >= 1000}


def normalizar(texto: str) -> str:
    return f"{float(texto.replace('_', '').replace(',', '')):.2f}"


def revisar_privacidad(base: str) -> list[str]:
    fallos = []
    anadidas = [
        (a, l) for a, l in lineas_anadidas(base) if not a.startswith(EXENTOS)
    ]

    for archivo, linea in anadidas:
        for patron, motivo in PATRONES_DE_ENTORNO:
            if re.search(patron, linea):
                fallos.append(f"{archivo}: {motivo} — {linea.strip()[:70]}")

    reales = importes_reales()
    if reales is None:
        print(f"  · base viva no disponible en {BASE_DE_DATOS}")
        print("    NO se comprobaron los importes. El visto bueno es parcial.")
        return fallos

    for archivo, linea in anadidas:
        for bruto in IMPORTE.findall(linea):
            try:
                valor = normalizar(bruto)
            except ValueError:
                continue
            if valor in reales:
                fallos.append(
                    f"{archivo}: el importe {bruto} existe en la base viva — "
                    "invéntalo en vez de copiarlo"
                )
    return fallos


def revisar_version() -> list[str]:
    declarado = {
        "package.json": r'"version":\s*"([^"]+)"',
        "src-tauri/Cargo.toml": r'^version\s*=\s*"([^"]+)"',
        "src-tauri/tauri.conf.json": r'"version":\s*"([^"]+)"',
    }
    versiones = {}
    for archivo, patron in declarado.items():
        texto = (RAIZ / archivo).read_text(encoding="utf-8")
        m = re.search(patron, texto, re.M)
        if m:
            versiones[archivo] = m.group(1)

    bloqueo = re.search(
        r'name = "michelitos-tauri"\nversion = "([^"]+)"',
        (RAIZ / "src-tauri" / "Cargo.lock").read_text(encoding="utf-8"),
    )
    if bloqueo:
        versiones["Cargo.lock"] = bloqueo.group(1)

    historial = re.search(
        r"Versión ([0-9.]+) \(Versión Actual\)",
        (RAIZ / "historial_versiones.md").read_text(encoding="utf-8"),
    )
    if historial:
        versiones["historial_versiones.md"] = historial.group(1)

    distintas = set(versiones.values())
    if len(distintas) > 1:
        detalle = ", ".join(f"{a}={v}" for a, v in versiones.items())
        return [f"la versión no coincide entre archivos: {detalle}"]
    return []


def revisar_pruebas() -> list[str]:
    fallos = []
    codigo, salida = ejecutar(
        ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml"]
    )
    if codigo != 0:
        resumen = [l for l in salida.splitlines() if l.startswith("test result")]
        fallos.append("las pruebas de Rust no pasan: " + ("; ".join(resumen) or "ver salida"))

    codigo, salida = ejecutar(["npm", "test"])
    if codigo != 0:
        fallos.append("las pruebas de JavaScript no pasan")
    return fallos


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--base", default="origin/main", help="referencia con la que comparar")
    p.add_argument("--rapido", action="store_true", help="omitir las pruebas")
    args = p.parse_args()

    bloques = [("Privacidad", lambda: revisar_privacidad(args.base)),
               ("Versión", revisar_version)]
    if not args.rapido:
        bloques.append(("Pruebas", revisar_pruebas))

    total = 0
    for nombre, comprobar in bloques:
        print(f"\n{nombre}")
        fallos = comprobar()
        if fallos:
            total += len(fallos)
            for f in fallos:
                print(f"  ✗ {f}")
        else:
            print("  ✓ sin hallazgos")

    print()
    if total:
        print(f"{total} hallazgo(s). No publiques hasta resolverlos.")
        return 1
    print("Listo para publicar.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
