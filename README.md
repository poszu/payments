# Simple payments simulation system

It parses a CSV file containing a list of transactions and applies them. On exit, it dumps all existing clients in a CSV form.

## Usage

```
cargo run transactions.csv > output.csv
```

# Opens

## Can a transaction be disputed again after a previous dispute was resolved?

I assumed it cannot, hence an operation after a dispute is resolved is put into a `Resolved` state, from where it cannot be disputed again.

## Should a failed transaction still end up in client being created?

For example consider the following input:

```csv
type, client, tx, amount
withdrawal, 1, 0, 1
```

The withdrawal transaction `0` will fail because a freshly created client `1` has zero funds available.

For simplicity, I assumed that this newly created client is left in the `Payments` database.

## Can a deposit transaction be disputed if it would result in account balance becoming negative?

I assumed it cannot. Such a dispute transaction is rejected.
