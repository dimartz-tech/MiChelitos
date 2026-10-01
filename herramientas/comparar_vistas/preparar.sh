#!/usr/bin/env bash
# Prepara la comparación de vistas con los datos REALES: ver README.md.
#
#   herramientas/comparar_vistas/preparar.sh <referencia-git-anterior> [carpeta-de-trabajo]
#   herramientas/comparar_vistas/preparar.sh limpiar [carpeta-de-trabajo]
#
# Nada de esto entra al repositorio: la carpeta de trabajo (por defecto
# /tmp/comparar-vistas) contiene una COPIA de tus datos y se borra con `limpiar`.
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/../.." && pwd)"
if [ "${1:-}" = "limpiar" ]; then
    TRABAJO="${2:-/tmp/comparar-vistas}"
    git -C "$RAIZ" worktree remove --force "$TRABAJO/anterior-fuente" 2>/dev/null || true
    rm -rf "$TRABAJO"
    echo "Borrado $TRABAJO (incluye la copia de los datos)."
    exit 0
fi

REF="${1:?uso: preparar.sh <referencia-git-anterior> [carpeta-de-trabajo]}"
TRABAJO="${2:-/tmp/comparar-vistas}"
BASE_VIVA="$HOME/.michelitos/databases/sql/michelitos.db"
CAPITAL_VIVO="$HOME/.michelitos/databases/nosql/capital.json"
[ -f "$BASE_VIVA" ] || { echo "No encuentro la base en $BASE_VIVA"; exit 1; }

rm -rf "$TRABAJO"; mkdir -p "$TRABAJO"; chmod 700 "$TRABAJO"

# 1. Copia consistente de los datos (VACUUM INTO solo LEE la base viva).
sqlite3 "$BASE_VIVA" "VACUUM INTO '$TRABAJO/copia.db'"
cp "$CAPITAL_VIVO" "$TRABAJO/capital.json" 2>/dev/null || echo '{}' > "$TRABAJO/capital.json"

# 2. Volcado de lo que leen las vistas, con el Rust ACTUAL, sobre la copia.
( cd "$RAIZ" && VOLCADO_DB="$TRABAJO/copia.db" VOLCADO_CAPITAL="$TRABAJO/capital.json" \
    VOLCADO_SALIDA="$TRABAJO/volcado.json" \
    cargo test --offline --manifest-path src-tauri/Cargo.toml volcado_de_datos_para_comparar_vistas -- --ignored >"$TRABAJO/cargo.log" 2>&1 )

# 3. Frontend anterior: se construye desde un worktree de la referencia.
git -C "$RAIZ" worktree add --detach "$TRABAJO/anterior-fuente" "$REF" >/dev/null 2>&1
if [ -f "$TRABAJO/anterior-fuente/tsconfig.build.json" ]; then
    ( cd "$TRABAJO/anterior-fuente" && npm install --no-audit --no-fund >/dev/null 2>&1 && npm run compilar >/dev/null 2>&1 )
fi
mkdir -p "$TRABAJO/anterior" "$TRABAJO/nueva"
cp -R "$TRABAJO/anterior-fuente/src/." "$TRABAJO/anterior/"

# 4. Frontend nuevo: el árbol actual, compilado.
( cd "$RAIZ" && npm run compilar >/dev/null 2>&1 )
cp -R "$RAIZ/src/." "$TRABAJO/nueva/"

# 5. Fuera los fuentes TypeScript y las pruebas: solo lo que sirve el navegador.
find "$TRABAJO/anterior" "$TRABAJO/nueva" \( -name '*.ts' -o -name '*.test.js' \) -delete

# 6. El puente simulado, antes de api.js, en las dos copias.
python3 - "$TRABAJO" "$RAIZ/herramientas/comparar_vistas/mock.js" <<'PY'
import sys, io
trabajo, mock = sys.argv[1], io.open(sys.argv[2], encoding='utf-8').read()
for carpeta in ('anterior', 'nueva'):
    ruta = f'{trabajo}/{carpeta}/index.html'
    s = io.open(ruta, encoding='utf-8').read()
    assert '<script src="js/api.js">' in s, f'{ruta}: no encuentro api.js'
    s = s.replace('<script src="js/api.js">', f'<script>{mock}</script>\n<script src="js/api.js">', 1)
    io.open(ruta, 'w', encoding='utf-8').write(s)
io.open(f'{trabajo}/comparar.html', 'w', encoding='utf-8').write('<!doctype html><meta charset="utf-8"><body></body>')
PY

echo "Listo en $TRABAJO. Siguientes pasos:"
echo "  1. (cd $TRABAJO && python3 -m http.server 8770 --bind 127.0.0.1)"
echo "  2. abre http://127.0.0.1:8770/comparar.html y ejecuta herramientas/comparar_vistas/comparar.js en la consola"
echo "  3. al terminar: herramientas/comparar_vistas/preparar.sh limpiar"
