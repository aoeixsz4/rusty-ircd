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
    let mut args_iter = env::args();
    let program = args_iter.next().unwrap();
    println!("program was: {}", program);
    let args_joined = args_iter.collect::<Vec<String>>().join(", ");
    println!("args were: {}", args_joined);
}

