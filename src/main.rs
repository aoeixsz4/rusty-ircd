mod irc;

fn main () {
	let mut msg_buf = irc::MessageBuffer::new();
	if let Err(_) = msg_buf.append(String::from("JOIN ##rust\r\n")) {
		println!("received an error!");
	}
	if let Err(_) = msg_buf.append(String::from("MSG ##rust Hello, guys\r\n")) {
		println!("received an error!");
	}
	if let Err(_) = msg_buf.append(String::from("LOLOL OMG WUT\r\n")) {
		println!("received an error!");
	}
	if let Err(_) = msg_buf.append(String::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")) {
		println!("received error on big string");
	}
	println!("msg_buf.index is {}", msg_buf.index);
	loop {
		if let Some(string) = msg_buf.extract() {
			println!("{}", string);
			println!("index is now: {}", msg_buf.index);
		}
	}
}

