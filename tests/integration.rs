use payments::{parser::parse, payments::Payments};

fn process_and_dump(input: &str) -> String {
    let mut payments = Payments::default();

    let rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(input.as_bytes());

    for trans in parse(rdr) {
        let _ = payments.apply(trans.unwrap()); // ignore errors
    }

    let mut output = Vec::<u8>::new();
    payments.serialize(&mut output).unwrap();
    String::from_utf8(output).unwrap()
}

#[test]
fn empty() {
    assert_eq!(process_and_dump("type,client,tx,amount"), "");
}

#[test]
fn one_deposit() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            deposit, 1, 1, 1.0"#
        ),
        ["client,available,held,total,locked", "1,1,0,1,false", ""].join("\n")
    );
}

#[test]
fn from_task_desc() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            deposit, 1, 1, 1.0
            deposit, 2, 2, 2.0
            deposit, 1, 3, 2.0
            withdrawal, 1, 4, 1.5
            withdrawal, 2, 5, 3.0"#
        ),
        r#"client,available,held,total,locked
        1, 1.5, 0, 1.5, false
        2, 2, 0, 2, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn withdraw_below_balance() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            withdrawal, 1, 4, 1.5"#
        ),
        r#"client,available,held,total,locked
        1, 0, 0, 0, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn dispute_non_existing_client() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            dispute, 1, 4,"#
        ),
        r#"client,available,held,total,locked
        1, 0, 0, 0, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn resolve_non_existing_client() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            resolve, 1, 4,"#
        ),
        r#"client,available,held,total,locked
        1, 0, 0, 0, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn chargeback_non_existing_client() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            chargeback, 1, 4,"#
        ),
        r#"client,available,held,total,locked
        1, 0, 0, 0, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn dispute_would_result_in_below_balance() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            deposit, 1, 1, 1
            withdrawal, 1, 2, 1
            dispute, 1, 1, "#
        ),
        r#"client,available,held,total,locked
        1, 0, 0, 0, false
        "#
        .replace(' ', "")
    );
}

#[test]
fn sophisticated() {
    assert_eq!(
        process_and_dump(
            r#"type,client,tx,amount
            deposit, 1, 1, 1
            deposit, 2, 3, 10.1234
            withdrawal, 1, 2, 1
            deposit, 1, 4, 0.6666
            dispute, 1, 2,
            chargeback, 1, 2,
            deposit, 3, 5, 1.7777
            dispute, 3, 5,
            deposit, 1, 5, 2"# // should fail as account frozen
        ),
        r#"client,available,held,total,locked
        1, 1.6666, 0, 1.6666, true
        2, 10.1234, 0, 10.1234, false
        3, 0.0000, 1.7777, 1.7777, false
        "#
        .replace(' ', "")
    );
}
