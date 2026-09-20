# Herramientas

## `revisar.py` — la revisión previa a publicar, en una orden

```bash
python3 herramientas/revisar.py            # todo
python3 herramientas/revisar.py --rapido   # sin las pruebas
```

Sustituye a la lista de comprobaciones que había que recordar y recomponer
antes de cada push. **Esa lista falló dos veces en la misma semana, y no por
ser corta: por ser una lista.** Se barrían nombres de entidades y rutas
absolutas —que es lo que uno recuerda— y se escapaban los importes.

Comprueba tres cosas, en orden de lo que más duele que falle:

| | Qué mira |
|---|---|
| **Privacidad** | Rutas del equipo local, y que ningún importe del diff exista en la base viva |
| **Versión** | Que el número coincida en los cinco sitios que lo declaran |
| **Pruebas** | Rust y JavaScript |

### Por qué compara contra la base y no contra una lista

Una lista de importes prohibidos **sería ella misma la fuga**. Los valores se
leen de la base viva, que está fuera del repositorio, de modo que este
directorio no contiene ningún dato del titular.

Si la base no está disponible —otra máquina, integración continua— la
comprobación de importes se omite **diciéndolo**, y el visto bueno queda
declarado como parcial. Un revisor que aprueba sin haber comprobado es peor
que no tener revisor.

### Dos decisiones que parecen detalles y no lo son

- **Mira también lo que no está confirmado.** La primera versión solo
  comparaba commits y por eso no cazaba nada: la revisión sirve justo antes de
  confirmar, no después.
- **Se exime a sí misma.** Un revisor contiene por fuerza los patrones que
  busca, y señalarse a sí mismo enseña a ignorar sus propios avisos.

### Umbral de ruido

Solo se comparan importes de 1 000 en adelante. Por debajo, cualquier ejemplo
coincide por casualidad con algo —`1.00`, `100.00`— y el aviso acabaría
ignorándose, que es la forma habitual en que una comprobación deja de servir.
