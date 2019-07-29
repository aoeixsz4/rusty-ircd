fn foo() -> String {
	let veccy = vec![String::from("lol"), String::from("stuff")];
	veccy[1]
}

fn main() {
	let string = foo();
	println!("string = {}", string);
}
