// this module will contain protocol definitions
// and core data types and handlers for IRC commands
//use crate::parser;

pub mod rfc_defs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Weak, RwLock};
use std::collections::HashMap;
use std::clone::Clone;
use crate::client;
use crate::client::{Client,ClientType};
use crate::parser::ParsedMsg;
use dns_lookup::lookup_addr;
use tokio::net::TcpStream;

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
    User(Weak<RwLock<User>>),
    Chan(Weak<RwLock<Channel>>)
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
// we could wrap each HashMap in a RwLock, while the
// Core itself will just have immutable borrows
// when a hashmap has been used to obtain a User, Channel or Server,
// again a RwLock wrapper will be needed, and used for as short a time
// as possible in every case
// commands probably doesn't need a RwLock, it will be populated once,
// then remain the same
// do we also need to wrap these in Arc<T> pointers? :/
// maybe it's possible just to have the Core in an Arc<T>,
// and give each thread a pointer to the core?
pub struct Core {
    // NamedEntity has Weak<RwLock<User/Chan>> inside                   // maps client IDs to clients
    pub namespace: Arc<RwLock<HashMap<String, NamedEntity>>>,
    pub clients: Arc<RwLock<HashMap<u64, Weak<RwLock<Client>>>>>,                    // maps client IDs to clients
    pub users: Arc<RwLock<HashMap<String, Weak<RwLock<User>>>>>,          // maps user IDs to users
    pub channels: Arc<RwLock<HashMap<String, Weak<RwLock<Channel>>>>>, // maps channames to Channel data structures
    pub id_counter: Arc<RwLock<u64>>,
    //pub servers: Arc<RwLock<HashMap<u64, Arc<RwLock<Server>>>>>,      // maps server IDs to servers
}

// init hash tables
// let's have this copyable, so whatever thread is doing stuff,
// needs to only mutex lock one hashmap at a time
impl Core {
    pub fn
    new ()
    -> Core {
        // initialize hash tables for clients, servers, commands
        // clones of the "irc Core" are passed as a field within
        // ClientFuture, but we can still have a client list within
        // irc::Core, as Client is a seperate element within ClientFuture,
        // so we still avoid cyclic refs
        let clients = Arc::new(RwLock::new(HashMap::new()));
        //let servers  = Arc::new(RwLock::new(HashMap::new()));
        let users = Arc::new(RwLock::new(HashMap::new()));
        let channels = Arc::new(RwLock::new(HashMap::new()));
        let namespace = Arc::new(RwLock::new(HashMap::new()));
        let id_counter = Arc::new(RwLock::new(1));
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

pub fn lookup_client (irc: &Core, id: &u64) -> Option<Weak<RwLock<Client>>> {
    let clients_list = irc.clients.read().unwrap(); // no need to mutex lock
    let client_ptr = clients_list.get(id).unwrap();
    Some(Weak::clone(client_ptr))
}

pub fn
lookup_name
(irc: &Core, name: &str)
-> Option<NamedEntity> {
    let name_hashmap = irc.namespace.read().unwrap();
    let enum_ot = name_hashmap.get(name);
    // just realised I need to be cloning this
    match &enum_ot {
        Some(NamedEntity::User(user_ptr)) =>
            Some(NamedEntity::User(Weak::clone(&user_ptr))),
        Some(NamedEntity::Chan(chan_ptr)) =>
            Some(NamedEntity::Chan(Weak::clone(&chan_ptr))),
        None =>  None
    }
}

pub fn
register
(irc: &Core, client: &Client, nick: String, username: String, real_name: String)
-> Option<ClientType> {
    if let Ok(address) = client.socket.peer_addr() {
    // rdns to get the hostname, or assign a string to the host
        let host = if let Ok(h) = lookup_addr(&address.ip()) {
            Host::Hostname(h)
        } else {
            Host::HostAddr(address.ip())
        };
        let user = Arc::new( RwLock::new (User {
                id: client.id,
                core: irc.clone(),
                nick,
                username,
                real_name,
                host,
                channel_list: Vec::new(),
                flags: UserFlags { registered: true }
            } ));
        // be damn careful what you do with these atomic reference
        // counted pointers my dear! they are not 100% memory safe
        // (not in terms of leaks anyway. Avoid circular references etc.
        // Core has strong pointers to everyone, but who has pointers
        // to Core?
        // Just realised it seemed weird to use the User struct key
        // when we still have a String in scope - probably still want
        // to clone() it though, as it will have moved into the User struct
        let mut hashmap = irc.namespace.write().unwrap();
        let user_dref = user.read().unwrap();
        let (nick, username, host, real_name)
            = (&user_dref.nick, &user_dref.username, &user_dref.host, &user_dref.real_name);
        hashmap.insert(nick.clone(), NamedEntity::User(Arc::downgrade((&user))));
        println!("registered user {}!{}@{}, Real name: {}", nick, username, create_host_string(host), real_name);
        println!("new User has strong-count {} and weak-count {}", Arc::strong_count(&user), Arc::weak_count(&user));
        println!("irc core/namespace has strong-count {} and weak-count  {}", Arc::strong_count(&user_dref.core.namespace), Arc::weak_count(&user_dref.core.namespace));
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
        "PRIVMSG" => msg(irc, &mut user, params, MsgType::PrivateMsg),
        "NOTICE" => msg(irc, &mut user, params, MsgType::Notice),
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
(irc: &mut Core, mut client: &mut Client, params: ParsedMsg)
{
    // we're matching a String to some &str literals, so may need this &
    match &params.command[..] {
        "NICK" => nick(irc, &mut client, params), // <-- will the borrow checker hate me for this? no,
        "USER" => user(irc, &mut client, params),  //     since it's immutable and passed-ownership, borrow-happy
        _ => match &client.client_type {
            ClientType::User(user_ptr) =>
            user_command(irc, &mut user_ptr.write().unwrap(), params),
            _ => client.send_line("please register")
        }
    };
}

pub fn msg
(irc: &Core, my_user: &mut User, params: ParsedMsg, msg_type: MsgType)
{
    println!("got a message command");
    let mut responses: Vec<String> = Vec::new();
    if let Some(args) = params.opt_params {
        if args.len() < 2 {
            return;
        }
        // if there are more than two arguments,
        // concatenate the remainder to one string
        let end_index = args.len();
        println!("target is {} and content is {}", args[0], args[1]);
        let recipient_string = args[0].clone();
        let msg_vec_slice = &args[1 .. end_index];
        let message = msg_vec_slice.join(" ");
        println!("processed message = {}", message);
        // loop over targets
        let split: Vec<&str> = recipient_string.split(',').collect();
        for target_str in split.iter() {
            let result = match lookup_name(irc, &target_str) {
                Some(target) => match target {
                    NamedEntity::User(user_ptr) => {
                        println!("found a user target! nickname: {}", target_str);
                        let user_arc = Weak::upgrade(&user_ptr).unwrap();
                        let user_ro = user_arc.read().unwrap();
                        let client_maybe = lookup_client(irc, &user_ro.id);
                        if let Some(client_ptr) = client_maybe {
                            let client_arc = Weak::upgrade(&client_ptr).unwrap();
                            let mut client = client_arc.write().unwrap();
                            client.send_line(&message);
                        }
                        None
                    }
                    NamedEntity::Chan(chan_ptr) => {
                        None
                    }
                },
                None => Some(String::from("No such nick/channel"))
            };
            if let Some(reply) = result {
                responses.push(String::from(&reply))
            }
        }
    }
    // according to the RFC server should never respond to
    // NOTICE messages, no matter how they fail
    match msg_type {
        MsgType::PrivateMsg => {
            let client_option = lookup_client(irc, &my_user.id);
            if let Some(pointer) = client_option {
                let arc_pointer = Weak::upgrade(&pointer).unwrap();
                let mut rw_pointer = arc_pointer.write().unwrap();
                for response in responses.iter() {
                    rw_pointer.send_line(&response);
                }
            }
        },
        MsgType::Notice => return
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
                    Some(ClientType::ProtoUser(Arc::new(RwLock::new(ProtoUser {
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
                    Some(ClientType::ProtoUser(Arc::new(RwLock::new(ProtoUser {
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
                        Some(ClientType::ProtoUser(Arc::new(RwLock::new(ProtoUser {
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
