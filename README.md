# Rust Payment Engine Challenge

## Intro

This is my submission for the Rust Payment Engine challenge. I wrote most of this manually with the assitance of AI inline-suggestions, with more indepth AI use detailed below and in the transaction logs.

I created many unit tests to cover potential transaction operations and error cases.

### Assumptions

I made some assumptions which weren't stricly defined in the brief. These could impact the expected behaviour of the system.

- Account Balance cannot drop below 0. In the case of a `chargeback` or `resolve` I ensure that the available or held funds can afford the reduction of funds.
- Once a chargeback has been processed, there's no coming back. Any further transactions including a `resolve` targeting the chargeback will not be processed.

### Trade-offs

I made some decisions on libraries, formatting & architecture which do contain some trade-offs which I'll detail below.

- Using **rust_decimal** `Decimal` type instead of the standard `f32`
    - This allows better precision when dealing with floating-point arithmetic which I deemed crucial for a financial application.
    - This comes at a cost of performance and readability/standards of a standard Rust application.
- Idempotency Keys
    - In order to account for the case of duplicate transactions, I implemented an idempotency key system. This encodes details of the transaction to ensure it doesn't appear twice.
    - This comes at the cost of performance and space.
- Duplicate User Data when exporting
    - When exporting it is exported as it's own `UserExport` type.
    - This does result in some data duplication for the lifetime of the exporting procedure.
    - Benefits
        - Internal `UserAccount` type and data will remain private to the class.
        - Real data types are exported for potential use elsewhere in application, rather than exporting pre-serialized data.
- Using getters & setters in internal functions
    - This doesn't necessarily have any performance impact but adds extra 'unnecessary' code.
    - Allows calls for dispute checks, editing user funds, etc. to have extra logic applied relatively easily compared to interacting with the data directly in the handlers.
    - These calls could also be implemented in other ways such as interfacing with customer service, test accounts etc.

## Testing

### Test Data

I have included 2 data sets which I used for testing:

- `transactions.csv`: Manually written test data to address each case
- `ai_test_data.csv`: Partially-AI generated test data
    - AI seems to have trouble reliably creating accurate test data, as seen in my transcripts.
    - I managed to get accurate `deposit` and `withdrawal` transaction data which I then manually added some dispute, resolve, chargeback cases to for testing.

### Unit Tests

Run tests: `cargo test`

**Coverage:**

- Basic deposits & withdrawals (including edge cases)
- Withdrawal failure when insufficient funds
- Dispute & resolve flow
- Dispute & chargeback flow
- Invalid/non-existent transaction handling
- 4+ decimal place rounding
- Duplicate transaction rejection
- Locked account transaction rejection

## AI Usage

I use Github Copilot built into VSCode as my main source of AI-assistance. This was mostly helping me via inline-suggestions for filling out boilerplate code or 'obvious' functions such as `handle_withdrawal` once I had the `handle_deposit` setup.

The transcripts for any specific prompts via the Chat/Agent feature can be found in `docs/transcripts/`

I mostly used `Claude Haiku 4.5` as it was a 'low cost' solution which is still powerful but not needed for anything complicated.

## Libraries Used

- `csv`: for reading CSV input file
- `serde`: for deserialization of CSV values
- `rust_decimal`: for more reliable floating point arithmetic
