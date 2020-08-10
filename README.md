# rusty-ircd
Goal is Rust implementation of an irc daemon.

## Branches
### main
This branch is now rather outdated and uses tokio 0.1 as an async IO library.
Clients can connect and any text is forwarded to all other clients.

### tokio-v0.2-port
This branch is much more interesting but doesn't yet compile.
As the name suggests, it's an overhaul of the codebase, moving to tokio 0.2 and the new Rust async/.await model.
Current target for that branch is:
* code that compiles,
* code that runs correctly,
* implementation of NICK/USER login handshake, and PRIVMSG/NOTICE between logged in clients.

## Future targets
### Development Workflow
* podman container deployment
* unit tests
* regular linting
* general intergration tests
* IRC bots for integration testing

### Features
* i18n support
* channel support with JOIN/PART
* support for additional server nodes

Stay tuned folks!

-- aoei/Joanna
