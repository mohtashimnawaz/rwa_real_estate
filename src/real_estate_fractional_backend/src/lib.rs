use candid::{CandidType, Deserialize};
use ic_cdk::query;
use ic_cdk::update;
use std::collections::HashMap;
use std::cell::RefCell;

// Types
pub type PropertyId = u64;
pub type UserId = String; // For now, use Principal as String

#[derive(CandidType, Deserialize, Clone)]
pub struct Property {
    pub id: PropertyId,
    pub name: String,
    pub total_shares: u64,
    pub shares_available: u64,
}

thread_local! {
    static PROPERTIES: RefCell<HashMap<PropertyId, Property>> = RefCell::new(HashMap::new());
    static OWNERSHIP: RefCell<HashMap<(PropertyId, UserId), u64>> = RefCell::new(HashMap::new());
    static NEXT_PROPERTY_ID: RefCell<PropertyId> = RefCell::new(1);
}

#[update]
pub fn register_property(name: String, total_shares: u64) -> Property {
    let property = PROPERTIES.with(|props| {
        let mut props = props.borrow_mut();
        let id = NEXT_PROPERTY_ID.with(|id| {
            let mut id = id.borrow_mut();
            let curr = *id;
            *id += 1;
            curr
        });
        let property = Property {
            id,
            name,
            total_shares,
            shares_available: total_shares,
        };
        props.insert(id, property.clone());
        property
    });
    property
}

#[update]
pub fn issue_shares(property_id: PropertyId, to: UserId, amount: u64) -> Result<String, String> {
    // Check property exists and has enough shares
    let mut success = false;
    PROPERTIES.with(|props| {
        let mut props = props.borrow_mut();
        if let Some(prop) = props.get_mut(&property_id) {
            if prop.shares_available >= amount {
                prop.shares_available -= amount;
                OWNERSHIP.with(|own| {
                    let mut own = own.borrow_mut();
                    *own.entry((property_id, to.clone())).or_insert(0) += amount;
                });
                success = true;
            }
        }
    });
    if success {
        Ok("Shares issued".to_string())
    } else {
        Err("Not enough shares or property not found".to_string())
    }
}

#[query]
pub fn get_property(property_id: PropertyId) -> Option<Property> {
    PROPERTIES.with(|props| props.borrow().get(&property_id).cloned())
}

#[query]
pub fn get_ownership(property_id: PropertyId, user: UserId) -> u64 {
    OWNERSHIP.with(|own| own.borrow().get(&(property_id, user)).cloned().unwrap_or(0))
}
