use serde::{Deserialize, Serialize};


#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub phone_number: String,
    pub email: String,
}

