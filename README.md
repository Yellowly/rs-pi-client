# RSPI Process Manager
Command line application that aims to provide a means of running and managing simple shell commands on a Raspberry PI server remotely.

This functions similarly to Secure Shell (SSH), but with the goal of allowing clients to manage long-running processes as well.

# RSPI Client
This repo is the client side of the RSPI Process Manager. See the server-side repo [here]([http://example.com](https://github.com/Yellowly/rs-pi-server)

# Usage
Before running the executable for this, make sure you define the following environment variables:
- RSPI_SERVER_ADDR = Server address to connect to, ie. "127.0.0.1:8080"
- RSPI_SERVER_HASHKEY = An unsigned 64-bit integer used to encrypt data sent between client and server
- RSPI_SERVER_PASS = Any string less than 64 bytes long that the client must send as their first message to connect to the server

Then, simply run the executable.

Once you connect to the server, you can run most basic shell commands such as 'ls', 'cd', 'cat', etc. These commands get run on the server, and their outputs get sent back to the client.

## RSPI commands
If the client sends a message starting with 'rspi' and it matches one of the following commands, the server will run some extra functions:
- rspi procs: lists all running processes which are being managed by the RSPI Server
- rspi orphan: run this *during* the execution of a process to give up client control of the process. the process will be managed by the server.
- rspi adopt [process index]: returns control of a process back to the client, allowing the client to input to the process's stdin and read its stdout. note that while the client is in control of
a process, if that client disconnects, then the process will be killed (the server does not take control of processes unless explicitely given control with 'rspi orphan')
- rspi sendfile [file]: sends a file from the client's local machine to the current working directory of the client session on the server.
- rspi getfile [file]: sends a file from the server to the client's local machine. 
