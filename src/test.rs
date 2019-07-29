use std::env;

pub enum Cmd {
    Join(String),
    Quit,
    Message(String, String)
}

pub enum MyErrorType {
    Fail
}

fn gimme_enum_box(cmd: &str) -> Result<(String, Box<Cmd>), MyErrorType> {
    match cmd {
        "JOIN" => {
            let msg = String::from("you joined!");
            let boxy = Box::new(Cmd::Join(String::from("channel")));
            Ok((msg, boxy))
        }
        "QUIT" => {
            let msg = String::from("oh no :( bye");
            let boxy = Box::new(Cmd::Quit);
            Ok((msg, boxy))
        }
        "MSG" => {
            let msg = String::from("hi!");
            let boxy_messages = (String::from("foo"), String::from("bar"));
            let boxy = Box::new(Cmd::Message(boxy_messages.0, boxy_messages.1));
            Ok((msg, boxy))
        }
        _ => Err(MyErrorType::Fail)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = if args.len() > 1 {
        &args[1]
    } else { "FAIL" };
    match gimme_enum_box(cmd) {
        Ok(tuple) => {
            let (msg, mybox) = tuple;
            println!("{}", msg);
            match *mybox {
                Cmd::Join(chan) => println!("you are now in: {}", chan),
                Cmd::Quit => (),
                Cmd::Message(string1, string2) => println!("your messages are: {}, {}", string1, string2),
            }
        }
        Err(error) => match error {
            MyErrorType::Fail => println!("you are a failure!")
        }
    }
}

