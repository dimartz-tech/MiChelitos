// Registro de las vistas: el único sitio donde se construye cada una con sus
// dependencias y se registra su ruta y su puente de manejadores.
//
// Lo llama `ui/componer.ts`, que es lo mismo que ejecutan la aplicación y las
// pruebas de interacción. Una pestaña nueva añade aquí una línea de construcción
// y otra de registro. Ver `division_de_ui_limpia.md`.

import type { ServiciosComunes, Vista } from '../ui/servicios';
import { VistaCapital, puenteCapital } from './capital.js';
import { VistaCuentas, puenteCuentas } from './cuentas.js';
import { VistaDashboard, puenteDashboard } from './dashboard.js';
import { VistaEfectivo, puenteEfectivo } from './efectivo.js';
import { VistaGastos, puenteGastos } from './gastos.js';
import { VistaIngresos, puenteIngresos } from './ingresos.js';
import { VistaPrestamos, puentePrestamos } from './prestamos.js';
import { VistaAjustes, puenteAjustes } from './ajustes.js';
import { VistaTarjetas, puenteTarjetas } from './tarjetas.js';
import { VistaResumen, puenteResumen } from './resumen.js';
import { VistaSuscripciones, puenteSuscripciones } from './suscripciones.js';

/** Dónde se registra una vista: su ruta, y los manejadores que el HTML llama como `appUI.<método>`. */
export interface RegistroDeVistas {
    registrar(ruta: string, vista: Vista, puente: object): void;
}

export function registrarVistas(registro: RegistroDeVistas, servicios: ServiciosComunes, api: typeof AppAPI): void {
    const efectivo = new VistaEfectivo({ ...servicios, api });
    registro.registrar('efectivo', efectivo, puenteEfectivo(efectivo));

    const cuentas = new VistaCuentas({ ...servicios, api });
    registro.registrar('cuentas', cuentas, puenteCuentas(cuentas));

    const resumen = new VistaResumen({ ...servicios, api });
    registro.registrar('resumen', resumen, puenteResumen(resumen));

    const dashboard = new VistaDashboard({ ...servicios, api });
    registro.registrar('dashboard', dashboard, puenteDashboard(dashboard));

    const capital = new VistaCapital({ ...servicios, api });
    registro.registrar('capital', capital, puenteCapital(capital));

    const suscripciones = new VistaSuscripciones({ ...servicios, api });
    registro.registrar('suscripciones', suscripciones, puenteSuscripciones(suscripciones));

    const gastos = new VistaGastos({ ...servicios, api });
    registro.registrar('gastos', gastos, puenteGastos(gastos));

    const ingresos = new VistaIngresos({ ...servicios, api });
    registro.registrar('ingresos', ingresos, puenteIngresos(ingresos));

    const prestamos = new VistaPrestamos({ ...servicios, api });
    registro.registrar('prestamos', prestamos, puentePrestamos(prestamos));

    const ajustes = new VistaAjustes({ ...servicios, api });
    registro.registrar('ajustes', ajustes, puenteAjustes(ajustes));

    const tarjetas = new VistaTarjetas({ ...servicios, api });
    registro.registrar('tarjetas', tarjetas, puenteTarjetas(tarjetas));
}
