//! Client: `taskmon SOCKET [COMMAND...]`.
//! Sends the command, or each line of stdin if there is none, and prints the replies.
use std::{
    env::args,
    io::{self, stdin, stdout, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    thread,
};

fn main() -> io::Result<()> {
    let mut args = args().skip(1);
    let Some(socket_path) = args.next() else {
        eprintln!("usage: taskmon SOCKET [COMMAND...]");
        std::process::exit(2);
    };
    let command = args.collect::<Vec<_>>().join(" ");
    let mut stream = UnixStream::connect(socket_path)?;
    let mut replies = stream.try_clone()?;
    let printer = thread::spawn(move || io::copy(&mut replies, &mut stdout()));
    if command.is_empty() {
        io::copy(&mut stdin(), &mut stream)?;
    } else {
        writeln!(stream, "{command}")?;
    }
    stream.shutdown(Shutdown::Write)?;
    printer.join().expect("printer thread panicked")?;
    Ok(())
}
