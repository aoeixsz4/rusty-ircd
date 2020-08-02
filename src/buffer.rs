// this module contains a buffer type for reading and writing to sockets
// and will be involved in the transfer of control from the event system
// to the core irc protocol handlers
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::error::Error;
use std::convert::From;
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

impl From<BufferError> for IoError {
    fn from(_e: BufferError) -> Self {
       IoError::new(IoErrorKind::Other, "buffer overflow")
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
    fn shift_bytes(&mut self, src_i: usize, dest_i: usize, len: usize) {
        // there's no need to copy everything to the very end of the buffer,
        // if it hasn't been completely filled
        for i in 0 .. len {
            buffer[dest_i + i] = buffer[src_i + i];
        }
        index = dest_i + len;
    }

    pub fn shift_bytes_to_start (&mut self, src_i: usize) {
        self.shift_bytes(src_i, 0, index - src_i);
    }
    
    fn get_eol (&self) -> Option<usize> {
        if index < 2 {
            return None;
        }
        for i in 0..index - 1 {
            if buffer[i] == ('\r' as u8) && buffer[i + 1] == ('\n' as u8) {
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
    pub fn extract_ln(&mut self) -> Option<String> {
        match self.get_eol() {
            Some(i) => {
                let out = String::from_utf8_lossy(&self.buffer[0..i]).to_string();
                self.shift_bytes_to_start(i + 2);
                Some(out)
            },
            None => None
        }
    }

    // need a pub fn to copy our private buffer to some external &mut borrowed buffer
    pub fn copy(&self, copy_buf: &mut [u8]) -> usize {
        for i in 0 .. index {
            copy_buf[i] = buffer[i];
        }
        index
    }

    pub fn append_ln(&mut self, line: &str) -> Result<(), BufferError> {
        // how much space is left in the buffer?
        // does it make sense to try a partial write?
        if line.len() + 2 > (rfc::MAX_MSG_SIZE - self.index) {
            return Err(BufferError::Overflow);
        }
        if let Err(e) = self.append_str(line) { return Err(e); }
        append_str("\r\n")
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
            buffer[index] = byte;
            index += 1;
        }
        return Ok(()); // returning Ok(current_index) as an output might be an option
    }

    pub fn append_bytes(&mut self, buf: &[u8]) -> Result<(), BufferError> {
        if buf.len() > (rfc::MAX_MSG_SIZE - self.index) {
            return Err(BufferError::Overflow);
        }
        for i in 0 .. buf.len() {
            buffer[self.index + i] = buf[i];
        }
        index += buf.len();
        Ok(())
    }

    pub fn new() -> MessageBuffer {
        MessageBuffer {
            buffer: [0; rfc::MAX_MSG_SIZE],
            index: 0,
        }
    }
}    

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::eq;

    #[test]
    fn extract_ln_test() {
        let buf = MessageBuffer::new();
        let mut lilbuf = [0; 32];

        buf.append_ln("foobar");
        buf.append_ln("asdf");
        buf.append_ln("OMGERD");
        let foo_str = buf.extract_ln();
        assert_eq!(&foo_str, "foobar");
        buf.copy(&mut lilbuf);
        let len = buf.index();
        assert_eq!(len, 14);
        let remnant_str = String::from_utf8_lossy(&lilbuf[..len]).to_string();
        assert_eq!(&remnant_str, "asdf\r\nOMGERD\r\n");
    }

    #[test]
    fn overflow_test() {
        let buf = MessageBuffer::new();
        let mut pass = false;
        for i in 1 .. 20 {
            if let Err(_) = buf.append_ln("I want this line to be fairly long to make sure 20 iterations is enough to ensure buffer overflow.") {
                pass = true;
            }
        }
        assert_eq!(pass, true);
    }

    #[test]
    fn shift_bytes_test() {
        // check that the index is updated correctly after 
        // a shift of buffer data in MessageBuffer
        let buf = MessageBuffer::new();
        buf.append_str("y0bitrazy borstord.");
        buf.append_str("y0u crai want arst.");
        buf.append_str("y0ofcra\r\nndom rstord.");
        buf.append_str("y0u crazy borstord.");
        buf.append_str("just trydata i get.");
        buf.append_str("yhererazn\r\n blittled.\r\n");
        //let buf_l = buf.buffer.read().unwrap();
        //let ind_l = buf.index.read().unwrap();

        // this is mainly just used to shift everything
        // to start of buffer. really simple & probably
        // could be done with an iterator over slices
        // but the code is here and it can do more than
        // that so theyre may be interesting corner cases
        // e.g. undesired behaviour if dest falls within
        // src and len, this then fails the test comparing
        // equality of the resulting strings

        // however, this function can in principle copy n bytes
        // starting from somewhere to some other particular location
        // try something relatively simple first:
        // copy 10 chars from pos 20 to pos 0
        let mut tmp_buf = [0; 128];
        let mut tmp_buf2  = [0; 128];
        let tuples = [ (20, 0, 10, 1),
                       (0, 45, 5, 3),
                       (35, 40, 5, 4) ];
        for tup in &tuples {
            let (src, dest, len, iter) = *tup;
            buf.copy(&mut tmp_buf);
            buf.shift_bytes(src, dest, len);
            buf.copy(&mut tmp_buf2);
            let str_a = String::from_utf8_lossy(&tmp_buf[src..src+len]).to_string();
            let str_b = String::from_utf8_lossy(&tmp_buf2[dest..dest+len]).to_string();
            assert!(str_a.eq(&str_b),
                    "failed {}th iteration; left: {}, right {}, src={}, dest={}, len={}",
                    iter, &str_a, &str_b, src, dest, len);
        }
    }
}
