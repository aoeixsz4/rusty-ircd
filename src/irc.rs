// this module will contain various definitions taken directly
// from the official IRC protocols (RFCs 1459, 2812)
// e.g. an enum type IRC_Command, which will include possible commands,
// a communication buffer type used for server<->client communication


const MESSAGE_SIZE: usize = 512;
const AZ_STRING: str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub enum BufferError {
	OverFlow,
}

// // since these won't be dynamic during run, it seems silly to use an actual hash table
// const COMMAND_STRING_MAPPING: [(str, Command); 6] = [
//	("JOIN", 
fn is_AZ(&string: String) -> bool {
	for &byte in string.as_bytes() {
		if AZ_STRING.find(byte) == None {
			return false;
		}
	}
	return true;
}

// this lil function snatches up a word and returns the rest of the string
// in an Option<String>, or just gives back the original String plus a None
pub fn split_colon_arg(&mut message_string: String, delimiter) -> Option<String> {
	if (let Some(colon_arg_index) = message_string.find(" :")) {
		let colon_arg = String::from(&message_string[(colon_arg_index + 2)..]);
		message_string.truncate(colon_arg_index);
		colon_arg
	} else {
		None
	}
}

pub enum Command {
	Join(Option<String>), // #channel
	Part(Option<String>, Option<String>), // #channel, part-message
	Message(Option<String>,Option<String>), // dest (user/#channel), message
	Nick(Option<String>), // choose nickname
	User(Option<String>), // choose username (might need addition parameters)
	Quit(Option<String>) // quit-message
}

pub enum ParseError {
	CommandNotRecognised(String),
	CommandNotAZ(String),
	TooFewArguments(String)
}


// parsing IRC messages :)
// we'll also take ownership, calling function shouldn't need the original string anymore
pub fn parse_message(message_string: String) -> Result<Ok(Command), Err(ParseError)> {
	let mut message_string = message_string.trim();

	// I forgot how IRC protocol actually works
	// first we need to check for a colon (but only do anything with the first one)
	// anything before is space-separated, everything after the colon is a single argument,
	// usually a message
	let colon_arg = split_colon_arg(&mut message_string);
	let args: Vec<&str> = message_string.split(' ').collect();
	if let Some(arg) = colon_arg {
		args.push(arg);
	}

	// first word is always a command
	if !is_AZ(command_word) {
		return Err(ParseError::CommandNotAZ(command_word));
	}
	
	// command_type is an enum, but needs its parameters filled
	// also need some checks for things like optional params
	match &args[0] {
		"JOIN" => {
			// error if not enough args
			if args.len < 2 {
				return Err(ParseError::TooFewArgs("JOIN"));
			}
			
			// anything else will be ignored, JOIN only needs a chan argument
			// arg strings will be cloned before passing to Command::type(),
			// otherwise we will have lifetime problems, and we can't move stuff
			// from the args vector - this way args will cleanly go out of scope
			let channel = args[1].clone();
			if !valid_channel_name(channel) {
				return Err(ParseError::InValidChan("JOIN", channel));
			}

			// return Join command
			Ok(Command::Join(channel))
		}
		"PART" => {
			// error if no chan given
			if args.len < 2 {
				return Err(ParseError::TooFewArguments("JOIN"));
			}

			let channel = args[1].clone();
			// anything in rest will be ignored, JOIN only needs a chan argument
			if !valid_channel_name(channel) {
				return Err(ParseError::InValidChan("JOIN", channel));
			}

			// Option<String> is the expected type for Command::Part.part_message
			let part_message: Option<String> = None;
			if args.len > 2 {
				part_message = Some(args[2].clone());
			}

			Ok(Command::Part(channel, part_message))
		}
		"NICK" => {
			// error if no nick given
			if args.len < 2 {
				return Err(ParseError::TooFewArguments("JOIN"));
			}

			let nick = args[1].clone();
			if !valid_nick_name(nick) {
				return Err(ParseError::InValidNick("NICK", nick));
			}

			Ok(Command::Nick(nick))
		}
		"USER" => {
			
		_ => Err(ParseError::CommandNotRecognised(command_word))
	}
}


// might not always want this public
pub struct MessageBuffer {
	// the IRC protocol defines a maximum message size of 512 bytes,
	// including CR-LF. This being the case it doesn't make sense to
	// use buffers that resize as the client sends data, having a fixed
	// size will be generally faster due to simplified memory management
	buffer: [char; MESSAGE_SIZE],  // this needs to be char for String::extend() to work with a slice
	pub index: usize, // for incoming buffers we need some type of error handling
		    // if we reach the end of the buffer and don't find CR-LF
}

impl MessageBuffer {
	// here we want to return the index of the next line, *after* CR-LF
	// so that the extract() fn spits out a string complete with carriage return
	// that will be stripped somewhere else in the program before/during message parsing
	fn get_eol(&self) -> Option<usize> {
		// anything past self.index is old (invalid!) data
		for i in 1..self.index {
			// byte literals are u8
			if self.buffer[i-1] == (b'\r' as char) && self.buffer[i] == (b'\n' as char) {
				return Some(i+1)
			}
		}
		None
	}

	// necessary to explicitly code for case where index is out of bounds? 
	// Rust should detect it and panic, I suppose
	fn shift_bytes_to_start(&mut self, start_index: usize) {
		// there's no need to copy everything to the very end of the buffer,
		// if it hasn't been completely filled
		for (i, j) in (start_index..self.index).enumerate() {
			self.buffer[i] = self.buffer[j];
		}
		self.index = self.index - start_index;  // there was a bug here! index should be shifted, not reset
	}

	// we only need this for client input buffers, so
	// might make more sense to implement in ClientIO?
	// then again its a task performed on the message buffer
	// and may prove to be more general
	// this probably should only be called when we know there's a CR-LF
	// to be found, but just incase we treat the no CR-LF case as
	// "return whatever string happens to currently be in there"
	pub fn extract(&mut self) -> Option<String> {
		if self.index == 0 {
			return None;
		}
		let mut out_string = String::new();
		if let Some(eol_index) = self.get_eol() {
			println!("got eol");
			out_string.extend(&self.buffer[0..eol_index]);
			self.shift_bytes_to_start(eol_index);
		} else {
			println!("no eol");
			out_string.extend(&self.buffer[..self.index]);
			self.index = 0;
		}
		println!("our string: {}", out_string);
		Some(out_string)
	}

	// we also want code for appending to these buffers, more for server-> client writes
	// this can fail if the buffer doesn't have room for our message (probably indicates a connection problem)
	// for client buffers we're reading, this might be called by the incoming socket data event handler
	pub fn append(&mut self, message_string: String) -> Result<(), BufferError> {
		// how much space is left in the buffer?
		// does it make sense to try a partial write?
		if message_string.len() > (MESSAGE_SIZE - self.index) {
			return Err(BufferError::OverFlow);
		}
		for &byte in message_string.as_bytes() {
			self.buffer[self.index] = byte as char;
			self.index += 1;
		}
		return Ok(()); // returning Ok(current_index) as an output might be an option
	}

	pub fn new() -> MessageBuffer {
		MessageBuffer {
			buffer: [0 as char; MESSAGE_SIZE],
			index: 0,
		}
	}
}	

//mod client;
