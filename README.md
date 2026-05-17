```markdown
# Guía de Desarrollo - STGC

Este proyecto utiliza un único repositorio con **ramas huérfanas independientes** para el desarrollo aislado de cada microservicio. Al finalizar el desarrollo, todas las ramas se fusionarán en `main`.

## Estructura del Repositorio
* `main`: Al final del desarrollo se alojara el código consolidado dentro de esta rama.
* `billing-service`: Rama independiente para el submodulo de facturación.
* `inventory-service`: Rama independiente para el submodulo de inventario.
* `pos-service`: Rama independiente para el punto de venta.
* `report-service`: Rama independiente para el submodulo de reportes.
* `auth-service`: Rama independiente para el submodulo de autenticación y seguridad.

---

## Flujo de Trabajo Diario

### 1. Cambiar al microservicio a trabajar
Asegúrate de estar en la rama correcta:
```bash
git checkout nombre-del-servicio

```

### 2. Reglas Estrictas de Desarrollo e Ignorados

Para evitar conflictos al fusionar las ramas al final, debes cumplir esto:

* **Todo el código va en carpetas:** Todo el código **debe crearse estrictamente dentro de una carpeta** con el nombre del microservicio (ej. `/billing-service/code.py`), no dejes archivos sueltos en la raíz de la rama.
* **Ubicación del `.gitignore`:** El archivo `.gitignore` debe crearse **adentro de la carpeta del microservicio** (ej. `/billing-service/.gitignore`), no en la raíz de la rama.

### 3. Guardar y subir cambios

```bash
git add .
git commit -m "Descripción breve de los cambios"
git push origin nombre-del-servicio

```

---

## Integración Final (Unir a `main`)

Cuando el servicio esté terminado, se  hará lo siguiente:

```bash
git checkout main
git pull origin main
git merge nombre-del-servicio --allow-unrelated-histories
git add .
git commit -m "Merge: Integración de nombre-del-servicio"
git push origin main

```