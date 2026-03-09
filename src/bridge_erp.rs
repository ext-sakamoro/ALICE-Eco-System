//! ERP bridges — ALICE-ERP ↔ DB, Cache, Analytics, Notify, API
//!
//! 5 bridges connecting enterprise resource planning to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: ERP → DB (inventory persistence) ──────────────────────────

/// Inventory snapshot record for ALICE-DB persistence.
pub struct ErpDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Number of distinct SKUs in the inventory.
    pub sku_count: u32,
    /// Total inventory value in minor currency units (e.g. cents).
    pub inventory_value: u64,
    /// Maximum depth of the bill-of-materials tree.
    pub bom_depth: u8,
    /// Number of open work orders.
    pub work_order_count: u32,
    /// Warehouse location ID this snapshot covers.
    pub warehouse_id: u32,
}

/// Convert an ERP inventory snapshot into an ALICE-DB record.
#[inline]
#[must_use]
pub fn erp_to_db_record(
    sku_count: u32,
    inventory_value: u64,
    bom_depth: u8,
    work_order_count: u32,
    warehouse_id: u32,
) -> ErpDbRecord {
    let mut data = [0u8; 17];
    data[0..4].copy_from_slice(&sku_count.to_le_bytes());
    data[4..12].copy_from_slice(&inventory_value.to_le_bytes());
    data[12] = bom_depth;
    data[13..17].copy_from_slice(&work_order_count.to_le_bytes());
    ErpDbRecord {
        content_hash: fnv1a(&data),
        sku_count,
        inventory_value,
        bom_depth,
        work_order_count,
        warehouse_id,
    }
}

// ── Bridge 2: ERP → Cache (BOM cache) ───────────────────────────────────

/// Bill-of-materials cache entry for ALICE-Cache.
pub struct ErpCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Root item identifier of the BOM tree.
    pub root_item_id: u64,
    /// Number of components in the flattened BOM.
    pub component_count: u32,
    /// Maximum BOM depth.
    pub bom_depth: u8,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an ALICE-Cache entry from an ERP BOM snapshot.
#[inline]
#[must_use]
pub fn erp_to_cache_entry(
    root_item_id: u64,
    component_count: u32,
    bom_depth: u8,
    ttl_secs: u32,
) -> ErpCacheEntry {
    let mut data = [0u8; 13];
    data[0..8].copy_from_slice(&root_item_id.to_le_bytes());
    data[8..12].copy_from_slice(&component_count.to_le_bytes());
    data[12] = bom_depth;
    ErpCacheEntry {
        content_hash: fnv1a(&data),
        root_item_id,
        component_count,
        bom_depth,
        ttl_secs,
    }
}

// ── Bridge 3: ERP → Analytics (production metrics) ──────────────────────

/// Production metrics event for ALICE-Analytics ingestion.
pub struct ErpAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Units produced in the observation window.
    pub units_produced: u64,
    /// Units scrapped (defect/reject) in the same window.
    pub units_scrapped: u32,
    /// Production rate in units per hour (fixed-point ×100).
    pub production_rate_x100: u32,
    /// Overall equipment effectiveness in basis points (0–10000).
    pub oee_bps: u16,
    /// Observation window duration in seconds.
    pub window_secs: u32,
}

/// Convert ERP production data into an ALICE-Analytics event.
#[inline]
#[must_use]
pub fn erp_to_analytics_event(
    units_produced: u64,
    units_scrapped: u32,
    production_rate_x100: u32,
    oee_bps: u16,
    window_secs: u32,
) -> ErpAnalyticsEvent {
    let mut data = [0u8; 18];
    data[0..8].copy_from_slice(&units_produced.to_le_bytes());
    data[8..12].copy_from_slice(&units_scrapped.to_le_bytes());
    data[12..16].copy_from_slice(&production_rate_x100.to_le_bytes());
    data[16..18].copy_from_slice(&oee_bps.to_le_bytes());
    ErpAnalyticsEvent {
        content_hash: fnv1a(&data),
        units_produced,
        units_scrapped,
        production_rate_x100,
        oee_bps,
        window_secs,
    }
}

// ── Bridge 4: ERP → Notify (reorder alerts) ─────────────────────────────

/// Reorder alert payload for ALICE-Notify delivery.
pub struct ErpNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// SKU identifier triggering the alert.
    pub sku_id: u64,
    /// Current stock quantity on hand.
    pub qty_on_hand: u32,
    /// Configured reorder point threshold.
    pub reorder_point: u32,
    /// Suggested reorder quantity.
    pub reorder_qty: u32,
    /// Alert severity level (0 = info, 1 = warning, 2 = critical).
    pub severity: u8,
}

/// Build an ALICE-Notify reorder alert from ERP stock data.
#[inline]
#[must_use]
pub fn erp_to_notify_alert(
    sku_id: u64,
    qty_on_hand: u32,
    reorder_point: u32,
    reorder_qty: u32,
    severity: u8,
) -> ErpNotifyAlert {
    let mut data = [0u8; 21];
    data[0..8].copy_from_slice(&sku_id.to_le_bytes());
    data[8..12].copy_from_slice(&qty_on_hand.to_le_bytes());
    data[12..16].copy_from_slice(&reorder_point.to_le_bytes());
    data[16..20].copy_from_slice(&reorder_qty.to_le_bytes());
    data[20] = severity;
    ErpNotifyAlert {
        content_hash: fnv1a(&data),
        sku_id,
        qty_on_hand,
        reorder_point,
        reorder_qty,
        severity,
    }
}

// ── Bridge 5: ERP → API (integration) ───────────────────────────────────

/// API integration payload exposing ERP state to external consumers.
pub struct ErpApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Tenant / organisation identifier.
    pub tenant_id: u32,
    /// Number of active purchase orders.
    pub active_po_count: u32,
    /// Number of active sales orders.
    pub active_so_count: u32,
    /// Total open-order value in minor currency units.
    pub open_order_value: u64,
    /// Schema version of this payload.
    pub schema_version: u16,
}

/// Compose an ALICE-API payload from ERP order state.
#[inline]
#[must_use]
pub fn erp_to_api_payload(
    tenant_id: u32,
    active_po_count: u32,
    active_so_count: u32,
    open_order_value: u64,
    schema_version: u16,
) -> ErpApiPayload {
    let mut data = [0u8; 20];
    data[0..4].copy_from_slice(&tenant_id.to_le_bytes());
    data[4..8].copy_from_slice(&active_po_count.to_le_bytes());
    data[8..12].copy_from_slice(&active_so_count.to_le_bytes());
    data[12..20].copy_from_slice(&open_order_value.to_le_bytes());
    data[18..20].copy_from_slice(&schema_version.to_le_bytes());
    ErpApiPayload {
        content_hash: fnv1a(&data),
        tenant_id,
        active_po_count,
        active_so_count,
        open_order_value,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = erp_to_db_record(100, 500_000, 4, 20, 1);
        let b = erp_to_db_record(100, 500_000, 4, 20, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_hash_changes_on_sku_count() {
        let a = erp_to_db_record(100, 500_000, 4, 20, 1);
        let b = erp_to_db_record(101, 500_000, 4, 20, 1);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_fields_preserved() {
        let r = erp_to_db_record(42, 999_999, 7, 5, 3);
        assert_eq!(r.sku_count, 42);
        assert_eq!(r.inventory_value, 999_999);
        assert_eq!(r.bom_depth, 7);
        assert_eq!(r.work_order_count, 5);
        assert_eq!(r.warehouse_id, 3);
    }

    #[test]
    fn cache_entry_hash_is_deterministic() {
        let a = erp_to_cache_entry(77, 300, 5, 3600);
        let b = erp_to_cache_entry(77, 300, 5, 3600);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn cache_entry_hash_changes_on_component_count() {
        let a = erp_to_cache_entry(77, 300, 5, 3600);
        let b = erp_to_cache_entry(77, 301, 5, 3600);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn analytics_event_oee_range() {
        let ev = erp_to_analytics_event(1000, 10, 9500, 8500, 3600);
        assert!(ev.oee_bps <= 10_000);
        assert_eq!(ev.units_produced, 1000);
    }

    #[test]
    fn notify_alert_severity_preserved() {
        let alert = erp_to_notify_alert(9999, 5, 50, 200, 2);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.sku_id, 9999);
        assert!(alert.qty_on_hand < alert.reorder_point);
    }

    #[test]
    fn api_payload_hash_changes_on_value() {
        let a = erp_to_api_payload(1, 10, 20, 100_000, 1);
        let b = erp_to_api_payload(1, 10, 20, 100_001, 1);
        assert_ne!(a.content_hash, b.content_hash);
        assert_eq!(a.tenant_id, b.tenant_id);
    }
}
