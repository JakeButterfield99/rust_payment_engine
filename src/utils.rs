use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

// Parse a f32 value into the `rust_decimal` Decimal type with 4 decimal places
pub fn parse_amount(amount: Option<f32>) -> Decimal {
    match amount {
        Some(val) => {
            let decimal = Decimal::from_f32(val).unwrap_or(Decimal::new(0, 0));
            let rounded = decimal.round_dp(4);
            rounded
        }
        None => Decimal::new(0, 0),
    }
}

// Check if the balance can cover the amount
pub fn can_afford(balance: Decimal, amount: Decimal) -> bool {
    balance >= amount
}

// Encode a transaction into a unique key string
pub fn get_transaction_key(type_: &str, client: u16, tx: u32) -> String {
    format!("{}_{}_{}", type_, client, tx)
}
