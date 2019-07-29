fn baz(a: &str) -> (&str, &str) {
    if let Some(index) = a.find('y') {
        a = &a[..index];
        (&a, &a)
    } else {
        ("", "")
    }
}

fn main () {
    let foo = String::from("xyz");
    let bar = &foo[..];

    {
        let (few, stuff) = baz(bar);
        bar = stuff;
    }

    println!("bar: {}", bar);
}
