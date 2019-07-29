use std::env;

// this lil function snatches up a word and returns the rest of the string
// in an Option<String>, or just gives back the original String plus a None
fn split_colon_arg(msg: &str) -> (&str, Option<&str>) {
    if let Some(tail) = msg.find(" :") {
        (&msg[..tail], Some(&msg[tail+2..]))
    } else {
        (msg, None)
    }
}

fn main () {
    let args: Vec<String> = env::args().collect();
    let cli_arg = if args.len() > 1 {
        args[1].parse::<usize>().unwrap()
    } else {
        9
    };
    // check for and split off the trailing argument
    // this time we won't modify what the message slice refers to, since we have to deal with an
    // edge-case involving two conflicting sources of a 'trailing' parameter
    let param_substring = "abc def ghi jkl foo bar baz xyz :trailing mother fucking arguments";
    let (middle, trailing) = split_colon_arg(&param_substring);
    let mut param_slices: Vec<&str>;
    match trailing {
        Some(trail_arg) if middle.matches(' ').count() < cli_arg => {
            // how many spaces would we have for 15 parameters? 14 spaces,
            // and if we have 15 parameters in *middle*, the last one has to
            // swallow up trailing
            param_slices = middle.splitn(cli_arg-1, ' ').collect();
            param_slices.push(trail_arg);
        }
        _ => param_slices = param_substring.splitn(cli_arg, ' ').collect()
    }
    for (i, &item) in param_slices.iter().enumerate() {
        println!("param {}: {}", i, item);
    }
    let mut new_owned_vec: Vec<String> = Vec::new();
    for i in param_slices.iter() {
        new_owned_vec.push(i.to_string());
    }
    println!("sizeof(new_owned_vec) = {}", new_owned_vec.len());
    println!("debug(new_owned_vec) = {:?}", new_owned_vec);
}
