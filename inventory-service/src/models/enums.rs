use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "estado_producto", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum EstadoInventario {
    DISPONIBLE,
    AGOTADO,
    STOCK_BAJO,
    INACTIVO,
    EN_TRANSITO,
    BLOQUEADO,
    CADUCADO,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "tipo_elemento", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum TipoElemento {
    INSUMO,
    PRODUCTO,
    CAFE_PROCESADO,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "unidad_medida", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum UnidadMedida {
    QUINTALES,
    ARROBAS,
    LIBRAS,
    UNIDADES,
    LITROS,
    KILOGRAMOS,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "modulo_inventario", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum ModuloInventario {
    FINCA,
    CAFETERIA,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "calidad_cafe", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum CalidadCafe {
    ALTA,
    MEDIA,
    BAJA,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "clasificacion_insumo", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum ClasificacionInsumo {
    QUIMICO_FERTILIZANTE,
    QUIMICO_FUNGICIDA,
    ORGANICO,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "fase_cafe", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum FaseCafe {
    PULPA,
    DESPULPADO,
    SECADO,
    TOSTADO,
    MOLIDO,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "tipo_movimiento", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(non_camel_case_types)]
pub enum TipoMovimiento {
    ENTRADA,
    SALIDA,
}
