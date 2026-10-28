// --- MICHELITOS TAURI - PUNTO DE ENTRADA E INICIALIZACIÓN DE LA SPA ---
//
// Es un módulo que arranca `composicion.ts` con los servicios ya compuestos (antes
// era un script clásico que llamaba al global `appUI`). Engancha además la delegación de
// eventos (`ui/delegacion.ts`): el HTML ya no lleva manejadores en línea, sino
// `data-accion="…"`, y `navegar` (el menú, los enlaces de las vistas) llama a `navigate`.

import { accionesGenerales, delegar, type Acciones } from './ui/delegacion.js';
import { enmascarar } from './ui/mascaras.js';
import type { ApiDe, Avisos, Dom, Enrutador } from './ui/servicios';

export interface EntornoDeInicio {
    enrutador: Enrutador;
    avisos: Avisos;
    dom: Dom;
    api: ApiDe<'procesarSuscripciones' | 'asegurarFotoDelMes'>;
    /** El puente de las vistas (`window.appUI`): las acciones a las que apuntan los `data-accion`. */
    acciones: Acciones;
}

export function iniciarAplicacion({ enrutador, avisos, dom, api, acciones }: EntornoDeInicio): void {
    // --- ROUTER GLOBAL ---
    function navigate(tab: string): void {
        // 1. Alternar active class en la navegación superior
        const navItems = document.querySelectorAll('.sidebar-nav .nav-item');
        navItems.forEach(item => {
            item.classList.remove('active');
            item.removeAttribute('aria-current');
        });

        const activeBtn = dom.buscar(`nav-btn-${tab}`);
        if (activeBtn) {
            activeBtn.classList.add('active');
            activeBtn.setAttribute('aria-current', 'page');
        }

        // 2. Renderizar a través de la UI
        void enrutador.mostrar(tab);
    }

    // --- MANEJADORES DELEGADOS (sustituyen a los onclick/onsubmit/onchange/oninput en línea) ---
    const generales = accionesGenerales({ dom, navegar: navigate });
    const repetidas = Object.keys(generales).filter(nombre => nombre in acciones);
    if (repetidas.length > 0) throw new Error(`Acciones generales pisadas por una vista: ${repetidas.join(', ')}.`);
    delegar(document, { ...acciones, ...generales });
    enmascarar(document);

    const arrancar = (): void => {
        // 1. Cargar la pestaña predeterminada (Dashboard)
        navigate('dashboard');

        // 2a. La foto mensual del patrimonio: solo se toma si todavía no existe la del mes. Un fallo se avisa, no bloquea.
        api.asegurarFotoDelMes().then(({ creada, foto }) => {
            if (!creada) return;
            if (foto && foto.calidad === 'con_aviso') {
                avisos.mostrar('Se tomó la foto del patrimonio de este mes, pero con aviso: no se pudo leer todo el capital y se contó como cero.', 'error');
            } else {
                avisos.mostrar('Se tomó la foto del patrimonio de este mes.');
            }
        }).catch(err => avisos.mostrar(`No se pudo tomar la foto del patrimonio de este mes: ${String(err)}`, 'error'));

        // 2. Procesar cargos de suscripciones pendientes al iniciar
        setTimeout(() => {
            api.procesarSuscripciones().then(({ cargos, fallos }) => {
                // Un cobro que falló no corta los demás: se avisa aparte, como error, y se reintenta al reabrir.
                fallos.forEach(msg => avisos.mostrar(msg, 'error'));
                if (cargos.length > 0) {
                    cargos.forEach(msg => avisos.mostrar(msg, 'success'));
                    // Si hubo cargos, recargar la pantalla activa para reflejar los balances
                    const activeTab = document.querySelector('.sidebar-nav .nav-item.active');
                    if (activeTab) {
                        const route = activeTab.id.replace('nav-btn-', '');
                        navigate(route);
                    }
                }
            }).catch(err => console.error("Error al procesar suscripciones automáticas:", err));
        }, 1500); // Dar un pequeño respiro al arranque

        // 3. Control de tema Claro/Oscuro
        const themeToggleBtn = dom.buscar('theme-toggle-btn');
        if (themeToggleBtn) {
            const savedTheme = localStorage.getItem('desktop-theme') || 'light';
            
            if (savedTheme === 'light') {
                document.body.classList.add('light-theme');
                themeToggleBtn.textContent = '☀️';
            } else {
                document.body.classList.remove('light-theme');
                themeToggleBtn.textContent = '🌙';
            }

            themeToggleBtn.addEventListener('click', () => {
                if (document.body.classList.contains('light-theme')) {
                    document.body.classList.remove('light-theme');
                    themeToggleBtn.textContent = '🌙';
                    localStorage.setItem('desktop-theme', 'dark');
                    avisos.mostrar("Modo Oscuro activado.");
                } else {
                    document.body.classList.add('light-theme');
                    themeToggleBtn.textContent = '☀️';
                    localStorage.setItem('desktop-theme', 'light');
                    avisos.mostrar("Modo Claro activado.");
                }
                
                // Re-renderizar pestaña actual
                const activeTab = document.querySelector('.sidebar-nav .nav-item.active');
                if (activeTab) {
                    const route = activeTab.id.replace('nav-btn-', '');
                    navigate(route);
                }
            });
        }

};

    // Un módulo se ejecuta antes de `DOMContentLoaded`, pero por si alguna vez no fuera así.
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arrancar);
    else arrancar();
}
