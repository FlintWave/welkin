// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::inventory_rez::{DeRezAction, save_group_to_inventory};
use uuid::Uuid;
use welkin_data::inventory::InventoryItemRow;

pub fn check_sale(
    root_sale_type: u8,
    root_price: i32,
    requested_sale_type: u8,
    requested_price: i32,
) -> Result<DeRezAction, &'static str> {
    if root_sale_type == 0 {
        return Err("not for sale");
    }
    if root_sale_type != requested_sale_type {
        return Err("sale type changed");
    }
    if requested_price != root_price {
        return Err("price changed");
    }
    match root_sale_type {
        1 => Ok(DeRezAction::Take),
        2 => Ok(DeRezAction::TakeCopy),
        _ => Err("sale type not supported"),
    }
}

#[derive(Debug)]
pub struct BuyResult {
    pub items: Vec<InventoryItemRow>,
    pub removed_local_ids: Vec<u32>,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct BuyEntry {
    pub local_id: u32,
    pub sale_type: u8,
    pub sale_price: i32,
}

pub async fn buy_objects(
    scene: &welkin_scene::scene::Scene,
    inventory: &welkin_services::remote::inventory::RemoteInventoryConnector,
    assets: &welkin_services::remote::asset::RemoteAssetConnector,
    buyer: Uuid,
    folder_id: Uuid,
    entries: &[BuyEntry],
) -> BuyResult {
    let mut result = BuyResult {
        items: Vec::new(),
        removed_local_ids: Vec::new(),
        errors: Vec::new(),
    };

    for entry in entries.iter().take(32) {
        let uuid = match scene.get_object_by_local_id(entry.local_id) {
            Some(uuid) => uuid,
            None => {
                result.errors.push("unknown object".to_string());
                continue;
            }
        };

        let (root_sale_type, root_price, _owner) = {
            let guard = match scene.get_object(&uuid) {
                Some(guard) => guard,
                None => {
                    result.errors.push("unknown object".to_string());
                    continue;
                }
            };
            let root = guard.root_part();
            (root.object_sale_type, root.sale_price, root.owner_id)
        };

        // For an original sale, buying one's own object is acceptable.
        let _ = _owner;
        let _ = buyer;

        let action = match check_sale(
            root_sale_type,
            root_price,
            entry.sale_type,
            entry.sale_price,
        ) {
            Ok(action) => action,
            Err(error) => {
                result.errors.push(error.to_string());
                continue;
            }
        };

        let group = {
            let guard = match scene.get_object(&uuid) {
                Some(guard) => guard,
                None => {
                    result.errors.push("unknown object".to_string());
                    continue;
                }
            };
            guard
        };

        let is_take = matches!(action, DeRezAction::Take);

        match save_group_to_inventory(inventory, assets, action, buyer, folder_id, &group).await {
            Ok(item) => {
                result.items.push(item);
                if is_take && scene.remove_object(&uuid).is_some() {
                    result.removed_local_ids.push(entry.local_id);
                }
            }
            Err(error) => {
                result.errors.push(error);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_for_sale() {
        assert!(matches!(check_sale(0, 5, 0, 5), Err("not for sale")));
    }

    #[test]
    fn type_changed() {
        assert!(matches!(check_sale(1, 5, 2, 5), Err("sale type changed")));
    }

    #[test]
    fn price_changed() {
        assert!(matches!(check_sale(1, 5, 1, 7), Err("price changed")));
    }

    #[test]
    fn original_is_take() {
        assert!(matches!(check_sale(1, 5, 1, 5), Ok(DeRezAction::Take)));
    }

    #[test]
    fn copy_is_take_copy() {
        assert!(matches!(check_sale(2, 5, 2, 5), Ok(DeRezAction::TakeCopy)));
    }

    #[test]
    fn contents_unsupported() {
        assert!(matches!(
            check_sale(3, 5, 3, 5),
            Err("sale type not supported")
        ));
    }

    #[test]
    fn zero_price_copy_ok() {
        assert!(matches!(check_sale(2, 0, 2, 0), Ok(DeRezAction::TakeCopy)));
    }
}
