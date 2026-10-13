// --- MICHELITOS TAURI - PUNTO DE ENTRADA E INICIALIZACIÓN DE LA SPA ---
//
// Es un módulo que arranca `composicion.ts` con los servicios ya compuestos (antes
// era un script clásico que llamaba al global `appUI`). Sigue definiendo `navigate`
// como global porque lo llaman los manejadores en línea del HTML
// (`onclick="navigate('ajustes')"`).

import type { ApiDe, Avisos, Dom, Enrutador } from './ui/servicios';

export interface EntornoDeInicio {
    enrutador: Enrutador;
    avisos: Avisos;
    dom: Dom;
    api: ApiDe<'procesarSuscripciones'>;
}

export function iniciarAplicacion({ enrutador, avisos, dom, api }: EntornoDeInicio): void {
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
    (window as unknown as { navigate: typeof navigate }).navigate = navigate;

    const arrancar = (): void => {
        // 1. Cargar la pestaña predeterminada (Dashboard)
        navigate('dashboard');

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
