use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::caller;
use ic_cdk::query;
use ic_cdk::update;
use std::collections::HashMap;
use std::cell::RefCell;

// Types
pub type PropertyId = u64;
pub type UserId = String; // For now, use Principal as String

#[derive(CandidType, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Role {
    Admin,
    Manager,
    User,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct PropertyMetadata {
    pub location: String,
    pub description: String,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct Property {
    pub id: PropertyId,
    pub name: String,
    pub total_shares: u64,
    pub shares_available: u64,
    pub metadata: PropertyMetadata,
    pub status: PropertyStatus,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct Listing {
    pub property_id: PropertyId,
    pub seller: Principal,
    pub amount: u64,
    pub price_per_share: u64,
}

// Ensure PropertyStatus is defined at the top level
#[derive(CandidType, Deserialize, Clone, PartialEq)]
pub enum PropertyStatus {
    Active,
    Maintenance,
    Sold,
}

thread_local! {
    static PROPERTIES: RefCell<HashMap<PropertyId, Property>> = RefCell::new(HashMap::new());
    static OWNERSHIP: RefCell<HashMap<(PropertyId, Principal), u64>> = RefCell::new(HashMap::new());
    static NEXT_PROPERTY_ID: RefCell<PropertyId> = RefCell::new(1);
    static RENTAL_INCOME: RefCell<HashMap<PropertyId, u64>> = RefCell::new(HashMap::new()); // total deposited
    static UNCLAIMED_INCOME: RefCell<HashMap<(PropertyId, Principal), u64>> = RefCell::new(HashMap::new()); // per user
    static MARKETPLACE: RefCell<Vec<Listing>> = RefCell::new(Vec::new());
    static ADMINS: RefCell<Vec<Principal>> = RefCell::new(vec![Principal::anonymous()]);
    static ROLES: RefCell<HashMap<Principal, Role>> = RefCell::new(HashMap::new());
    static KYC: RefCell<HashMap<Principal, bool>> = RefCell::new(HashMap::new());
    static BOOTSTRAPPED: RefCell<bool> = RefCell::new(false);
}

fn get_role(principal: &Principal) -> Role {
    ROLES.with(|roles| roles.borrow().get(principal).cloned().unwrap_or(Role::User))
}

fn is_kyc_verified(principal: &Principal) -> bool {
    KYC.with(|kyc| kyc.borrow().get(principal).cloned().unwrap_or(false))
}

#[update]
pub fn set_kyc_status(user: Principal, status: bool) -> Result<String, String> {
    let caller_principal = caller();
    if get_role(&caller_principal) != Role::Admin {
        return Err("Only admin can set KYC status".to_string());
    }
    KYC.with(|kyc| {
        kyc.borrow_mut().insert(user, status);
    });
    Ok("KYC status updated".to_string())
}

#[update]
pub fn set_role(user: Principal, role: Role) -> Result<String, String> {
    let caller_principal = caller();
    if get_role(&caller_principal) != Role::Admin {
        return Err("Only admin can set roles".to_string());
    }
    ROLES.with(|roles| {
        roles.borrow_mut().insert(user, role);
    });
    Ok("Role updated".to_string())
}

#[update]
pub fn bootstrap_admin(admin: Principal) -> Result<String, String> {
    let already_bootstrapped = BOOTSTRAPPED.with(|b| *b.borrow());
    if already_bootstrapped {
        return Err("Admin already bootstrapped".to_string());
    }
    ROLES.with(|roles| {
        roles.borrow_mut().insert(admin, Role::Admin);
    });
    BOOTSTRAPPED.with(|b| *b.borrow_mut() = true);
    Ok("Admin bootstrapped".to_string())
}

#[query]
pub fn get_my_role() -> Role {
    get_role(&caller())
}

#[query]
pub fn is_my_kyc_verified() -> bool {
    is_kyc_verified(&caller())
}

#[update]
pub fn update_property_metadata(property_id: PropertyId, metadata: PropertyMetadata, caller: Principal) -> Result<String, String> {
    if get_role(&caller) != Role::Admin {
        return Err("Only admin can update property metadata".to_string());
    }
    PROPERTIES.with(|props| {
        let mut props = props.borrow_mut();
        if let Some(prop) = props.get_mut(&property_id) {
            prop.metadata = metadata;
            Ok("Property metadata updated".to_string())
        } else {
            Err("Property not found".to_string())
        }
    })
}

#[update]
pub fn update_property_status(property_id: PropertyId, status: PropertyStatus, caller: Principal) -> Result<String, String> {
    if get_role(&caller) != Role::Admin {
        return Err("Only admin can update property status".to_string());
    }
    PROPERTIES.with(|props| {
        let mut props = props.borrow_mut();
        if let Some(prop) = props.get_mut(&property_id) {
            prop.status = status;
            Ok("Property status updated".to_string())
        } else {
            Err("Property not found".to_string())
        }
    })
}

// Update register_property to include metadata and status
#[update]
pub fn register_property(name: String, total_shares: u64, metadata: PropertyMetadata) -> Property {
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
            metadata,
            status: PropertyStatus::Active,
        };
        props.insert(id, property.clone());
        property
    });
    property
}

#[update]
pub fn issue_shares(property_id: PropertyId, to: Principal, amount: u64) -> Result<String, String> {
    // Check property exists and has enough shares
    let mut success = false;
    PROPERTIES.with(|props| {
        let mut props = props.borrow_mut();
        if let Some(prop) = props.get_mut(&property_id) {
            if prop.shares_available >= amount {
                prop.shares_available -= amount;
                OWNERSHIP.with(|own| {
                    let mut own = own.borrow_mut();
                    *own.entry((property_id, to)).or_insert(0) += amount;
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
pub fn get_ownership(property_id: PropertyId, user: Principal) -> u64 {
    OWNERSHIP.with(|own| own.borrow().get(&(property_id, user)).cloned().unwrap_or(0))
}

/// Admin deposits rental income for a property. Distributes to all current owners proportionally.
#[update]
pub fn deposit_rental_income(property_id: PropertyId, amount: u64) -> Result<String, String> {
    // Track total income
    RENTAL_INCOME.with(|ri| {
        let mut ri = ri.borrow_mut();
        *ri.entry(property_id).or_insert(0) += amount;
    });
    // Distribute to owners
    let mut total_shares = 0;
    PROPERTIES.with(|props| {
        if let Some(prop) = props.borrow().get(&property_id) {
            total_shares = prop.total_shares;
        }
    });
    if total_shares == 0 {
        return Err("Property not found or has no shares".to_string());
    }
    // Find all owners
    OWNERSHIP.with(|own| {
        let own = own.borrow();
        for ((pid, user), shares) in own.iter() {
            if *pid == property_id && *shares > 0 {
                let user_income = amount * shares / total_shares;
                UNCLAIMED_INCOME.with(|ui| {
                    let mut ui = ui.borrow_mut();
                    *ui.entry((property_id, user.clone())).or_insert(0) += user_income;
                });
            }
        }
    });
    Ok("Rental income distributed".to_string())
}

/// User claims their unclaimed rental income for a property.
#[update]
pub fn claim_income(property_id: PropertyId, user: Principal) -> u64 {
    let mut claimed = 0;
    UNCLAIMED_INCOME.with(|ui| {
        let mut ui = ui.borrow_mut();
        claimed = ui.remove(&(property_id, user)).unwrap_or(0);
    });
    claimed
}

/// Query unclaimed rental income for a user and property.
#[query]
pub fn get_unclaimed_income(property_id: PropertyId, user: Principal) -> u64 {
    UNCLAIMED_INCOME.with(|ui| ui.borrow().get(&(property_id, user)).cloned().unwrap_or(0))
}

/// List shares for sale on the marketplace
#[update]
pub fn list_shares_for_sale(property_id: PropertyId, seller: Principal, amount: u64, price_per_share: u64) -> Result<String, String> {
    // Check seller owns enough shares
    let owned = OWNERSHIP.with(|own| own.borrow().get(&(property_id, seller)).cloned().unwrap_or(0));
    if owned < amount {
        return Err("Not enough shares to list".to_string());
    }
    // Add listing
    MARKETPLACE.with(|mp| {
        mp.borrow_mut().push(Listing {
            property_id,
            seller,
            amount,
            price_per_share,
        });
    });
    Ok("Shares listed for sale".to_string())
}

/// Buy shares from the marketplace
#[update]
pub fn buy_shares(property_id: PropertyId, seller: Principal, buyer: Principal, amount: u64) -> Result<String, String> {
    let mut found = false;
    MARKETPLACE.with(|mp| {
        let mut mp = mp.borrow_mut();
        if let Some(pos) = mp.iter().position(|l| l.property_id == property_id && l.seller == seller && l.amount >= amount) {
            let price_per_share = mp[pos].price_per_share;
            // Transfer shares
            OWNERSHIP.with(|own| {
                let mut own = own.borrow_mut();
                // Remove from seller
                let seller_shares = own.entry((property_id, seller)).or_insert(0);
                if *seller_shares < amount {
                    return;
                }
                *seller_shares -= amount;
                // Add to buyer
                *own.entry((property_id, buyer)).or_insert(0) += amount;
            });
            // Reduce or remove listing
            if mp[pos].amount == amount {
                mp.remove(pos);
            } else {
                mp[pos].amount -= amount;
            }
            found = true;
        }
    });
    if found {
        Ok("Shares bought successfully".to_string())
    } else {
        Err("Listing not found or insufficient shares".to_string())
    }
}

/// Transfer shares directly between users
#[update]
pub fn transfer_shares(property_id: PropertyId, from: Principal, to: Principal, amount: u64) -> Result<String, String> {
    OWNERSHIP.with(|own| {
        let mut own = own.borrow_mut();
        let from_shares = own.entry((property_id, from)).or_insert(0);
        if *from_shares < amount {
            return Err("Not enough shares to transfer".to_string());
        }
        *from_shares -= amount;
        *own.entry((property_id, to)).or_insert(0) += amount;
        Ok("Shares transferred".to_string())
    })
}

/// Get all marketplace listings
#[query]
pub fn get_marketplace_listings() -> Vec<Listing> {
    MARKETPLACE.with(|mp| mp.borrow().clone())
}
