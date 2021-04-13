# rusty-ircd
Goal is Rust implementation of an irc daemon. Primarily the project is a sandbox for the author to play with while learning Rust.

However, the intention is not to fall prey to the notion that nobody will ever read/use this code.
Therefore, efforts will be made to prioritise readable, maintainable code that is extensible and supports e.g. i18n.
Indeed these will be valuable tools for the author to learn about, too.

This software is licensed under the Mozilla Public License v2.0.

## Branches
### irc-proto-port (not yet begun)
This may be worth implementing before error handling, given the irc-proto crate appears to include definitions of protocol errors.
The irc_proto library (written by Aaron Weiss) was recommended by kerio (of Freenode IRC/#ascension.run fame). Thanks kerio!
Porting to use this library most likely means we can throw away parser.rs, forget re-implementing the RFC and focus simply on
server features, i18n and encrypted connectivity. Fantastic. :D This will be a major target for rusty-ircd version 0.3.0.

Thanks Aaron Weiss!
Don't be surprised if I make some PRs to your library.
irc_proto (https://docs.rs/irc-proto/0.14.0/irc_proto/) appears to be a fairly
complete implementation of IRCv3 (https://ircv3.net/irc/) which is based on the core RFCs (https://tools.ietf.org/html/rfc1459,
https://tools.ietf.org/html/rfc2812 and https://tools.ietf.org/html/rfc7194).

### release-0.3.0 (done)
Targets for this release:
* correct handling of errors <-- made some progress on this but need to write test suites
* i18n support <-- didn't do this yet
* SSL support <-- works!
* NICK/USER/PRIVMSG/NOTICE/JOIN/TOPIC/PART/LIST <-- these commands are so far implemented

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
* PING/PONG support (weechat clients will repeatedly reconnect, believing the connection to the server has died)
* i18n support (FR from stenno)
* ~~channel support with JOIN/PART <-- done
* support for additional server nodes
* ~~SSL encrypted connectivity <-- done

Stay tuned folks!

-- aoei/Joanna
openssl pkcs12 -export -out identity.pfx -inkey rusty-aoei.key -in legit-self-auth.crt
