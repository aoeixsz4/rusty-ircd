// this module contains a buffer type for reading and writing to sockets
// and will be involved in the transfer of control from the event system
// to the core irc protocol handlers
use std::error::Error;
use std::fmt;
use crate::irc::rfc_defs as rfc;

#[derive(Debug)]
pub enum BufferError {
    Overflow,
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Buffer overflow in client output buffer")
    }
}

impl Error for BufferError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

// might not always want this public
pub struct MessageBuffer {
    // the IRC protocol defines a maximum message size of 512 bytes,
    // including CR-LF. This being the case it doesn't make sense to
    // use buffers that resize as the client sends data, having a fixed
    // size will be generally faster due to simplified memory management
    buffer: [u8; rfc::MAX_MSG_SIZE],  // this needs to be char for String::extend() to work with a slice
    pub index: usize, // for incoming buffers we need some type of error handling
            // if we reach the end of the buffer and don't find CR-LF
}

impl MessageBuffer {
    // necessary to explicitly code for case where index is out of bounds? 
    // Rust should detect it and panic, I suppose
    // this should normally be called to move a chunk of buffer content to the beginning of the
    // buffer, but in principle it can be used for other things too
    // if dest_i > src_i it's more like a copy than a shift
    pub fn shift_bytes(&mut self, src_i: usize, dest_i: usize, len: usize) {
        // there's no need to copy everything to the very end of the buffer,
        // if it hasn't been completely filled
        for i in 0 .. len {
            self.buffer[dest_i + i] = self.buffer[src_i + len];
        }
        self.index = dest_i + len;
    }

    pub fn shift_bytes_to_start (&mut self, src_i: usize) {
        self.shift_bytes(src_i, 0, self.index - src_i);
    }
    
    fn get_eol (&self) -> Option<usize> {
        if self.index < 2 {
            return None;
        }
        for i in 0..self.index - 1 {
            if self.buffer[i] == ('\r' as u8) && self.buffer[i + 1] == ('\n' as u8) {
                return Some(i);
            }
        }
        None
    }

    pub fn has_delim (&self) -> bool {
        match self.get_eol() {
            Some(_) => true,
            None => false
        }
    }

    // we only need this for input buffers, so
    // might make more sense to implement in ClientIO?
    // then again its a task performed on the message buffer
    // and may prove to be more general
    // this probably should only be called when we know there's a CR-LF
    // to be found, but just incase we treat the no CR-LF case as
    // "return whatever string happens to currently be in there"
    // we'll also silently throw away the CR-LF itself and return
    // the line itself, already clipped
    pub fn extract_ln(&mut self) -> String {
        match self.get_eol() {
            Some(i) => {
                let out = String::from_utf8_lossy(&self.buffer[0..i]).to_string();
                self.shift_bytes_to_start(i + 2);
                out
            }
            None => {
                let out = String::from_utf8_lossy(&self.buffer[..]).to_string();
                self.index = 0;
                out
            }
        }
    }

    // need a pub fn to copy our private buffer to some external &mut borrowed buffer
    pub fn copy(&self, copy_buf: &mut [u8]) -> usize {
        for i in 0 .. self.index {
            copy_buf[i] = self.buffer[i];
        }
        self.index
    }

    // we also want code for appending to these buffers, more for server-> client writes
    // this can fail if the buffer doesn't have room for our message (probably indicates a connection problem)
    // for client buffers we're reading, this might be called by the incoming socket data event handler
    pub fn append_str(&mut self, message_string: &str) -> Result<(), BufferError> {
        // how much space is left in the buffer?
        // does it make sense to try a partial write?
        if message_string.len() > (rfc::MAX_MSG_SIZE - self.index) {
            return Err(BufferError::Overflow);
        }
        for &byte in message_string.as_bytes() {
            self.buffer[self.index] = byte;
            self.index += 1;
        }
        return Ok(()); // returning Ok(current_index) as an output might be an option
    }

    pub fn append_bytes(&mut self, buf: &[u8]) -> Result<(), BufferError> {
        if buf.len() > (rfc::MAX_MSG_SIZE - self.index) {
            return Err(BufferError::Overflow);
        }
        for i in 0 .. buf.len() {
            self.buffer[self.index + i] = buf[i];
        }
        self.index += buf.len();
        Ok(())
    }

    pub fn new() -> MessageBuffer {
        MessageBuffer {
            buffer: [0; rfc::MAX_MSG_SIZE],
            index: 0,
        }
    }
}    

