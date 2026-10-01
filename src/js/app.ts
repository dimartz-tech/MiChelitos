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
        // 1. Alternar active class en la barra lateral
        const navItems = document.querySelectorAll('.sidebar-nav .nav-item');
        navItems.forEach(item => item.classList.remove('active'));

        const activeBtn = dom.buscar(`nav-btn-${tab}`);
        if (activeBtn) {
            activeBtn.classList.add('active');
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
            api.procesarSuscripciones().then(mensajes => {
                if (mensajes && mensajes.length > 0) {
                    mensajes.forEach(msg => avisos.mostrar(msg, 'success'));
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
            const savedTheme = localStorage.getItem('desktop-theme') || 'dark';
            
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

        // 4. Control de colapsado del menú lateral (Sidebar)
        const sidebarCollapseBtn = dom.buscar('sidebar-collapse-btn');
        const sidebar = document.querySelector('.app-sidebar');
        if (sidebarCollapseBtn && sidebar) {
            const savedCollapsed = localStorage.getItem('sidebar-collapsed') === 'true';
            if (savedCollapsed) {
                sidebar.classList.add('collapsed');
                sidebarCollapseBtn.textContent = '▶️';
            } else {
                sidebarCollapseBtn.textContent = '◀️';
            }

            sidebarCollapseBtn.addEventListener('click', () => {
                sidebar.classList.toggle('collapsed');
                const isCollapsed = sidebar.classList.contains('collapsed');
                sidebarCollapseBtn.textContent = isCollapsed ? '▶️' : '◀️';
                localStorage.setItem('sidebar-collapsed', String(isCollapsed));
            });
        }
};

    // Un módulo se ejecuta antes de `DOMContentLoaded`, pero por si alguna vez no fuera así.
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arrancar);
    else arrancar();
}
