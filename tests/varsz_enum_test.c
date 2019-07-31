// ok, so wanna try another way of testing whether or not I can have a function return a variable
// size enum or struct or whatever...

use std::env;

pub enum MyEnum {
    AsdfType,
    FooType(String),
    BarType(u32, u32, u32)
}

fn main () {
    for arg in env::args() {
        match enum_generator(&arg) {
            MyEnum::AsdfType => println!("asdf!! lol!!"),
            MyEnum::FooType(s) => println!("Foo type! {}", s),
            MyEnum::BarType(a, b, c) => println!("{}-{}-{}", a, b, c)
        }
    }
}

fn enum_generator(arg: &str) -> MyEnum {
    let mut toks: Vec<&str> = arg.split('_').collect();
    match toks.remove(0) {
        "foo" if toks.len() > 0 => MyEnum::FooType(toks.join("").to_string()),
        "bar" if toks.len() == 3 => MyEnum::BarType(toks[0].parse().unwrap(), toks[1].parse().unwrap(), toks[2].parse().unwrap()),    // could I make a closure that fills this in?
        _ => MyEnum::AsdfType
    }
}
