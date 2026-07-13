-- migrations/20260714000000_create_report_tables.sql
-- ============================================
-- TABLAS PARA REPORTES DEL SISTEMA STGC
-- ============================================

-- ============================================
-- 1. TABLA DE INVENTARIO
-- ============================================
CREATE TABLE IF NOT EXISTS inventario_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sku VARCHAR(50) UNIQUE NOT NULL,
    nombre VARCHAR(100) NOT NULL,
    tipo VARCHAR(50) NOT NULL,
    cantidad DOUBLE PRECISION DEFAULT 0,
    stock_minimo DOUBLE PRECISION DEFAULT 0,
    unidad_medida VARCHAR(20) NOT NULL,
    estado VARCHAR(20) DEFAULT 'ACTIVO',
    is_deleted BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- 2. TABLA DE MOVIMIENTOS DE STOCK
-- ============================================
CREATE TABLE IF NOT EXISTS movimientos_stock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id UUID REFERENCES inventario_items(id) ON DELETE CASCADE,
    tipo VARCHAR(50) NOT NULL,
    cantidad DOUBLE PRECISION NOT NULL,
    motivo TEXT,
    fecha TIMESTAMPTZ DEFAULT NOW()  -- <--- TIMESTAMPTZ
);

-- ============================================
-- 3. TABLA DE USUARIOS
-- ============================================
CREATE TABLE IF NOT EXISTS usuarios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nombre VARCHAR(100) NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    rol VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- 4. TABLA DE PRODUCTOS
-- ============================================
CREATE TABLE IF NOT EXISTS productos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nombre VARCHAR(100) NOT NULL,
    precio DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- 5. TABLA DE VENTAS
-- ============================================
CREATE TABLE IF NOT EXISTS ventas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    usuario_id UUID REFERENCES usuarios(id) ON DELETE CASCADE,
    total DOUBLE PRECISION NOT NULL,
    fecha TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- 6. TABLA DE DETALLE DE VENTAS
-- ============================================
CREATE TABLE IF NOT EXISTS detalle_ventas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    venta_id UUID REFERENCES ventas(id) ON DELETE CASCADE,
    producto_id UUID REFERENCES productos(id) ON DELETE CASCADE,
    cantidad DOUBLE PRECISION NOT NULL,
    precio_unitario DOUBLE PRECISION NOT NULL,
    subtotal DOUBLE PRECISION NOT NULL
);

-- ============================================
-- ÍNDICES PARA MEJORAR RENDIMIENTO
-- ============================================
CREATE INDEX IF NOT EXISTS idx_inventario_items_estado ON inventario_items(estado);
CREATE INDEX IF NOT EXISTS idx_inventario_items_tipo ON inventario_items(tipo);
CREATE INDEX IF NOT EXISTS idx_inventario_items_nombre ON inventario_items(nombre);
CREATE INDEX IF NOT EXISTS idx_movimientos_stock_item_id ON movimientos_stock(item_id);
CREATE INDEX IF NOT EXISTS idx_movimientos_stock_fecha ON movimientos_stock(fecha);
CREATE INDEX IF NOT EXISTS idx_ventas_usuario_id ON ventas(usuario_id);
CREATE INDEX IF NOT EXISTS idx_ventas_fecha ON ventas(fecha);
CREATE INDEX IF NOT EXISTS idx_detalle_ventas_venta_id ON detalle_ventas(venta_id);
CREATE INDEX IF NOT EXISTS idx_detalle_ventas_producto_id ON detalle_ventas(producto_id);
CREATE INDEX IF NOT EXISTS idx_usuarios_email ON usuarios(email);
CREATE INDEX IF NOT EXISTS idx_usuarios_rol ON usuarios(rol);

-- ============================================
-- DATOS DE PRUEBA
-- ============================================

-- Insertar usuarios de prueba
INSERT INTO usuarios (id, nombre, email, rol) VALUES 
    (gen_random_uuid(), 'Admin Test', 'admin@test.com', 'ADMIN'),
    (gen_random_uuid(), 'Cajero Test', 'cajero@test.com', 'CAJERO'),
    (gen_random_uuid(), 'Gerente Test', 'gerente@test.com', 'GERENTE'),
    (gen_random_uuid(), 'Inventario Test', 'inventario@test.com', 'INVENTARIO'),
    (gen_random_uuid(), 'Cocina Test', 'cocina@test.com', 'COCINA')
ON CONFLICT (email) DO NOTHING;

-- Insertar productos de prueba
INSERT INTO productos (id, nombre, precio) VALUES 
    (gen_random_uuid(), 'Café Americano', 3.50),
    (gen_random_uuid(), 'Café Latte', 4.00),
    (gen_random_uuid(), 'Café Mocha', 4.50),
    (gen_random_uuid(), 'Capuchino', 4.00),
    (gen_random_uuid(), 'Espresso', 2.50),
    (gen_random_uuid(), 'Té Verde', 3.00),
    (gen_random_uuid(), 'Té Negro', 3.00),
    (gen_random_uuid(), 'Sandwich de Jamón', 5.00),
    (gen_random_uuid(), 'Sandwich de Pollo', 5.50),
    (gen_random_uuid(), 'Croissant', 2.50),
    (gen_random_uuid(), 'Jugo Natural', 4.00),
    (gen_random_uuid(), 'Ensalada', 6.00)
ON CONFLICT DO NOTHING;

-- Insertar inventario de prueba
INSERT INTO inventario_items (id, sku, nombre, tipo, cantidad, stock_minimo, unidad_medida, estado) VALUES 
    (gen_random_uuid(), 'CAFE-001', 'Café Americano', 'PRODUCTO', 100, 10, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'CAFE-002', 'Café Latte', 'PRODUCTO', 80, 10, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'CAFE-003', 'Café Mocha', 'PRODUCTO', 60, 10, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'CAFE-004', 'Capuchino', 'PRODUCTO', 75, 10, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'CAFE-005', 'Espresso', 'PRODUCTO', 120, 15, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'TEA-001', 'Té Verde', 'PRODUCTO', 50, 5, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'TEA-002', 'Té Negro', 'PRODUCTO', 45, 5, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'FOOD-001', 'Sandwich de Jamón', 'PRODUCTO', 30, 5, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'FOOD-002', 'Sandwich de Pollo', 'PRODUCTO', 25, 5, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'FOOD-003', 'Croissant', 'PRODUCTO', 40, 8, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'DRINK-001', 'Jugo Natural', 'PRODUCTO', 35, 5, 'UNIDAD', 'ACTIVO'),
    (gen_random_uuid(), 'FOOD-004', 'Ensalada', 'PRODUCTO', 20, 3, 'UNIDAD', 'ACTIVO')
ON CONFLICT (sku) DO NOTHING;

-- Insertar movimientos de stock de prueba
INSERT INTO movimientos_stock (id, item_id, tipo, cantidad, motivo, fecha) 
SELECT 
    gen_random_uuid(),
    id,
    'ENTRADA',
    50,
    'Compra inicial',
    NOW() - INTERVAL '7 days'
FROM inventario_items
WHERE cantidad > 50
LIMIT 5;

INSERT INTO movimientos_stock (id, item_id, tipo, cantidad, motivo, fecha) 
SELECT 
    gen_random_uuid(),
    id,
    'ENTRADA',
    20,
    'Reabastecimiento',
    NOW() - INTERVAL '3 days'
FROM inventario_items
WHERE cantidad > 30
LIMIT 3;

INSERT INTO movimientos_stock (id, item_id, tipo, cantidad, motivo, fecha) 
SELECT 
    gen_random_uuid(),
    id,
    'SALIDA',
    10,
    'Venta',
    NOW() - INTERVAL '2 days'
FROM inventario_items
LIMIT 3;

INSERT INTO movimientos_stock (id, item_id, tipo, cantidad, motivo, fecha) 
SELECT 
    gen_random_uuid(),
    id,
    'SALIDA',
    5,
    'Consumo interno',
    NOW() - INTERVAL '1 day'
FROM inventario_items
LIMIT 2;

-- Insertar ventas de prueba
DO $$
DECLARE
    v_usuario_id UUID;
    v_producto_id UUID;
    v_venta_id UUID;
    v_precio DOUBLE PRECISION;
    v_cantidad DOUBLE PRECISION;
    v_subtotal DOUBLE PRECISION;
    v_total DOUBLE PRECISION;
    v_fecha TIMESTAMPTZ;
BEGIN
    -- Obtener un usuario para las ventas
    SELECT id INTO v_usuario_id FROM usuarios WHERE rol = 'CAJERO' LIMIT 1;
    IF v_usuario_id IS NULL THEN
        SELECT id INTO v_usuario_id FROM usuarios LIMIT 1;
    END IF;
    
    -- Crear 8 ventas de prueba
    FOR i IN 1..8 LOOP
        v_venta_id := gen_random_uuid();
        v_fecha := NOW() - (INTERVAL '1 day' * (9 - i));
        v_total := 0;
        
        INSERT INTO ventas (id, usuario_id, total, fecha) 
        VALUES (v_venta_id, v_usuario_id, 0, v_fecha);
        
        FOR j IN 1..(2 + (i % 3)) LOOP
            SELECT id, precio INTO v_producto_id, v_precio 
            FROM productos 
            ORDER BY random() 
            LIMIT 1;
            
            v_cantidad := (random() * 3 + 0.5)::INT;
            v_subtotal := v_precio * v_cantidad;
            v_total := v_total + v_subtotal;
            
            INSERT INTO detalle_ventas (id, venta_id, producto_id, cantidad, precio_unitario, subtotal)
            VALUES (gen_random_uuid(), v_venta_id, v_producto_id, v_cantidad, v_precio, v_subtotal);
        END LOOP;
        
        UPDATE ventas SET total = v_total WHERE id = v_venta_id;
    END LOOP;
END $$;