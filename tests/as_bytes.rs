fn main () {
    let str_slice = "asdf";
    let (first_char, rest) = (str_slice.as_bytes()[0] as char, &str_slice[..]);
    println!("first char: {}, rest: {}", first_char, rest);
}

