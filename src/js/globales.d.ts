// Lo que la aplicación recibe del entorno y TypeScript no conoce.
interface Window {
    /** Puente de Tauri (`withGlobalTauri`). Ausente fuera de la aplicación. */
    __TAURI__?: any;
}
