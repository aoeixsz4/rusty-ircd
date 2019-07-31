fn main() {
    let mut foo = vec!["abc".to_string(), "def".to_string(), "ghi".to_string()];
    let mut bar = vec!["xyz".to_string()];
    foo.append(&mut bar);
    for i in foo {
        println!("{}", i);
    }
}
