// La composición de la interfaz: crea los servicios, construye cada vista con sus
// dependencias, las registra en el enrutador y arma el puente `appUI` que llaman
// los manejadores en línea del HTML (los atributos `onclick`, `onsubmit` y `onchange`).
//
// Es **una función** y no el cuerpo de `composicion.ts` para que lo mismo que ejecuta
// la aplicación lo ejecuten las pruebas de interacción con servicios y API falsos
// (`pruebas/js/ayudas/cargar_interfaz.js`): así no hay una copia del cableado que
// pueda desviarse de la real. Sustituye a la clase `AppUI`, que lo hacía todo a la
// vez (dibujar, avisar, formatear, enrutar y atender todos los manejadores).
//
// Es un módulo ES; `composicion.ts` lo llama con el entorno real.

import {
    crearAvisos, crearDialogosDePagina, crearEnrutador, crearMenuFlotanteDelNavegador, crearMotivo,
    formatoDelNavegador, modalesDelNavegador, referenciasDelNavegador,
    type Avisos, type Dialogos, type Dom, type Enrutador, type EntornoDeAvisos, type MenuFlotante,
    type Modales, type Reloj, type ServiciosComunes, type Vista,
} from './servicios.js';
import { registrarVistas, type RegistroDeVistas } from '../vistas/registro.js';

/** Lo que la composición necesita del entorno: el real en la aplicación, dobles en una prueba. */
export interface EntornoDeUi {
    dom: Dom;
    /** Donde se dibuja la pestaña activa (`#app-content`). */
    contenido: { innerHTML: string };
    /** Donde se apilan los avisos (`#notification-container`). */
    avisos: EntornoDeAvisos;
    ahora?: Reloj;
    dialogos?: Dialogos;
    modales?: Modales;
    menus?: MenuFlotante;
    /** Solo para pruebas: sustituyen al servicio real (por ejemplo, para registrar los avisos). */
    sustituir?: { avisos?: Avisos; enrutador?: Enrutador };
}

export interface InterfazComponida {
    /** El puente que cuelga de `window.appUI`: los manejadores de todas las vistas, y nada más. */
    appUI: Record<string, (...args: never[]) => unknown>;
    servicios: ServiciosComunes;
    /** El enrutador real (aunque una prueba haya sustituido el que ven las vistas). */
    enrutador: Enrutador;
    /** Las vistas registradas, por ruta. */
    vistas: ReadonlyMap<string, Vista>;
}

/**
 * Dónde se registra cada vista: su ruta y su puente de manejadores. Rechaza lo que se
 * pisaría **en silencio**: dos vistas con la misma ruta (la segunda ocultaría a la
 * primera) o con un manejador del mismo nombre (el botón de una haría lo de la otra).
 */
export function crearRegistroDeVistas(vistas: Map<string, Vista>, appUI: InterfazComponida['appUI']): RegistroDeVistas {
    return {
        registrar(ruta, vista, puente) {
            if (vistas.has(ruta)) throw new Error(`La ruta «${ruta}» ya tiene una vista registrada.`);
            const repetidos = Object.keys(puente).filter(nombre => nombre in appUI);
            if (repetidos.length > 0) throw new Error(`Manejadores repetidos en «${ruta}»: ${repetidos.join(', ')}.`);
            vistas.set(ruta, vista);
            Object.assign(appUI, puente);
        },
    };
}

export function componerInterfaz(entorno: EntornoDeUi, api: typeof AppAPI): InterfazComponida {
    const vistas = new Map<string, Vista>();
    const appUI: InterfazComponida['appUI'] = {};

    const pantalla = { contenido: entorno.contenido };
    const dialogos = entorno.dialogos ?? crearDialogosDePagina();
    const avisos = entorno.sustituir?.avisos ?? crearAvisos(entorno.avisos);
    const enrutador = crearEnrutador(pantalla, vistas);

    const servicios: ServiciosComunes = {
        avisos,
        formato: formatoDelNavegador,
        enrutador: entorno.sustituir?.enrutador ?? enrutador,
        pantalla,
        dom: entorno.dom,
        ahora: entorno.ahora ?? (() => new Date()),
        dialogos,
        motivo: crearMotivo(dialogos, avisos),
        modales: entorno.modales ?? modalesDelNavegador,
        menus: entorno.menus ?? crearMenuFlotanteDelNavegador(),
        referencias: referenciasDelNavegador,
    };

    registrarVistas(crearRegistroDeVistas(vistas, appUI), servicios, api);

    return { appUI, servicios, enrutador, vistas };
}
