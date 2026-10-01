// Registro de las vistas extraídas de `ui.ts`: el único sitio donde se
// construye cada una con sus dependencias y se cuelga de `appUI`.
//
// Es una función y no el cuerpo de `composicion.ts` para que **lo mismo que
// ejecuta la aplicación lo ejecuten las pruebas de interacción** con servicios
// y API falsos (`pruebas/js/ayudas/cargar_interfaz.js`): así no hay una copia
// del cableado que pueda desviarse de la real.
//
// Cada extracción de `ui.ts` añade aquí su vista: una línea de construcción y
// otra de registro. Ver `division_de_ui_limpia.md`.

import type { AppUIAntigua, ServiciosComunes } from '../ui/servicios';
import { VistaCuentas, puenteCuentas } from './cuentas.js';
import { VistaEfectivo, puenteEfectivo } from './efectivo.js';

export function registrarVistas(app: AppUIAntigua, servicios: ServiciosComunes, api: typeof AppAPI): void {
    const efectivo = new VistaEfectivo({ ...servicios, api });
    app.registrarVista('efectivo', efectivo, puenteEfectivo(efectivo));

    const cuentas = new VistaCuentas({ ...servicios, api });
    app.registrarVista('cuentas', cuentas, puenteCuentas(cuentas));
}
