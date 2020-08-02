// this module will contain protocol definitions
// and core data types and handlers for IRC commands
//use crate::parser;

pub mod rfc_defs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Weak, Mutex};
use std::collections::HashMap;
use std::clone::Clone;
use dns_lookup::lookup_addr;
use tokio::net::TcpStream;
use crate::client;
use crate::client::{Client,ClientType};
use crate::parser::ParsedMsg;

// I hope it doesnt get too confusing that parser.rs and irc.rs both have a 'Host' enum,
// main difference is the parser's variants only contain strings (either for hostname
// or ip address), the type here will contain a hostname string or a proper ip address type
// from std::net
pub enum Host {
    Hostname(String),
    HostAddr(IpAddr)
}

pub fn create_host_string (host_var: &Host) -> String {
    match host_var {
        Host::Hostname(hostname_str) => hostname_str.to_string(),
        Host::HostAddr(ip_addr) => ip_addr.to_string()
    }
}

pub enum Origin {
    Server(Host),
    User(String, Option<String>, Option<Host>)
}

// having to define and match these types here,
// as well as matching the raw command strings in parser::parse_message()
// feels somehow not quite right, but I haven't yet got a more elegant solution
// maybe parser should just hand over an Option<String> containing the src/origin prefix,
// followed by a vector reference of String args
// but the construction of the enum / match case is still clumsy,
// I want an elegant and fast way to go from a string literal to an enum...
// even with a match to string literals, I think it makes more sense to
// have the specific command cases handled here, and not have to change lots of things both
// here and in parser() if we want to add/change how commands work
pub enum Command {
    Join(Vec<String>), // #channel
    Part(Vec<String>, Option<String>), // #channel, part-message
    Message(Vec<String>, String), // dest (user/#channel), message
    Nick(String), // choose nickname
    User(String, u64, String), // choose username (might need addition parameters)
    Quit(Option<String>), // quit-message
}

pub enum MsgType {
    Notice,
    PrivateMsg
}

pub enum NamedEntity {
    User(Weak<Mutex<User>>),
    Chan(Weak<Mutex<Channel>>)
}

pub struct UserFlags {
    registered: bool,
}

// of the above, Message(targets, conntent) is arguably the most important feature of an irc server
// it's also emblematic of the way we need to think about structuring data and finding targets,
// there are a few cases we need to consider
// if target is a channel, we have to map that to a list of users JOINed to that channel,
// this can be done with the Channel::users field, but to get to the Channel data struct,
// we need a HashMap of channel names in order to locate that data
// once we have converted our list of targets to a list of nicks, we need to check which of those
// are local (connected as clients to our server), and which are remote - for those 
// which are remote, we have to remove their names from the target list and substitute in a server,
// so we need to know which server acts as a relay for each remote user
pub struct User {
    id: u64,                            // we can have this just be the same as the client_id
    nick: String,
    username: String,
    real_name: String,
    host: Host,
    channel_list: Vec<String>,
    flags: UserFlags,
    core: Core              // Arc::clone copies of all the important stuff
}

pub struct ProtoUser {
    nick: Option<String>,
    username: Option<String>,
    real_name: Option<String>
}

pub struct ServerUserFlags {
    server_op: bool
}

pub struct ServerUser {
    nick: String,
    flags: ServerUserFlags
}

// client may need to be mutable, and probably needs some sort of lifetime parameter
// this is OK as long as the Client is not borrowed anywhere else
// alternatively, we once again use an id as a place holder for a borrow,
// and the core contains a hashmap of client IDs
pub struct Server {
    id: u64,
    host: Host,
    users: Vec<ServerUser>,
    client_id: u64             // need to map 'server' to socketed client
}

//pub struct Message {
//    pub content: Box<Command>,// commands and their parameters are defined as an enum above
//    pub origin: Option<Origin> // important when servers relay commands originating from their users
//}

// flags related to user priveleges in a specific channel
pub struct ChanUserFlags {
    chan_op: bool,
    chan_halfop: bool,
    chan_voice: bool
}

pub struct ChanUser {
    nick: String,
    flags: ChanUserFlags
}

// channel needs a name, a topic, and a list of joined users
// this list can't just be a list of nicks, as additional flags are required: is the user an op on
// the channel, for example?
pub struct Channel {
    name: String,
    users: Vec<ChanUser>,
    topic: String
}

// when we want to do all this with concurrency,
// we could wrap each HashMap in a Mutex, while the
// Core itself will just have immutable borrows
// when a hashmap has been used to obtain a User, Channel or Server,
// again a Mutex wrapper will be needed, and used for as short a time
// as possible in every case
// commands probably doesn't need a Mutex, it will be populated once,
// then remain the same
// do we also need to wrap these in Arc<T> pointers? :/
// maybe it's possible just to have the Core in an Arc<T>,
// and give each thread a pointer to the core?
pub struct Core {
    // NamedEntity has Weak<Mutex<User/Chan>> inside                   // maps client IDs to clients
    pub namespace: Arc<Mutex<HashMap<String, NamedEntity>>>,
    pub clients: Arc<Mutex<HashMap<u64, Weak<Mutex<Client>>>>>,                    // maps client IDs to clients
    pub users: Arc<Mutex<HashMap<String, Weak<Mutex<User>>>>>,          // maps user IDs to users
    pub channels: Arc<Mutex<HashMap<String, Weak<Mutex<Channel>>>>>, // maps channames to Channel data structures
    pub id_counter: Arc<Mutex<u64>>,
    //pub servers: Arc<Mutex<HashMap<u64, Arc<Mutex<Server>>>>>,      // maps server IDs to servers
}

// init hash tables
// let's have this copyable, so whatever thread is doing stuff,
// needs to only mutex lock one hashmap at a time
impl Core {
    pub fn new () -> Core {
        // initialize hash tables for clients, servers, commands
        // clones of the "irc Core" are passed as a field within
        // ClientFuture, but we can still have a client list within
        // irc::Core, as Client is a seperate element within ClientFuture,
        // so we still avoid cyclic refs
        let clients = Arc::new(Mutex::new(HashMap::new()));
        //let servers  = Arc::new(Mutex::new(HashMap::new()));
        let users = Arc::new(Mutex::new(HashMap::new()));
        let channels = Arc::new(Mutex::new(HashMap::new()));
        let namespace = Arc::new(Mutex::new(HashMap::new()));
        let id_counter = Arc::new(Mutex::new(1));
        Core {
            clients,
            channels,
            users,
            namespace,
            id_counter
            //servers
        }
    }
}

pub fn lookup_client (irc: &Core, id: &u64) -> Option<Weak<Mutex<Client>>> {
    // NOTICE: lossy behaviour when lock fails - calling
    // function will assume lookup failed
    // gonna mark all these with NOTICE_LOSSY
    let clients_list = irc.clients.try_lock().unwrap()
        .or_else(|_| { None });
    let client_ptr = clients_list.get(id).unwrap()
        .or_else(|_| { None });
    Some(Weak::clone(client_ptr))
}

pub fn lookup_name (irc: &Core, name: &str) -> Option<NamedEntity> {
    match irc.namespace.try_lock().unwrap()
        .or_else(|_| { None }) { // NOTICE_LOSSY
        Some(NamedEntity::User(user_ptr)) =>
            Some(NamedEntity::User(Weak::clone(&user_ptr))),
        Some(NamedEntity::Chan(chan_ptr)) =>
            Some(NamedEntity::Chan(Weak::clone(&chan_ptr))),
        None =>  None
    }
}

pub fn register (irc: Arc<Mutex<Core>>, client: Arc<Mutex<Client>>, nick: String, username: String, real_name: String) -> Option<ClientType> {
    if let Ok(address) = client.socket.peer_addr() {
    // rdns to get the hostname, or assign a string to the host
        let host = if let Ok(h) = lookup_addr(&address.ip()) {
            Host::Hostname(h)
        } else {
            Host::HostAddr(address.ip())
        };
        println!("register user {}!{}@{}, Real name: {}", nick, username, create_host_string(host), real_name);
        let user = Arc::new( Mutex::new (User {
                id: client.id,
                core: irc.clone(),
                nick: nick.clone(),
                username: username.clone(),
                real_name: realname.clone(),
                host, // moves into struct here
                channel_list: Vec::new(),
                flags: UserFlags { registered: true }
            } ));
        // be damn careful what you do with these atomic reference
        // counted pointers my dear! they are not 100% memory safe
        // (not in terms of leaks anyway. Avoid circular references etc.
        // Core has strong pointers to everyone, but who has pointers
        // to Core?

        // NOTICE_BLOCKY - even worse than lossy, but currently unsure
        // how to handle it properly if we fail to acquire the lock...
        irc.lock().unwrap().namespace
            .insert(nick.clone(), NamedEntity::User(Arc::downgrade((&user))));
        println!("new User has strong-count {} and weak-count {}", Arc::strong_count(&user), Arc::weak_count(&user));
        println!("irc core/namespace has strong-count {} and weak-count {}", Arc::strong_count(&irc.namespace), Arc::weak_count(&irc.namespace));
        Some(ClientType::User(Arc::clone(&user)))
    } else {
        None
    }
}

// handle command should take a Client and a ParseMsg
// the command string will be converted to uppercase and a match block
// will redirect to the specific command handler
pub fn
user_command
(irc: &Core, mut user: &mut User, params: ParsedMsg) 
{
    // we're matching a String to some &str literals, so may need this &
    match &params.command[..] {
        "PRIVMSG" => msg(irc, client, params, MsgType::PrivateMsg),
        "NOTICE" => msg(irc, client, params, MsgType::Notice),
        //"JOIN" => self.join_channel(&mut user, params),
        _ => {
            let client_ptr = lookup_client(irc, &user.id).unwrap();
            let client_arc = Weak::upgrade(&client_ptr).unwrap();
            let mut client = client_arc.write().unwrap();
            client.send_line("unkown command")
        }
    };
}

// handle command should take a Client and a ParseMsg
// the command string will be converted to uppercase and a match block
// will redirect to the specific command handler
pub fn
command
(irc: Arc<Mutex<Core>, client: Arc<Mutex<Client>>, params: ParsedMsg)
    -> Vec<String>
{
    let client_t = client.try_lock().unwrap()
        .or_else(|_|
                 { ret = Vec::new(); ret.push(String::from("mutex error")); ret })
        .client_type.clone();   // NOTICE_LOSSY
    // also I think we want to clone it as the enum can contain an Arc,
    // and rather than move client type outside of the client struct,
    // we need to instead make Arc clones
    // we're matching a String to some &str literals, so may need this &
    // after calling whatever command function, we don't use irc again,
    // so I think just moving the Arc is fine, don't have to clone
    match &params.command[..] {
        "NICK" => nick(irc, client_t, params), // <-- will the borrow checker hate me for this? no,
        "USER" => user(irc, client_t, params),  //     since it's immutable and passed-ownership, borrow-happy
        "PRIVMSG" => msg(irc, client_t, params, MsgType::PrivateMsg),
        "NOTICE" => msg(irc, client_t, params, MsgType::Notice),
        _ => { ret = Vec::new(); ret.push(String::from("unknown command"); ret }
    }
}

pub fn msg
(irc: &Core, my_user: &mut User, params: ParsedMsg, msg_type: MsgType)
    -> Vec<String>
{
    println!("got a message command");
    let mut responses: Vec<String> = Vec::new();
    // if there are more than two arguments,
    // concatenate the remainder to one string
    let args_iter = params.opt_params.iter();
    let target_str = match args_iter.next() {
        Some(arg) => arg,
        None => {
            // probably wanna make an enum of all these
            reponses.push(String::from("411 ERR_NORECIPIENT"));
            return responses;
        }
    }
    let message = args_iter.collect::<Vec<String>>().join(" ");
    // if there were no more args, message should be an empty String
    if message.len() == 0 {
        responses.push(String::from("412 ERR_NOTEXTTOSEND"));
        return;
    }
    println!("target is {} and content is {}", target_str, message);
    // loop over targets
    for target in target_str.split(',') {
        match lookup_name(irc, &target) {
            None =>
                responses.push(String::from("401 ERR_NOSUCHNICK")),
            NamedEntity::User(user_ptr) => {
                // NOTICE_BLOCKY
                if let Err(msg) = user_ptr.upgrade().unwrap()
                    .lock().unwrap().send_msg(&message) {
                        responses.push(msg);
                }
            },
            NamedEntity::Chan(_chan_ptr) => () // not implemented chans yet
        }
    }

    // according to the RFC server should never respond to
    // NOTICE messages, no matter how they fail
    match msg_type {
        MsgType::PrivateMsg => responses,
        MsgType::Notice => Vec::new()
    }
}

pub fn
user(irc: &Core, mut client: &mut Client, params: ParsedMsg)
{
    if let Some(args) = params.opt_params {
        // a USER command should have exactly four parameters
        // <username> <hostname> <servername> <realname>,
        // though we ignore the middle two unless a server is
        // forwarding the message
        if args.len() != 4 {
            // strictly speaking this should be an RFC-compliant
            // numeric error ERR_NEEDMOREPARAMS
            client.send_line("incorrect number of parameters");
            return;
        }
        let username = args[0].clone();
        let real_name = args[3].clone();

        // tuple Some(&str), Some(ClientType), bool died
        let (message_o, new_type_o, died) = match &client.client_type {
            ClientType::Unregistered => {
                // initiate handshake
                (   Some(String::from("HELLO! welcome to IRC, new user")),
                    Some(ClientType::ProtoUser(Arc::new(Mutex::new(ProtoUser {
                        nick: None,
                        username: Some(username),
                        real_name: Some(real_name)})))),
                    false
                )
            },
            ClientType::User(_user_ref) => {
                // already registered! can't change username
                (   Some(String::from("you are already registered!")),
                    None,
                    false
                )
            },
            ClientType::ProtoUser(proto_user_ref) => {
                // got nick already? if so, complete registration
                let proto_user = proto_user_ref.read().unwrap();
                if let Some(nick) = &proto_user.nick {
                    // had nick already, complete registration
                    if let Some(reg_type) = register(irc, &client, nick.clone(), username, real_name) {
                        // register with the server hash map too
                        (   Some(String::from("welcome, new user!")),
                            Some(reg_type),
                            false
                        )
                    } else {
                        (   None,
                            None,
                            true    // connection died
                        )
                    }
                } else {
                    // can only send USER once
                    (   Some(String::from("you already sent USER")),
                        None,
                        false
                    )
                }
            },
            ClientType::Server(_server_ref) => (None, None, false)
        };
        // did we return a message?
        if let Some(message) = message_o {
            client.send_line(&message);
        }

        // update client type if necessary
        if let Some(client_type) = new_type_o {
            client.client_type = client_type;
        }

        // connection error?
        if died == true {
            client.dead = true;
        }
    }
}


pub fn nick(irc: &Core, mut client: &mut Client, params: ParsedMsg) {
    if let Some(args) = params.opt_params {
        let nick = args[0].clone();
        // is this nick already taken?
        if let Some(hit) = lookup_name(irc, &nick) {
            println!("nick collision!");
            client.send_line("433 ERR_NICKNAMEINUSE {} :Nickname is already in use");
        }

        // we can return a tuple and send messages after the match
        // to avoid borrowing mutably inside the immutable borrow
        // (Some(&str), Some(ClientType), bool died)
        let (message_o, new_type_o, died) = match &client.client_type {
            ClientType::Unregistered => { // in this case we need to create a "proto user"
                (   Some(String::from("created a proto user thingy :o")),
                    Some(ClientType::ProtoUser(Arc::new(Mutex::new(ProtoUser {
                        nick: Some(nick),
                        username: None,
                        real_name: None })))),
                    false
                )
            },
            ClientType::User(user_ref) => { // just a nick change
                let mut user = user_ref.write().unwrap();
                user.nick = nick;
                (   None,
                    None,
                    false
                )
            },
            ClientType::ProtoUser(proto_user_ref) => { // in this case we already got USER
                let proto_user = proto_user_ref.read().unwrap();
                // need to account for the case where NICK is sent
                // twice without any user command
                if let Some(_) = &proto_user.nick {
                    (   Some(String::from("you need to send USER to complete registration")),
                        Some(ClientType::ProtoUser(Arc::new(Mutex::new(ProtoUser {
                            nick: Some(nick),
                            username: None,
                            real_name: None })))),
                        false
                    )
                } else {
                    // full registration! wooo
                    let username = match &proto_user.username {
                        Some(user) => user.clone(),
                        None => panic!("no user")  // these panics are legit now
                    };
                    let real_name = match &proto_user.real_name {
                        Some(rname) => rname.clone(),
                        None => panic!("no real name") // i think
                    };
                    if let Some(ctype) = register(irc, &client, nick, username, real_name) {
                        (   Some(String::from("Welcome to IRC! You are registered")),
                            Some(ctype),
                            false
                        )
                    } else {
                        (   None,
                            None,
                            true // dead client
                        )
                    }
                }
            },
            ClientType::Server(_server_ref) => ( None, None, false )
        };

        if let Some(message) = message_o {
            client.send_line(&message);
        }
        if let Some(client_type) = new_type_o {
            client.client_type = client_type;
        }
        if died == true {
            client.dead = true;
        }
    } else {
        client.send_line("not enough parameters!");
    }
}

impl Clone for Core {
    fn clone (&self) -> Self {
        Core {
            clients: Arc::clone(&self.clients),
            channels: Arc::clone(&self.channels),
            users: Arc::clone(&self.users),
            id_counter: Arc::clone(&self.id_counter),
            namespace: Arc::clone(&self.namespace)
            //servers: Arc::clone(irc: &Core.servers)
        }
    }
}
