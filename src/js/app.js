// --- MICHELITOS TAURI - PUNTO DE ENTRADA E INICIALIZACIÓN DE LA SPA ---

document.addEventListener('DOMContentLoaded', () => {
    // 1. Cargar la pestaña predeterminada (Dashboard)
    navigate('dashboard');

    // 2. Procesar cargos de suscripciones pendientes al iniciar
    setTimeout(() => {
        AppAPI.procesarSuscripciones().then(mensajes => {
            if (mensajes && mensajes.length > 0) {
                mensajes.forEach(msg => appUI.showToast(msg, 'success'));
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
    const themeToggleBtn = document.getElementById('theme-toggle-btn');
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
                appUI.showToast("Modo Oscuro activado.");
            } else {
                document.body.classList.add('light-theme');
                themeToggleBtn.textContent = '☀️';
                localStorage.setItem('desktop-theme', 'light');
                appUI.showToast("Modo Claro activado.");
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
    const sidebarCollapseBtn = document.getElementById('sidebar-collapse-btn');
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
            localStorage.setItem('sidebar-collapsed', isCollapsed);
        });
    }
});

// --- ROUTER GLOBAL ---
function navigate(tab) {
    // 1. Alternar active class en la barra lateral
    const navItems = document.querySelectorAll('.sidebar-nav .nav-item');
    navItems.forEach(item => item.classList.remove('active'));

    const activeBtn = document.getElementById(`nav-btn-${tab}`);
    if (activeBtn) {
        activeBtn.classList.add('active');
    }

    // 2. Renderizar a través de la UI
    appUI.render(tab);
}
