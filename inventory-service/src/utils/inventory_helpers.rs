//! Lógica compartida entre los handlers de POS (CAFETERIA) y Finca (FINCA).
//!
//! Antes, `pos_inventory_handler.rs` y `finca_inventory_handler.rs` duplicaban
//! ~95% de estas funciones. Se centralizan aquí para mantener una sola fuente de
//! verdad (validaciones, conversión de unidades, reglas de estado).

use chrono::{DateTime, TimeZone, Utc};

use crate::models::enums::{EstadoInventario, UnidadMedida};
use crate::models::{CreateInventarioItem, InventarioItem, UpdateInventarioItem};

/// Parsea fechas flexibles (RFC3339 o `AAAA-MM-DD`). Devuelve `None` si es vacío/nulo/ inválido.
pub fn parse_flex_date(date_str: Option<String>) -> Option<DateTime<Utc>> {
    let s = date_str?.trim().to_string();
    if s.is_empty() || s == "null" {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return naive_date
            .and_hms_opt(0, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt));
    }
    None
}

/// Validación segura de decimales (EC-06). Evita los fallos de `f64::to_string()`.
/// Un valor es válido si al redondear a 2 decimales coincide consigo mismo.
pub fn precio_valido(valor: f64) -> bool {
    (valor * 100.0).round() / 100.0 == valor
}

/// Convierte cantidades entre unidades de masa. Otras combinaciones no son convertibles.
pub fn convert_unit(cantidad: f64, from: &UnidadMedida, to: &UnidadMedida) -> Result<f64, String> {
    if from == to {
        return Ok(cantidad);
    }
    let is_mass = |u: &UnidadMedida| {
        matches!(
            u,
            UnidadMedida::QUINTALES
                | UnidadMedida::ARROBAS
                | UnidadMedida::LIBRAS
                | UnidadMedida::KILOGRAMOS
        )
    };

    if is_mass(from) && is_mass(to) {
        let in_lb = match from {
            UnidadMedida::LIBRAS => cantidad,
            UnidadMedida::ARROBAS => cantidad * 25.0,
            UnidadMedida::QUINTALES => cantidad * 100.0,
            UnidadMedida::KILOGRAMOS => cantidad * 2.20462,
            _ => unreachable!(),
        };
        let result = match to {
            UnidadMedida::LIBRAS => in_lb,
            UnidadMedida::ARROBAS => in_lb / 25.0,
            UnidadMedida::QUINTALES => in_lb / 100.0,
            UnidadMedida::KILOGRAMOS => in_lb / 2.20462,
            _ => unreachable!(),
        };
        Ok((result * 100.0).round() / 100.0)
    } else {
        Err(format!("No se puede convertir de {:?} a {:?}", from, to))
    }
}

/// Determina el estado de stock automático tras un movimiento.
pub fn determinar_estado_inventario(cantidad: f64, stock_minimo: f64) -> EstadoInventario {
    match cantidad {
        q if q <= 0.0 => EstadoInventario::AGOTADO,
        q if q <= stock_minimo => EstadoInventario::STOCK_BAJO,
        _ => EstadoInventario::DISPONIBLE,
    }
}

/// Validaciones de negocio para la creación de un ítem (idéntico en POS y Finca).
pub fn validate_create_item(payload: &CreateInventarioItem) -> Result<(), String> {
    let nombre = payload.nombre.trim();
    if nombre.is_empty() {
        return Err("El nombre del producto es obligatorio.".into());
    }
    if nombre.len() < 3 || nombre.len() > 90 {
        return Err("El nombre debe tener entre 3 y 90 caracteres.".into());
    }

    let sku = payload.sku.trim();
    if sku.is_empty() {
        return Err("El SKU es obligatorio.".into());
    }
    if sku.len() != 6 {
        return Err("El SKU debe tener exactamente 6 caracteres.".into());
    }
    let chars: Vec<char> = sku.chars().collect();
    if !sku.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err("El SKU solo puede contener letras mayúsculas y números.".into());
    }
    let has_3_upper = chars[0..3].iter().all(|c| c.is_ascii_uppercase());
    let has_3_digits = chars[3..6].iter().all(|c| c.is_ascii_digit());
    if !has_3_upper || !has_3_digits {
        return Err("El SKU debe tener 3 letras mayúsculas seguidas de 3 números.".into());
    }

    if payload.precio == 0.0 {
        return Err("El precio debe ser mayor a 0.".into());
    }
    if payload.precio < 0.0 {
        return Err("El precio no puede ser negativo.".into());
    }
    if payload.precio > 10000.0 {
        return Err("El precio no puede superar 10000.".into());
    }
    if !precio_valido(payload.precio) {
        return Err("El precio solo puede tener hasta dos decimales.".into());
    }

    if payload.stock_minimo.is_none() {
        return Err("El stock mínimo es obligatorio.".into());
    }
    let minimo = payload.stock_minimo.unwrap();
    if minimo <= 0.0 {
        return Err("El stock mínimo debe ser mayor a 0.".into());
    }
    if minimo.fract().abs() > 1e-6 {
        return Err("El stock mínimo debe ser un número entero.".into());
    }
    if minimo > 10000.0 {
        return Err("El stock mínimo no puede superar 10000.".into());
    }

    let desc = payload.descripcion.as_deref().unwrap_or("").trim();
    if desc.is_empty() {
        return Err("La descripción del producto es obligatoria.".into());
    }
    if desc.len() < 20 || desc.len() > 250 {
        return Err("La descripción debe tener entre 20 y 250 caracteres.".into());
    }

    if let Some(ci) = payload.cantidad_inicial {
        if ci <= 0.0 {
            return Err("La cantidad debe ser mayor a 0.".into());
        }
        if ci > 10000.0 {
            return Err("La cantidad no puede superar 10000.".into());
        }
        if !precio_valido(ci) {
            return Err("La cantidad debe ser un número con hasta dos decimales.".into());
        }
    }

    Ok(())
}

/// Validaciones de negocio para la actualización de un ítem (HU023). Antes Finca no
/// validaba nada; ahora POS y Finca comparten esta batería.
pub fn validate_update_item(payload: &UpdateInventarioItem) -> Result<(), String> {
    if let Some(ref nombre) = payload.nombre {
        let n = nombre.trim();
        if n.is_empty() {
            return Err("El nombre del producto es obligatorio.".into());
        }
        if n.len() < 3 || n.len() > 90 {
            return Err("El nombre debe tener entre 3 y 90 caracteres.".into());
        }
    }

    if let Some(ref desc) = payload.descripcion {
        let d = desc.trim();
        if d.is_empty() {
            return Err("La descripción del producto es obligatoria.".into());
        }
        if d.len() < 20 || d.len() > 250 {
            return Err("La descripción debe tener entre 20 y 250 caracteres.".into());
        }
    }

    if let Some(precio) = payload.precio {
        if precio == 0.0 {
            return Err("El precio debe ser mayor a 0.".into());
        }
        if precio < 0.0 {
            return Err("El precio no puede ser negativo.".into());
        }
        if precio > 10000.0 {
            return Err("El precio no puede superar 10000.".into());
        }
        if !precio_valido(precio) {
            return Err("El precio solo puede tener hasta dos decimales.".into());
        }
    }

    if let Some(minimo) = payload.stock_minimo {
        if minimo <= 0.0 {
            return Err("El stock mínimo debe ser mayor a 0.".into());
        }
        if minimo.fract().abs() > 1e-6 {
            return Err("El stock mínimo debe ser un número entero.".into());
        }
        if minimo > 10000.0 {
            return Err("El stock mínimo no puede superar 10000.".into());
        }
    }

    Ok(())
}

/// Reglas de transición de estado manual (HU025). Devuelve `Err(mensaje)` si la
/// transición es inviable según la matemática del inventario. Los estados
/// operativos (INACTIVO/EN_TRANSITO/BLOQUEADO) son de libre asignación manual.
pub fn validar_transicion_estado(nuevo: &EstadoInventario, item: &InventarioItem) -> Result<(), String> {
    let caducado = item
        .fecha_caducidad
        .map(|f| f <= Utc::now())
        .unwrap_or(false);

    match nuevo {
        EstadoInventario::DISPONIBLE => {
            if item.cantidad <= 0.0 {
                return Err("No se puede marcar como DISPONIBLE un producto sin stock.".into());
            }
            if item.cantidad <= item.stock_minimo {
                return Err(
                    "No se puede marcar como DISPONIBLE: el stock está en o por debajo del mínimo.".into(),
                );
            }
            if caducado {
                return Err("No se puede marcar como DISPONIBLE un producto caducado.".into());
            }
            Ok(())
        }
        EstadoInventario::AGOTADO => {
            if item.cantidad != 0.0 {
                return Err("Solo se puede marcar AGOTADO cuando la cantidad es 0.".into());
            }
            Ok(())
        }
        EstadoInventario::STOCK_BAJO => {
            if !(item.cantidad > 0.0 && item.cantidad <= item.stock_minimo) {
                return Err(
                    "STOCK_BAJO requiere cantidad mayor a 0 y menor o igual al stock mínimo.".into(),
                );
            }
            Ok(())
        }
        EstadoInventario::CADUCADO => {
            if !caducado {
                return Err(
                    "Solo se puede marcar CADUCADO si la fecha de caducidad ya venció.".into(),
                );
            }
            Ok(())
        }
        // Estados operativos: asignación manual libre.
        EstadoInventario::INACTIVO
        | EstadoInventario::EN_TRANSITO
        | EstadoInventario::BLOQUEADO => Ok(()),
    }
}
