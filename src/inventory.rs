use crate::model::Item;
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Sum stacks from one explicit storage. Unknown inventory remains an error.
pub fn count(items: Option<&[Item]>, storage: u16, type_id: u16) -> Result<i64> {
    let items = items.ok_or_else(|| anyhow::anyhow!("Inventory unavailable"))?;
    let mut quantity = 0i64;
    for item in items {
        ensure!(item.quantity >= 0, "Invalid negative stack");
        if item.storage == storage && item.type_id == type_id {
            quantity = quantity
                .checked_add(item.quantity)
                .ok_or_else(|| anyhow::anyhow!("Inventory total overflow"))?;
        }
    }
    Ok(quantity)
}

pub fn gains(before: &[Item], after: &[Item], storage: u16) -> Result<BTreeMap<u16, i64>> {
    let mut result = BTreeMap::new();
    for item in after {
        if item.storage != storage || result.contains_key(&item.type_id) {
            continue;
        }
        let old = count(Some(before), storage, item.type_id)?;
        let new = count(Some(after), storage, item.type_id)?;
        if new > old {
            result.insert(item.type_id, new - old);
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_and_confirmation_do_not_mix_warehouse_or_unknown_data() {
        let old = vec![
            Item {
                storage: 1,
                type_id: 23,
                quantity: 4,
            },
            Item {
                storage: 4,
                type_id: 23,
                quantity: 50,
            },
        ];
        let mut new = old.clone();
        new.push(Item {
            storage: 1,
            type_id: 23,
            quantity: 2,
        });
        assert_eq!(count(Some(&new), 1, 23).unwrap(), 6);
        assert_eq!(gains(&old, &new, 1).unwrap(), BTreeMap::from([(23, 2)]));
        assert!(count(None, 1, 23).is_err());
    }
}
