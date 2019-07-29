// this test won't work without some sort of macro to 
// include all the contents of ../src/irc/rfc_defs.rs

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        println!("usage: ./rfc_defs token_type test_string");
        return;
    }

    let token_type: &str = &args[1];
    let test_string: &str = &args[2];

    // i feel like this is the sort of thing you can generate with
    // macros to save a bit of time actually writing repetetive code
    // like this...
    let return_value = match token_type {
        "nick" => valid_nick(test_string),
        "user" => valid_user(test_string),
        "channel" => valid_channel(test_string),
        "hostname" => valid_hostname(test_string),
        "ipv4addr" => valid_ipv4_addr(test_string),
        "ipv6addr" => valid_ipv6_addr(test_string),
        "command" => valid_command(test_string),
        _ => { println!("unrecognised token type {}", token_type); return }
    };

    if return_value {
        println!("yes, valid");
    } else {
        println!("no, invalid");
    }
}

