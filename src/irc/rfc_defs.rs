// rfc_defs
//
use std::env;

struct Range(char, char);

const MAX_MSG_PARAMS: usize = 15; // including tailing, but not including COMMAND
const LETTER_RANGES: &'static [Range; 2] = &[Range('a', 'z'), Range('A', 'Z')];
const SPECIAL_RANGES: &'static [Range; 2] = &[Range(0x5b as char, 0x60 as char),
                                        Range(0x7b as char, 0x7d as char)];
const DIGIT_RANGE: Range = Range('0', '9');
const HEXDIGIT_RANGES: &'static [Range; 2] = &[DIGIT_RANGE, Range('A', 'F')];

// user can have any character which is not in the set CONTROL, or an '@'
const CONTROL: &'static [char; 5] = &[0x0 as char, 0x0a as char, 0x0d as char, ' ', ':'];


// this can probably be generalised a bit
fn matches_allowed_set (msg: &str, allowed: &str) -> bool {
    for i in 0..msg.len() {
        if !(allowed.contains(&msg[i..i+1])) {
            return false
        }
    }
    true
}

fn matches_disallowed_set (msg: &str, disallowed: &str) -> bool {
    for i in 0..msg.len() {
        if disallowed.contains(&msg[i..i+1]) {
            return true;
        }
    }
    false
}

// rfc states nick should be max 9 in length,
// pretty sure I've seen far longer nicks on most IRC servers though
fn valid_nick (nick: &str) -> bool {
    if nick.len() > 9 || nick.len() < 1 {
        return false;
    }

    for (i, item) in nick.chars().enumerate() {
        match item {
            'a' ... 'z' => continue,
            'A' ... 'Z' => continue,
            '[' ... '^' => continue,
            '{' ... '}' => continue,
            '0' ... '9' if i > 0 => continue,
            '-' if i > 0 => continue,
            _ => return false
        }
    }

    true
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let nick = if args.len() > 1 {
        &args[1][..]
    } else {
        "JoeyMoe"
    };

    if valid_nick(nick) {
        println!("nick is valid!");
    } else {
        println!("nick is not valid :(");
    }
}
