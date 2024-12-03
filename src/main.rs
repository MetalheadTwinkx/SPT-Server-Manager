use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

macro_rules! erase_line {
    () => {
        print!("\x1b[A\x1b[2K");
        std::io::stdout().flush().unwrap();
    };
}

#[allow(dead_code)]
enum CommandMessage {
    StartServer,
    StopServer,
    RestartServer,
    SendCommand(String),
    UpdateServerPath(String),
    Exit,
}

enum LogTag {
    ServerManager,
    ConsoleInput,
    Warning,
    Error,
    Restart,
    Help,
}

impl LogTag {
    fn tag(&self) -> &'static str {
        match self {
            LogTag::ServerManager => "\x1b[30m\x1b[48;5;134m[Server-Manager]\x1b[0m",
            LogTag::ConsoleInput => "\x1b[30m\x1b[48;5;195m > \x1b[0m",
            LogTag::Warning => "\x1b[30m\x1b[48;5;214m ! \x1b[0m",
            LogTag::Error => "\x1b[30m\x1b[41m[ERROR]\x1b[0m",
            LogTag::Restart => "\x1b[30m\x1b[44m 🔄 \x1b[0m",
            LogTag::Help => "\x1b[30m\x1b[44m[HELP]\x1b[0m",
        }
    }
}

enum Color {
    Red,
    Green,
    Blue,
    Blurp,
    Orange,
    Reset,
    PaleMint,
    Magenta,
}

impl Color {
    fn text(&self) -> &'static str {
        match self {
            Color::Red => "\x1b[31m",
            Color::Green => "\x1b[32m",
            Color::PaleMint => "\x1b[38;5;195m",
            Color::Blue => "\x1b[34m",
            Color::Blurp => "\x1b[38;5;111m",
            Color::Orange => "\x1b[38;5;214m", // 256-color orange
            Color::Reset => "\x1b[0m",
            Color::Magenta => "\x1b[35m",
        }
    }
}

struct ServerManager {
    server_path: String,
    server_dir: String,
    current_process: Option<Child>,
}

impl ServerManager {
    fn new(server_path: &str) -> Self {
        let server_dir = Path::new(server_path)
            .parent()
            .unwrap_or_else(|| {
                eprintln!("Invalid server path provided.");
                std::process::exit(1);
            })
            .to_string_lossy()
            .into_owned();

        ServerManager {
            server_path: server_path.to_string(),
            server_dir,
            current_process: None,
        }
    }

    fn start_server(&mut self) -> io::Result<()> {
        // Ensure any existing process is terminated
        self.stop_server();

        // Start new server process
        let child = Command::new(&self.server_path)
            .current_dir(&self.server_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()?;

        println!("{} Server started with PID: {}{}{}\n",LogTag::ServerManager.tag(), Color::Magenta.text() ,child.id(), Color::Reset.text());
        self.current_process = Some(child);
        Ok(())
    }

    fn stop_server(&mut self) {
        if let Some(mut process) = self.current_process.take() {
            // Attempt to gracefully terminate the server
            if let Some(ref mut stdin) = process.stdin {
                let _ = stdin.write_all(b"exit\n");
                let _ = stdin.flush();
            }

            // Give the server some time to shut down gracefully
            thread::sleep(std::time::Duration::from_secs(2));

            // Check if the process has exited
            match process.try_wait() {
                Ok(Some(_status)) => {
                    println!("Server stopped gracefully");
                }
                Ok(None) => {
                    // Force kill if still running
                    let _ = process.kill();
                    let _ = process.wait();
                    println!("Server stopped forcefully");
                }
                Err(e) => {
                    eprintln!("{} Error while stopping server: {}", LogTag::Error.tag(), e);
                }
            }
        }
    }

    fn pipe_output(&mut self) -> io::Result<()> {
        if let Some(ref mut process) = self.current_process {
            if let Some(stdout) = process.stdout.take() {
                Self::pipe_stream(stdout, false);
            }
            if let Some(stderr) = process.stderr.take() {
                Self::pipe_stream(stderr, true);
            }
        }
        Ok(())
    }

    fn pipe_stream<R: 'static + Send + io::Read>(reader: R, is_stderr: bool) {
        let buf_reader = io::BufReader::new(reader);
        thread::spawn(move || {
            for line in buf_reader.lines() {
                match line {
                    Ok(line) => {
                        if is_stderr {
                            eprintln!("{}", line);
                        } else {
                            println!("{}", line);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Error reading process output: {}", LogTag::Error.tag(), e);
                        break;
                    }
                }
            }
        });
    }

    fn send_command(&mut self, command: &str) -> io::Result<()> {
        if let Some(ref mut process) = self.current_process {
            if let Some(ref mut stdin) = process.stdin {
                writeln!(stdin, "{}", command)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Stdin is not available",
                ));
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Process is not running",
            ));
        }
        Ok(())
    }
}

fn display_help() {
    println!(
        "\n{} Available commands:
    {}help{}       - Display this help message
    {}exit{}       - Stop the server and exit the program
    {}restart{}    - Restart the server
    {}setpath{}    - Change the server executable path
    [command]  - Any other input will be sent to the server as a command\n{}",
        LogTag::Help.tag(),
        Color::Blue.text(), Color::Reset.text(),
        Color::Red.text(), Color::Reset.text(),
        Color::Blue.text(), Color::Reset.text(),
        Color::Blurp.text(), Color::Reset.text(),
        Color::Reset.text());
}

fn main() -> io::Result<()> {

    // Attempt to read the server path from a config file
    let server_path = match std::fs::read_to_string("SPTSMconfig.txt") {
        Ok(path) => clean_path(&path),
        Err(_) => {
            // If reading the config file fails, attempt to find the server in the same directory
            let current_exe = std::env::current_exe()?;
            let current_dir = current_exe.parent().unwrap();
            let default_server_path = current_dir.join("SPT.Server.exe");

            if default_server_path.exists() {
                default_server_path.to_string_lossy().to_string()
            } else {
                // Prompt the user to input the server path
                println!(
                    "{}{}{} Could not find SPT.Server.exe in the current directory.{}",
                    LogTag::ServerManager.tag(),
                    LogTag::Warning.tag(),
                    Color::Orange.text(),
                    Color::Reset.text()
                );
                print!(
                    "{}{} Please enter the path to SPT.Server.exe: {}",
                    LogTag::ConsoleInput.tag(),
                    Color::PaleMint.text(),
                    Color::Blurp.text()
                );
                io::stdout().flush()?; // Ensure the prompt is displayed before waiting for input
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                print!("{}", Color::Reset.text());
                let input = clean_path(&input);

                // Ask if the user wants to remember this path
                print!(
                    "- Do you want to remember this path for future executions? ({}Y{}/{}N{}):\n{}{} ",
                    Color::Green.text(),
                    Color::Reset.text(),
                    Color::Red.text(),
                    Color::Reset.text(),
                    LogTag::ConsoleInput.tag(),
                    Color::PaleMint.text()
                );
                io::stdout().flush()?; // Ensure the prompt is displayed before waiting for input
                let mut remember = String::new();
                io::stdin().read_line(&mut remember)?;
                if remember.trim().eq_ignore_ascii_case("Y") {
                    // Save the path to the config file
                    std::fs::write("SPTSMconfig.txt", &input)?;
                }
                print!("{}", Color::Reset.text());

                input
            }
        }
    };

    println!(
        "- Using server path: {}{}{}\n\n", Color::Blurp.text(), server_path, Color::Reset.text());
    let mut server_manager = ServerManager::new(&server_path);

    // Start the server
    if let Err(e) = server_manager.start_server() {
        eprintln!(
            "{}{} Failed to start server: {}{}", LogTag::Error.tag(), Color::Red.text(), Color::Reset.text(), e,
        );

        println!("Press Enter to close this window...");
        // Wait for the user to press Enter
        let _ = io::stdin().read_line(&mut String::new());

        return Ok(());
    }

    // Cleans the input path by removing leading/trailing quotes, replacing backslashes with forward slashes,
    fn clean_path(path: &str) -> String {
        // Trim leading and trailing whitespace
        let trimmed = path.trim();
        // Remove leading quotes
        let cleaned = trimmed.trim_start_matches('"');
        // Remove trailing quotes if they exist
        let cleaned = cleaned.trim_end_matches('"');
        // Replace backslashes with forward slashes
        let cleaned_path = cleaned.replace('\\', "/");
        cleaned_path.to_string()
    }

    // Pipe output
    server_manager.pipe_output()?;

    // Create a channel for command communication
    let (cmd_tx, cmd_rx) = mpsc::channel::<CommandMessage>();

    // Spawn a thread to handle user input
    let cmd_tx_clone = cmd_tx.clone();
    let input_thread = thread::spawn(move || {
        println!("Type '{}help{}' to see available commands.\n", Color::Blue.text(), Color::Reset.text());
        loop {
            print!("> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break; // Exit if there's an error reading input
            }
            let command = input.trim().to_string();

            match command.as_str() {
                "help" => {
                    erase_line!();
                    display_help();
                }
                "exit" => {
                    erase_line!();
                    let _ = cmd_tx_clone.send(CommandMessage::Exit);
                    break;
                }
                "restart" => {
                    erase_line!();
                    println!("{}{}{} Restarting server...{}", LogTag::ServerManager.tag(), LogTag::Restart.tag(), Color::Blue.text(), Color::Reset.text());
                    let _ = cmd_tx_clone.send(CommandMessage::RestartServer);
                }
                "setpath" => {
                    erase_line!();
                    // Prompt the user for the new path
                    println!("- Please enter the new path to SPT.Server.exe:");
                    let mut path_input = String::new();
                    io::stdin().read_line(&mut path_input).unwrap();
                    let new_path = path_input.trim().to_string();

                    // Ask if the user wants to remember this path
                    print!(
                        "- Do you want to remember this path for future executions? ({}Y{}/{}N{}):\n{}{} ",
                        Color::Green.text(),
                        Color::Reset.text(),
                        Color::Red.text(),
                        Color::Reset.text(),
                        LogTag::ConsoleInput.tag(),
                        Color::PaleMint.text()
                    );
                    let mut remember = String::new();
                    io::stdin().read_line(&mut remember).unwrap();

                    if remember.trim().eq_ignore_ascii_case("Y") {
                        // Save the path to the config file
                        if let Err(e) = std::fs::write("SPTSMconfig.txt", &new_path) {
                            eprintln!("{} Failed to save path to config file: {}", LogTag::Error.tag(), e);
                        }
                    }

                    // Send the new path to the main thread
                    let _ = cmd_tx_clone.send(CommandMessage::UpdateServerPath(new_path));
                }
                _ => {
                    // Send other commands to the server's stdin
                    let _ = cmd_tx_clone.send(CommandMessage::SendCommand(command));
                }
            }
        }
    });

    // Main thread handles commands received from the input thread
    for message in cmd_rx {
        match message {
            CommandMessage::StartServer => {
                if let Err(e) = server_manager.start_server() {
                    eprintln!("Failed to start server: {}", e);
                } else {
                    let _ = server_manager.pipe_output();
                }
            }
            CommandMessage::StopServer => {
                server_manager.stop_server();
            }
            CommandMessage::RestartServer => {
                server_manager.stop_server();
                if let Err(e) = server_manager.start_server() {
                    eprintln!("{} Failed to restart server: {}", LogTag::Error.tag(), e);
                } else {
                    let _ = server_manager.pipe_output();
                }
            }
            CommandMessage::SendCommand(cmd) => {
                if let Err(e) = server_manager.send_command(&cmd) {
                    eprintln!("{} Failed to send command to server: {}", LogTag::Error.tag(), e);
                }
            }
            CommandMessage::UpdateServerPath(new_path) => {
                server_manager.stop_server();
                server_manager = ServerManager::new(&new_path);
                if let Err(e) = server_manager.start_server() {
                    eprintln!("{} Failed to start server with new path: {}", LogTag::Error.tag(), e);
                } else {
                    let _ = server_manager.pipe_output();
                }
            }
            CommandMessage::Exit => {
                server_manager.stop_server();
                break;
            }
        }
    }

    // Wait for the input thread to finish
    input_thread.join().unwrap();

    Ok(())
}
