fn main () {
//    let message = ":nick!user@host ASDF irc command bla bla stuff :tailing mother fucking args";
//    let message = ":nick!user@host";
    let message = ":";
    let (opt_prefix, opt_msg_body) = find_prefix(message);

    let msg_body: &str = if let Some(substr) = opt_msg_body {
        substr
    } else {
        // prefix string but no message body shouldn't really happen...
        println!("ParseError::NoCommand"); return
    };
    if let Some(prefix) = opt_prefix {
        println!("prefix: {}", prefix);
    }
    println!("body of message: {}", msg_body);
    let (command, param_substring) = get_command(msg_body);
    println!("command: {}", command);
    println!("rest of stuff: {}", param_substring.expect("foobar, fucked it"));
}

pub enum ParseError {
    InvalidPrefix
}

// this'll do a splitn(2, ' '), then return the command,
// plus optionally the rest of the message body, or None
// if the command has no parameters at all (which is a valid case)
// note one of the return values is a String! that's because
// the caller wants one, and we also give up ownership, not a borrow
fn get_command(msg_main: &str) -> (String, Option<&str>) {
    let substrings: Vec<&str> = msg_main.splitn(2, ' ').collect();
    match substrings.len() {
        1 => (substrings[0].to_string(), None),
        2 => (substrings[0].to_string(), Some(substrings[1])),
        _ => panic!("unhandled exception, should be impossible")
    }
}

fn find_prefix(message: &str) -> (Option<&str>, Option<&str>) {
    // if we have a prefix, we will first have a colon indicator
    // we know we will never have an empty line, but message.chars().nth(0) can give a
    // Some(whatever) or a None, so we have to explicitly check that, or use a string slice
    if &message[..1] == ":" {
        // check for a space
        let substrings: Vec<&str> = (&message[1..]).splitn(2, ' ').collect();
        match substrings.len() {
            1 => (Some(substrings[0]), None),
            2 => (Some(substrings[0]), Some(substrings[1])),
            _ => panic!("unhandled exception, should never happen")
        }
    } else {
        // no prefix found, so we just come back with only Some(message)
        // but a None for the prefix option
        (None, Some(message))
    }
}

// parse the prefix part of an IRC message
