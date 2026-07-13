// --- MICHELITOS TAURI - PUNTO DE ENTRADA E INICIALIZACIÓN DE LA SPA ---

document.addEventListener('DOMContentLoaded', () => {
    // 1. Cargar la pestaña predeterminada (Dashboard)
    navigate('dashboard');

    // 2. Control de tema Claro/Oscuro
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
