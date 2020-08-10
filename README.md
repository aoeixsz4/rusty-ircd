# rusty-ircd
Goal is Rust implementation of an irc daemon.

main branch is now rather outdated and uses tokio 0.1 as an async IO library.
Clients can connect and any text is forwarded to all other clients.
tokio-0.2-port branch is much more interesting but doesn't yet compile.
As the branch name suggests, it's an overhaul of the codebase, moving to tokio 0.2 and the new Rust async/.await model.
Current target for that branch is:
  a) code that compiles,
  b) code that runs correctly,
  c) implementation of NICK/USER login handshake, and PRIVMSG/NOTICE between logged in clients.
The next target after that will be i18n support, and channels with JOIN/PART.

Stay tuned folks!

-- aoei/Joanna
