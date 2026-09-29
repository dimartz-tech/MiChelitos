# Respaldos: crear y restaurar

## Dónde están
`~/.michelitos/databases/respaldos/`. Cada respaldo es `michelitos_<fecha>_<motivo>.db` y, si había capital, `michelitos_<fecha>_<motivo>.capital.json` con el mismo nombre. Se conservan los 10 más recientes.

## Cuándo se crean
* Al arrancar, si la base o el capital cambiaron desde el último respaldo (antes de migrar el esquema).
* Bajo demanda, con el comando `crear_respaldo`.
* Antes de cada restauración («antes de restaurar»).

## Cómo restaurar
1. `listar_respaldos` → elegir un nombre.
2. `restaurar_respaldo(nombre)`. Devuelve dónde quedó el respaldo del estado sustituido, si se restauró el capital y la versión del esquema.
3. Para deshacer: `restaurar_respaldo` con el nombre de ese respaldo «antes de restaurar».

## Qué garantiza
* Se rechaza un respaldo que no abre, no es íntegro, tiene referencias rotas o es de un esquema más nuevo. En esos casos no cambia nada.
* La base se sustituye con un renombrado atómico y se descartan los archivos `-wal`/`-shm` de la anterior.

## Límites
* Migrar es de ida: un respaldo de un esquema **más viejo** se restaura y se migra al abrir; uno **más nuevo** no se restaura.
* Un respaldo hecho antes de esta versión no trae capital; al restaurarlo se conserva el capital actual.
* Los respaldos viven en el mismo disco. Para un desastre del equipo hace falta la copia fuera de él (método 2027, aún sin conectar).
* Tras restaurar, conviene reiniciar la aplicación para que las pantallas abiertas recarguen los datos.
