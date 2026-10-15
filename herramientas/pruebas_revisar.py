#!/usr/bin/env python3
"""Pruebas de `revisar.py`: sus códigos de salida y su manera de fallar.

Todo con datos inventados y un HOME temporal: nunca toca la base viva.

    python3 -m unittest herramientas/pruebas_revisar.py
"""

import os
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERRAMIENTAS = Path(__file__).resolve().parent
REVISAR = HERRAMIENTAS / "revisar.py"
sys.path.insert(0, str(HERRAMIENTAS))
import revisar  # noqa: E402


def ejecutar(*args, home):
    entorno = {**os.environ, "HOME": str(home)}
    entorno.pop("GITHUB_ACTIONS", None)
    r = subprocess.run(
        [sys.executable, str(REVISAR), "--rapido", *args],
        capture_output=True, text=True, env=entorno,
    )
    return r.returncode, r.stdout + r.stderr


class SalidasDeRevisar(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.home = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def test_una_referencia_base_inexistente_no_da_visto_bueno(self):
        codigo, salida = ejecutar("--base", "origen/que-no-existe", home=self.home)
        self.assertEqual(codigo, 2, salida)
        self.assertIn("no existe", salida)
        self.assertNotIn("Listo para publicar", salida)

    def test_sin_base_viva_el_visto_bueno_es_parcial_y_lo_dice(self):
        codigo, salida = ejecutar("--base", "HEAD", home=self.home)
        self.assertEqual(codigo, 0, salida)
        self.assertIn("PARCIAL", salida)
        self.assertNotIn("Listo para publicar", salida)

    def test_exigir_importes_falla_si_no_hay_base_viva(self):
        codigo, salida = ejecutar("--base", "HEAD", "--exigir-importes", home=self.home)
        self.assertEqual(codigo, 3, salida)

    def test_en_github_actions_el_aviso_parcial_se_anota(self):
        entorno = {**os.environ, "HOME": str(self.home), "GITHUB_ACTIONS": "true"}
        r = subprocess.run(
            [sys.executable, str(REVISAR), "--rapido", "--base", "HEAD"],
            capture_output=True, text=True, env=entorno,
        )
        self.assertIn("::warning::", r.stdout)


class ImportesDeLaBaseViva(unittest.TestCase):
    def test_solo_cuentan_los_importes_de_mil_o_mas_con_dos_decimales(self):
        with tempfile.TemporaryDirectory() as tmp:
            ruta = Path(tmp) / "inventada.db"
            con = sqlite3.connect(ruta)
            con.executescript(
                """
                CREATE TABLE gastos (monto REAL, costo_adicional REAL);
                CREATE TABLE pagos_tarjeta (monto_pagado REAL);
                CREATE TABLE cuentas_ahorro (balance_actual REAL);
                CREATE TABLE transacciones_cuentas (monto_origen REAL, monto_destino REAL);
                CREATE TABLE tarjetas (balance_pesos REAL, balance_dolares REAL);
                CREATE TABLE prestamos (monto_cuota REAL, monto_prestamo REAL);
                INSERT INTO gastos VALUES (12345.67, 0), (50.00, 0);
                INSERT INTO pagos_tarjeta VALUES (2000.5);
                INSERT INTO cuentas_ahorro VALUES (0);
                INSERT INTO transacciones_cuentas VALUES (0, 0);
                INSERT INTO tarjetas VALUES (0, 0);
                INSERT INTO prestamos VALUES (0, 0);
                """
            )
            con.commit()
            con.close()
            anterior = revisar.BASE_DE_DATOS
            revisar.BASE_DE_DATOS = ruta
            try:
                reales = revisar.importes_reales()
            finally:
                revisar.BASE_DE_DATOS = anterior
        self.assertEqual(reales, {"12345.67", "2000.50"})

    def test_sin_archivo_no_hay_importes(self):
        anterior = revisar.BASE_DE_DATOS
        revisar.BASE_DE_DATOS = Path("/no/existe/michelitos.db")
        try:
            self.assertIsNone(revisar.importes_reales())
        finally:
            revisar.BASE_DE_DATOS = anterior


if __name__ == "__main__":
    unittest.main()
