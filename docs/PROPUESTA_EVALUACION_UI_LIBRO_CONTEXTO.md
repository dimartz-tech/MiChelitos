# Propuesta local para evaluar «Libro y contexto» en main

Estado: rama de evaluación `review/ui-libro-contexto`, creada desde el `origin/main` local en `d6add00`. No se ha modificado `main`, no se ha publicado la rama y no se ha abierto PR.

## Qué se ofrece para evaluar ahora

- Cabecera azul profundo con navegación superior de las once secciones actuales, sin retirar rutas.
- Tema claro de lectura como predeterminado y tema oscuro disponible; se respeta una preferencia ya guardada.
- Sistema visual transversal para tarjetas, tablas, formularios, botones, estados y avisos; esmeralda para estados positivos y dorado para atención.
- Foco visible y navegación horizontal desplazable en pantallas estrechas; el antiguo control de colapsar el menú ya no tiene sentido en esta disposición.
- Sin cambios en IPC, dominio, persistencia, cálculos o reglas financieras.

La propuesta procede de los commits `d6ea88a` (Centro operativo) y `1f15533` (Libro y contexto) de la versión independiente SDD. **No se deben aplicar esos commits por cherry-pick directo:** allí las vistas son JavaScript y `main` ya usa vistas TypeScript con dependencias inyectadas. Esta rama adapta únicamente la capa visual compartida de forma revisable.

## Alcance aún no portado

El Inicio de la versión SDD separa saldos DOP/USD y muestra gastos recientes junto a alertas y compromisos. El `Dashboard` de `main` usa otro contrato de datos y pruebas de totales de período. Portar aquella vista literalmente mezclaría modelos y podría reproducir importes engañosos. Antes de un merge del rediseño completo, trasladar el modelo de lectura a TypeScript, acordar las fuentes de saldos por divisa y probar con datos sintéticos los estados vacío, normal y de error. Hasta entonces, `main` conserva su Dashboard funcional actual.

## Validación y decisión de integración

- `pnpm run compilar`: pasó.
- `node --test --test-reporter=dot "pruebas/js/**/*.test.js"`: pasó en este worktree.
- Pendiente: revisión visual manual de las once secciones en una instalación desechable con datos sintéticos, incluyendo anchos reducidos y los dos temas. No abrir la base activa para esta evaluación.

Si la evaluación visual se aprueba, integrar primero este commit de armazón en una rama de integración actualizada desde `main`; después portar el Centro operativo como incremento independiente, con sus pruebas de divisa y sin cambiar reglas financieras. No fusionar esta propuesta únicamente por una compilación verde.
