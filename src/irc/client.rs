// IRC::client
// this file contains protocol defitions related to client commands coming to the server

pub enum Command {
	Join(String), // #channel
	Part(String, Option<String>), // #channel, part-message
	Message(String, String), // dest (user/#channel), message
	Nick(String), // choose nickname
	User(String), // choose username (might need addition parameters)
	Quit(String), // quit-message
}

pub struct IO {
	// and 512 is after all not huge
	// this way, we allocate 1 MB of fixed sized buffers on the heap once per client
	// if we make this an Object rather than just a normal struct, we can
	// include here a write method, and a reference to a Socket type of some sort
	out_buffer: super::MessageBuffer, // out -> to client
	in_buffer: super::MessageBuffer, // in <- from client
}

impl IO {
	pub fn new() -> IO {
		IO {
			mut out_buffer: super::MessageBuffer;
			mut in_buffer: super::MessageBuffer;
		}
	}

	// an event handler waiting on new data from the client
	// must call this handler when a CR-LF is found
	// return type is a ClientCommand, which will be processed elsewhere
	pub fn end_of_line(&mut self) -> ClientCommand {
		// NB: buffer index might not be directly after the CR-LF
		// first bit of code locates a CR-LF
		// next bit extracts a string and moves buffer data after CR-LF
		// to front, reseting the index afterwards
		let command_string = self.out_buffer.extract();
	}
}
