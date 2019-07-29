fn funny_stuff () -> (String, String) {
	(String::from("how now"), String::from("brown cow"))
}

fn option_test (thingy: String) -> Option<String> {
	if thingy.find(' ') == None {
		None
	} else {
		Some(String::from("LOLS"))
	}
}

#[derive(Debug)]
pub enum BoxDenizen {
	Small,
	Medium(&f64, &f64),
	Large(f64, f64, f64, f64, f64)
}

fn main () {
	let bar = "woah";
	let val_1 = 5.3e-7;
	let val_2 = 4.8e4;
	let x: Box<BoxDenizen> = match bar {
		"jujubee" => Box::new(BoxDenizen::Small as BoxDenizen),
		"woah" => Box::new(BoxDenizen::Medium(&val_1, &val_2) as BoxDenizen),
		"asdf" => Box::new(BoxDenizen::Large(1.2e-4, 32.6e-8, 32.54e3, 217.1e5, 1.0) as BoxDenizen),
		_ => Box::new(BoxDenizen::Small)
	};

	match *x {
		BoxDenizen::Small => println!("small box denizen"),
		BoxDenizen::Medium(&mut x, &mut y) => {
			println!("medium box denizen, contains: {}, {}", x, y);
			x = 9.0f64;
			y = 10.0f64;
		}
		BoxDenizen::Large(a, b, c, d, e) => println!("large box denizen, contains: {}, {}, {}, {}, {}", a, b, c, d, e)
	}
	
	match *x {
		BoxDenizen::Small => println!("small box denizen"),
		BoxDenizen::Medium(x, y) => {
			println!("medium box denizen, contains: {}, {}", x, y);
		}
		BoxDenizen::Large(a, b, c, d, e) => println!("large box denizen, contains: {}, {}, {}, {}, {}", a, b, c, d, e)
	}
}
