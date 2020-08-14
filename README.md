# rusty-ircd
Goal is Rust implementation of an irc daemon. Primarily the project is a sandbox for the author to play with while learning Rust.

However, the intention is not to fall prey to the notion that nobody will ever read/use this code.
Therefore, efforts will be made to prioritise readable, maintainable code that is extensible and supports e.g. i18n.
Indeed these will be valuable tools for the author to learn about, too.

## Branches
### main << merge tokio-v0.2-port
The ported code to tokio 0.2 compiles and runs so is now merged to main.
As the name suggests, it's an overhaul of the codebase, moving to tokio 0.2 and the new Rust async/.await model.
Current target for that branch is:
* code that compiles, <-- check
* code that runs correctly, <-- need more testing
* implementation of NICK/USER login handshake, and PRIVMSG/NOTICE between logged in clients. <-- yes
* NB an erroneous client command will cause rusty-ircd to drop the client, rather than sending an error message, which would be the intended behaviour

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
